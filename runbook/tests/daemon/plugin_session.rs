//! The Claude Code plugin in `plugin/` serves a session's delegation (`docs/model-routes.md`, Part two): its
//! `PreToolUse` hook denies every Agent call with a reason naming the runbook MCP tool `delegate` and the session's
//! route, its MCP server is `runbook mcp` on the session's daemon, and `delegate` runs the task as a Runbook run and
//! hands its result back to the session. A route that is down is a tool error naming the route, and the Agent call
//! stays denied: nothing runs on the session's own model.
//!
//! No model is called and nothing leaves loopback. One stand-in Anthropic Messages server on a port of the test's own
//! answers every model call: the session's own (it plays the model: an Agent call first, then `delegate` by the name
//! the deny reason gives, loaded first with ToolSearch as the reason says when the session lists it as a deferred tool,
//! then the tool's result as its answer) and the run's, which reaches it through the daemon's relay as the daemon's
//! gateway. The session runs with Claude Code's tool search at its default (on), so `delegate` is deferred. The run's agent node is the stand-in `claude` of `delegate.rs` on the daemon's `PATH`.
//!
//! The headless cases start the `claude` found on `PATH` with `--plugin-dir plugin`, and print why they skip when
//! there is none (the forge has none). That session gets a fresh `HOME` and `CLAUDE_CONFIG_DIR` under the test's own
//! directory, `/` as its working directory and only the user setting source (both keep it from reading a `.claude`
//! directory above a working directory), a token the case invents, and an HTTP proxy that refuses every connection,
//! so any call other than to loopback fails.
//! Each headless case runs again in a process of its own inside a user and network namespace whose one interface is
//! loopback (`unshare -rn`), with the stand-in, the daemon and the session: nothing it starts reaches past loopback.

use super::default_confinement::{rows, Live};
use super::delegate::{daemon_at, until_ended, Mcp, ANSWER, ROUTE};
use super::gateway::{claude_cli, model_table, repo};
use super::http::{exits_within, refusing_port};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const BIN: &str = env!("CARGO_BIN_EXE_runbook");
/// The task the session's model hands to a sub-agent.
const TASK: &str = "Write one sentence about tea.";
/// The ids of the session model's tool calls: Agent, ToolSearch (when `delegate` is deferred) and `delegate`.
const AGENT_CALL: &str = "toolu_agent_1";
const SEARCH_CALL: &str = "toolu_search_1";
const DELEGATE_CALL: &str = "toolu_delegate_1";
/// What the stand-in model answers once it has the result of `delegate`, before that result.
const SAYS: &str = "The sub-agent answered: ";
/// The session's credential: a string the case invents.
const SESSION_TOKEN: &str = "stand-in-session-token";
/// The name Claude Code gives the tool `delegate` of the plugin `runbook`'s MCP server `runbook`.
const DELEGATE_TOOL: &str = "mcp__plugin_runbook_runbook__delegate";

/// How the stand-in serves the daemon's side: as a gateway that is ready and answers calls, one that is ready and
/// refuses every call, or one that is not ready.
#[derive(Clone, Copy, PartialEq)]
enum Gate {
    Ready,
    CallsFail,
    Down,
}

/// One request the stand-in got: its head and its JSON body.
#[derive(Clone, Debug)]
struct Got {
    head: String,
    body: Value,
}

struct Standin {
    base_url: String,
    got: Arc<Mutex<Vec<Got>>>,
}

impl Standin {
    fn got(&self) -> Vec<Got> {
        self.got.lock().unwrap().clone()
    }

    /// The Messages calls of the session: those that offer tools.
    fn session_calls(&self) -> Vec<Got> {
        self.got().into_iter().filter(|g| g.head.starts_with("POST ") && g.body["tools"].is_array()).collect()
    }

    /// The Messages calls that came through the daemon's relay: those without tools.
    fn run_calls(&self) -> Vec<Got> {
        self.got().into_iter().filter(|g| g.head.starts_with("POST ") && !g.body["tools"].is_array()).collect()
    }
}

fn standin(gate: Gate) -> Standin {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let got = Arc::new(Mutex::new(Vec::new()));
    let kept = got.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let kept = kept.clone();
            std::thread::spawn(move || serve_one(stream, gate, &kept));
        }
    });
    Standin { base_url, got }
}

fn serve_one(mut stream: std::net::TcpStream, gate: Gate, kept: &Mutex<Vec<Got>>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let mut raw = Vec::new();
    let mut byte = [0u8; 1];
    while !raw.ends_with(b"\r\n\r\n") && raw.len() < 64 * 1024 {
        match stream.read(&mut byte) {
            Ok(1) => raw.push(byte[0]),
            _ => return,
        }
    }
    let head = String::from_utf8_lossy(&raw).into_owned();
    let length = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
    let mut body = vec![0u8; length];
    if stream.read_exact(&mut body).is_err() {
        return;
    }
    let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    kept.lock().unwrap().push(Got { head: head.clone(), body: body.clone() });
    let line = head.lines().next().unwrap_or("");
    let path = line.split_whitespace().nth(1).unwrap_or("");
    let error = |code: &str| json!({"type": "error", "error": {"type": code, "message": "the stand-in gateway refuses"}}).to_string();
    let (status, kind, payload) = if line.starts_with("GET ") {
        match gate {
            Gate::Down if path.ends_with("/ready") => ("503 Service Unavailable", "application/json", error("overloaded_error")),
            // the daemon asks a route's gateway which models it serves before a run starts: the route `default` is `a-model`
            _ if path.ends_with("/v1/models") => ("200 OK", "application/json", json!({"object": "list", "data": [{"id": "a-model", "wires": ["messages"]}]}).to_string()),
            _ => ("200 OK", "application/json", "{}".to_owned()),
        }
    } else if path.contains("/v1/messages") && body["tools"].is_array() {
        let (content, stop) = session_turn(&body);
        if body["stream"] == json!(true) {
            ("200 OK", "text/event-stream", events(&body, &content, stop))
        } else {
            ("200 OK", "application/json", message(&body, &content, stop).to_string())
        }
    } else if path.contains("/v1/messages") {
        match gate {
            Gate::CallsFail => ("503 Service Unavailable", "application/json", error("overloaded_error")),
            _ => ("200 OK", "application/json", message(&body, &[json!({"type": "text", "text": "ok"})], "end_turn").to_string()),
        }
    } else {
        ("404 Not Found", "application/json", "{}".to_owned())
    };
    let _ = write!(stream, "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}", payload.len());
}

/// The text of a tool result: its content as text, or the texts of its blocks.
fn result_text(r: &Value) -> String {
    match &r["content"] {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks.iter().filter_map(|b| b["text"].as_str()).collect(),
        _ => String::new(),
    }
}

/// The route a deny reason tells the model to name: the text between "`model_route` = `" and the next backtick.
fn route_named(reason: &str) -> Option<String> {
    let rest = reason.split_once("`model_route` = `")?.1;
    Some(rest.split_once('`')?.0.to_owned())
}

/// The tool a deny reason tells the model to load: the name in "`select:<name>`".
fn tool_to_load(reason: &str) -> Option<String> {
    let rest = reason.split_once("`select:")?.1;
    Some(rest.split_once('`')?.0.to_owned())
}

/// The result of the tool call `id` among the messages of a request, if any.
fn result_of(body: &Value, id: &str) -> Option<Value> {
    body["messages"].as_array().into_iter().flatten().flat_map(|m| m["content"].as_array().cloned().unwrap_or_default()).find(|c| c["type"] == "tool_result" && c["tool_use_id"] == id)
}

/// The session's model, played, knowing nothing of the plugin but what the deny reason says: with no tool result yet
/// it calls Agent with the task. Given the deny, it takes the tool the reason names (`select:<name>`): when the request
/// offers it, it calls it (the task as `request`, the route the reason names); when it does not (the tool is deferred:
/// Claude Code's tool search lists it by name only), it loads it with ToolSearch and calls it once that has answered.
/// Given any other result it answers that result's text.
fn session_turn(body: &Value) -> (Vec<Value>, &'static str) {
    let tools: Vec<&str> = body["tools"].as_array().into_iter().flatten().filter_map(|t| t["name"].as_str()).collect();
    let last = body["messages"].as_array().and_then(|m| m.last()).cloned().unwrap_or(Value::Null);
    let results: Vec<&Value> = last["content"].as_array().into_iter().flatten().filter(|c| c["type"] == "tool_result").collect();
    let tool_use = |id: &str, name: &str, input: Value| (vec![json!({"type": "tool_use", "id": id, "name": name, "input": input})], "tool_use");
    let deny = result_of(body, AGENT_CALL).map(|r| result_text(&r)).unwrap_or_default();
    let delegate = |name: &str| {
        let mut input = json!({"request": TASK});
        if let Some(route) = route_named(&deny) {
            input["model_route"] = json!(route);
        }
        tool_use(DELEGATE_CALL, name, input)
    };
    match (results.last(), tool_to_load(&deny)) {
        (Some(r), Some(name)) if r["tool_use_id"] == AGENT_CALL && tools.contains(&name.as_str()) => delegate(&name),
        (Some(r), Some(name)) if r["tool_use_id"] == AGENT_CALL && tools.contains(&"ToolSearch") => tool_use(SEARCH_CALL, "ToolSearch", json!({"query": format!("select:{name}")})),
        (Some(r), Some(name)) if r["tool_use_id"] == SEARCH_CALL => delegate(&name),
        (Some(r), _) => (vec![json!({"type": "text", "text": format!("{SAYS}{}", result_text(r))})], "end_turn"),
        (None, _) if tools.contains(&"Agent") => tool_use(AGENT_CALL, "Agent", json!({"description": "tea", "prompt": TASK, "subagent_type": "general-purpose"})),
        (None, _) => (vec![json!({"type": "text", "text": "this session has no Agent tool"})], "end_turn"),
    }
}

fn message(body: &Value, content: &[Value], stop: &str) -> Value {
    json!({"id": "msg_standin", "type": "message", "role": "assistant", "model": body["model"], "content": content, "stop_reason": stop, "stop_sequence": null, "usage": {"input_tokens": 11, "output_tokens": 5}})
}

/// `message` as the events of a Messages stream.
fn events(body: &Value, content: &[Value], stop: &str) -> String {
    let mut out = Vec::new();
    let mut start = message(body, &[], "");
    start["stop_reason"] = Value::Null;
    out.push(("message_start", json!({"type": "message_start", "message": start})));
    for (i, block) in content.iter().enumerate() {
        let (opened, delta) = if block["type"] == "text" {
            (json!({"type": "text", "text": ""}), json!({"type": "text_delta", "text": block["text"]}))
        } else {
            (json!({"type": "tool_use", "id": block["id"], "name": block["name"], "input": {}}), json!({"type": "input_json_delta", "partial_json": block["input"].to_string()}))
        };
        out.push(("content_block_start", json!({"type": "content_block_start", "index": i, "content_block": opened})));
        out.push(("content_block_delta", json!({"type": "content_block_delta", "index": i, "delta": delta})));
        out.push(("content_block_stop", json!({"type": "content_block_stop", "index": i})));
    }
    out.push(("message_delta", json!({"type": "message_delta", "delta": {"stop_reason": stop, "stop_sequence": null}, "usage": {"output_tokens": 5}})));
    out.push(("message_stop", json!({"type": "message_stop"})));
    out.iter().map(|(name, data)| format!("event: {name}\ndata: {data}\n\n")).collect()
}

/// The daemon of `delegate.rs` (`runbook serve` with the Loom runtime, the route `default`) with the stand-in as its
/// gateway.
fn daemon(tag: &str, gw: &Standin) -> Live {
    daemon_at(tag, &gw.base_url, &format!("{}{ROUTE}", model_table(Some(&claude_cli())))).1
}

/// The `claude` on `PATH`, or `None`.
fn claude_on_path() -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join("claude")).find(|p| std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0))
}

/// Set in the process [`in_own_network`] starts inside its own network namespace.
const OWN_NETWORK: &str = "RUNBOOK_TEST_OWN_NETWORK";

/// Whether this process goes on with the headless case `case` (a test of this module, by its function name). Without a
/// `claude` on `PATH` it does not, and says why. Inside its own network namespace it does. Elsewhere it runs the case
/// again in a process of its own inside a user and network namespace whose one interface is loopback (`unshare -rn`,
/// then `ip link set lo up`), asserts that run passed, and does not go on itself.
fn in_own_network(case: &str) -> bool {
    if claude_on_path().is_none() {
        println!("skipped: no `claude` on PATH, so no headless Claude Code session can run here");
        return false;
    }
    if std::env::var_os(OWN_NETWORK).is_some() {
        return true;
    }
    let name = format!("{}::{case}", module_path!().split_once("::").map_or(module_path!(), |(_, rest)| rest));
    let mut cmd = Command::new("unshare");
    cmd.args(["-rn", "sh", "-c", "ip link set lo up && exec \"$0\" \"$@\""])
        .arg(std::env::current_exe().unwrap())
        .args(["--exact", &name, "--nocapture", "--test-threads", "1"])
        .env(OWN_NETWORK, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let (status, out, err) = exits_within(cmd, Duration::from_secs(55));
    assert!(status == Some(0) && out.contains("1 passed") && !out.contains("skipped:"), "the case did not pass inside its own network namespace (exit {status:?}):\n{out}\n{err}");
    false
}

/// A headless Claude Code session with the plugin, on the daemon `l` and the stand-in model `gw`, given the prompt
/// `prompt`: the JSON result it prints. `None`, with the reason printed, when no `claude` is on `PATH`.
fn headless(tag: &str, l: &Live, gw: &Standin, prompt: &str) -> Option<Value> {
    let Some(claude) = claude_on_path() else {
        println!("skipped: no `claude` on PATH, so no headless Claude Code session can run here");
        return None;
    };
    let base = super::state::scratch(&format!("{tag}-session"));
    let (home, bin, tmp) = (base.join("home"), base.join("bin"), base.join("tmp"));
    for d in [home.join(".claude"), bin.clone(), tmp.clone()] {
        std::fs::create_dir_all(d).unwrap();
    }
    std::os::unix::fs::symlink(std::fs::canonicalize(&claude).unwrap(), bin.join("claude")).unwrap();
    std::os::unix::fs::symlink(BIN, bin.join("runbook")).unwrap();
    let (_refusing, port) = refusing_port();
    let proxy = format!("http://127.0.0.1:{port}");
    let mut cmd = Command::new(bin.join("claude"));
    cmd.current_dir("/")
        .env_clear().envs(crate::kept())
        // the `runbook mcp` the plugin starts reads no runbook.toml of the host
        .env("RUNBOOK_NO_CONFIG", "1")
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("HOME", &home)
        .env("CLAUDE_CONFIG_DIR", home.join(".claude"))
        .env("TMPDIR", &tmp)
        .env("ANTHROPIC_BASE_URL", &gw.base_url)
        .env("ANTHROPIC_AUTH_TOKEN", SESSION_TOKEN)
        .env("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1")
        .env("DISABLE_AUTOUPDATER", "1")
        .env("HTTPS_PROXY", &proxy)
        .env("HTTP_PROXY", &proxy)
        .env("NO_PROXY", "127.0.0.1,localhost")
        .env("RUNBOOK_DAEMON", format!("http://{}", l.addr))
        .env("RUNBOOK_MODEL_ROUTE", "default")
        .args(["-p", prompt, "--output-format", "json", "--setting-sources", "user", "--allowedTools", DELEGATE_TOOL, "--plugin-dir"])
        .arg(repo().join("plugin"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let (status, out, err) = exits_within(cmd, Duration::from_secs(50));
    let v: Value = serde_json::from_str(&out).unwrap_or_else(|e| panic!("claude printed no JSON result ({e}, exit {status:?}): {out}\n{err}"));
    Some(v)
}

/// The tool results of the session's last call, by the id of the tool call they answer.
fn results_of(gw: &Standin) -> std::collections::BTreeMap<String, Value> {
    let calls = gw.session_calls();
    let last = calls.last().expect("the session called no model");
    last.body["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|m| m["content"].as_array().cloned().unwrap_or_default())
        .filter(|c| c["type"] == "tool_result")
        .map(|c| (c["tool_use_id"].as_str().unwrap_or("").to_owned(), c))
        .collect()
}

/// The session's Agent call was denied by the hook with a reason naming `delegate` by the name the session lists it
/// under, how to load it, and the session's route, and no sub-agent ran: the session called its model three times
/// (Agent, `delegate`, the answer), and once more to load `delegate` with ToolSearch when the session had deferred it,
/// each with the case's token, and Claude Code spawned no sub-agent.
fn assert_agent_denied(out: &Value, gw: &Standin) {
    let results = results_of(gw);
    let agent = results.get(AGENT_CALL).unwrap_or_else(|| panic!("the Agent call has no result: {results:?}"));
    let reason = result_text(agent);
    assert_eq!(agent["is_error"], json!(true), "{agent}");
    for want in ["`delegate`", "RUNBOOK_MODEL_ROUTE", "`model_route` = `default`", TASK, &format!("`{DELEGATE_TOOL}`"), &format!("`select:{DELEGATE_TOOL}`"), "ToolSearch"] {
        assert!(reason.contains(want), "the deny reason lacks {want}: {reason}");
    }
    assert_eq!(out["subagent_stats"]["spawned"], json!(0), "a sub-agent ran: {out}");
    let calls = gw.session_calls();
    // the name the reason gives is the one Claude Code lists: offered as a tool, or named among the deferred tools
    let first = calls.first().expect("the session called no model");
    let offered = first.body["tools"].as_array().into_iter().flatten().any(|t| t["name"] == DELEGATE_TOOL);
    assert!(offered || first.body["messages"].to_string().contains(&format!("\\n{DELEGATE_TOOL}\\n")), "the session lists no tool {DELEGATE_TOOL}");
    let searched = usize::from(results.contains_key(SEARCH_CALL));
    assert_eq!(searched, usize::from(!offered), "ToolSearch was called (1) or not (0) where the tool was deferred (1) or offered (0)");
    assert_eq!(calls.len(), 3 + searched, "the session's model calls: {:?}", calls.iter().map(|c| &c.head).collect::<Vec<_>>());
    for c in &calls {
        assert!(c.head.lines().any(|l| l.eq_ignore_ascii_case(&format!("authorization: Bearer {SESSION_TOKEN}"))), "a call without the case's token: {}", c.head);
    }
}

// ---- the consumer's test 7: a headless session delegates through `delegate` ----

#[test]
fn a_headless_session_with_the_plugin_delegates_through_runbook_and_gets_the_runs_result() {
    if !in_own_network("a_headless_session_with_the_plugin_delegates_through_runbook_and_gets_the_runs_result") {
        return;
    }
    let gw = standin(Gate::Ready);
    let l = daemon("plg-session", &gw);
    let Some(out) = headless("plg-session", &l, &gw, "Hand the tea sentence to a sub-agent.") else { return };
    assert_eq!(out["is_error"], json!(false), "{out}");
    assert_agent_denied(&out, &gw);
    // `delegate` ran the task as a run on the route the reason named and answered its result.md
    let results = results_of(&gw);
    let delegated = results.get(DELEGATE_CALL).unwrap_or_else(|| panic!("`delegate` has no result: {results:?}"));
    let text = result_text(delegated);
    assert!(delegated["is_error"] != json!(true), "{delegated}");
    // Claude Code 2.1 hands the model the tool's structured content, the row of `delegation_result`, when the result has
    // one; a client that reads the text gets the line naming the run first.
    let run_id = match serde_json::from_str::<Value>(&text) {
        Ok(v) => {
            let row = &v["rows"][0];
            assert_eq!((row["run_state"].clone(), row["outcome"].clone(), row["route"].clone(), row["result_text"].clone()), (json!("Ended"), json!("delivered"), json!("default"), json!(format!("{ANSWER}\n"))), "{text}");
            row["run_id"].as_str().unwrap_or_else(|| panic!("no run id: {text}")).to_owned()
        }
        Err(_) => {
            let head = text.lines().next().unwrap_or("");
            assert!(head.starts_with("run ") && head.contains(" on route default: Ended (delivered)") && text.contains(ANSWER), "the first line names the run: {text}");
            head.trim_start_matches("run ").split(' ').next().unwrap().to_owned()
        }
    };
    let session_says = out["result"].as_str().unwrap_or("");
    assert!(session_says.starts_with(SAYS) && session_says.contains(ANSWER), "the session got the run's result back: {session_says}");
    // the run's model call reached the stand-in through the daemon's relay, with the route's alias
    let end = until_ended(&l, &run_id);
    assert_eq!((end["route"].clone(), end["path"].clone(), end["calls"].clone()), (json!("default"), json!("free"), json!(1)), "{end}");
    let relayed = gw.run_calls();
    assert_eq!(relayed.len(), 1, "{relayed:?}");
    assert_eq!(relayed[0].body["model"], "a-model");
    assert!(!relayed[0].head.contains(SESSION_TOKEN), "the session's token reached the gateway: {}", relayed[0].head);
}

#[test]
fn a_headless_session_whose_route_is_down_gets_a_tool_error_naming_the_route_and_runs_no_agent() {
    if !in_own_network("a_headless_session_whose_route_is_down_gets_a_tool_error_naming_the_route_and_runs_no_agent") {
        return;
    }
    let gw = standin(Gate::Down);
    let l = daemon("plg-session-down", &gw);
    let Some(out) = headless("plg-session-down", &l, &gw, "Hand the tea sentence to a sub-agent.") else { return };
    assert_agent_denied(&out, &gw);
    let results = results_of(&gw);
    let delegated = results.get(DELEGATE_CALL).unwrap_or_else(|| panic!("`delegate` has no result: {results:?}"));
    let text = result_text(delegated);
    assert_eq!(delegated["is_error"], json!(true), "{delegated}");
    assert!(text.contains("route-unavailable") && text.contains("route default"), "{text}");
    assert!(rows(&l, "/daemon/views/runs").is_empty(), "a run was created on a route that is down");
    assert!(gw.run_calls().is_empty(), "a model call reached the gateway of a route that is down");
}

// ---- `runbook mcp` alone: a route that is down is a tool error naming the route ----

/// `delegate` through `runbook mcp` on the daemon `l`: the result of the call.
fn mcp_delegate(l: &Live) -> Value {
    let mut m = Mcp::to(l);
    m.send(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "delegate", "arguments": {"request": TASK, "model_route": "default", "wait_seconds": 40}}}));
    m.until(2).pop().unwrap()["result"].clone()
}

#[test]
fn mcp_delegate_on_a_route_that_is_not_ready_is_a_tool_error_naming_the_route() {
    let gw = standin(Gate::Down);
    let l = daemon("plg-mcp-down", &gw);
    let r = mcp_delegate(&l);
    let text = r["content"][0]["text"].as_str().unwrap_or("");
    assert_eq!(r["isError"], json!(true), "{r}");
    assert!(text.contains("route-unavailable") && text.contains("route default"), "{text}");
    assert!(gw.run_calls().is_empty(), "{:?}", gw.run_calls());
}

#[test]
fn mcp_delegate_whose_route_fails_during_the_run_is_a_tool_error_naming_the_route() {
    let gw = standin(Gate::CallsFail);
    let l = daemon("plg-mcp-fails", &gw);
    let r = mcp_delegate(&l);
    let text = r["content"][0]["text"].as_str().unwrap_or("");
    assert_eq!(r["isError"], json!(true), "a run whose route refused its call answered as a success: {r}");
    assert!(text.contains("route-unavailable") && text.contains("route default"), "{text}");
    let calls = gw.run_calls();
    assert!(!calls.is_empty() && calls.iter().all(|c| c.body["model"] == "a-model"), "the route's calls, and no other route's: {calls:?}");
}

// ---- the plugin's words agree with what `runbook mcp` serves ----

/// Every runbook MCP tool, argument and member of a delegation's result the plugin names to the model (in backticks,
/// in the hook's deny reason, its session-start context and the skill `delegation`) is a tool `runbook mcp` lists, an
/// argument of one, or a member of the row of the view `delegation-result` that `delegate` answers. The session's route
/// is not a word of the plugin: it is a value with a dash here, which the check does not read as a name.
#[test]
fn the_plugin_names_only_tools_and_arguments_runbook_mcp_serves() {
    let hook = |event: &str, input: &str| {
        let mut child = Command::new("sh")
            .arg(repo().join("plugin/hooks/delegation.sh"))
            .arg(event)
            .env_clear().envs(crate::kept())
            .env("PATH", "/usr/bin:/bin")
            .env("RUNBOOK_MODEL_ROUTE", "qwen-local")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
        let out: Value = serde_json::from_slice(&child.wait_with_output().unwrap().stdout).unwrap();
        let h = &out["hookSpecificOutput"];
        h["permissionDecisionReason"].as_str().or(h["additionalContext"].as_str()).unwrap().to_owned()
    };
    let texts = [
        ("the deny reason", hook("pre-tool-use", &json!({"tool_name": "Agent", "tool_input": {"prompt": TASK}}).to_string())),
        ("the session-start context", hook("session-start", "{}")),
        ("the skill delegation", std::fs::read_to_string(repo().join("plugin/skills/delegation/SKILL.md")).unwrap()),
    ];
    let tools = runbook::mcp::tools::list();
    let mut known: Vec<String> = Vec::new();
    for t in tools {
        let name = t["name"].as_str().unwrap();
        // the name Claude Code lists the tool under (`DELEGATE_TOOL` is one): the prefix of the plugin's server, then the tool's
        known.push(format!("{}{name}", DELEGATE_TOOL.trim_end_matches("delegate")));
        known.push(name.to_owned());
        known.extend(t["inputSchema"]["properties"].as_object().into_iter().flatten().map(|(k, _)| k.clone()));
    }
    let row = &super::openapi::doc()["components"]["schemas"]["sdc.daemon.DelegationResult.Row"]["properties"];
    assert!(row.is_object(), "the document has no row of the view delegation-result");
    known.extend(row.as_object().into_iter().flatten().map(|(k, _)| k.clone()));
    let mut unknown = Vec::new();
    for (place, text) in &texts {
        for word in text.split('`').skip(1).step_by(2) {
            let snake = !word.is_empty() && word.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') && word.starts_with(|c: char| c.is_ascii_lowercase());
            if snake && !known.iter().any(|k| k == word) {
                unknown.push(format!("{place}: `{word}`"));
            }
        }
    }
    unknown.dedup();
    assert!(unknown.is_empty(), "the plugin names what `runbook mcp` does not serve: {unknown:?}");
}
