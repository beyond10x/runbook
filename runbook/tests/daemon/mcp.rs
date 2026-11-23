//! MCP over stdio (`runbook mcp`, a child process) and over streamable HTTP (the endpoint of
//! `runbook serve --mcp`, in process on port 0), held against the HTTP routes of the same daemon.

use super::commands::Fx;
use super::http::{send, Resp};
use runbook::api;
use runbook::mcp;
use runbook::stub::{Script, Step};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::net::SocketAddr;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_runbook");
const MODERN: &str = "2026-07-28";
const SEVEN: [&str; 7] = ["submit_run", "list_runs", "read_run", "read_events", "read_open_decisions", "answer_decision", "cancel_run"];

// ---- a daemon with the MCP endpoint, in process ----

struct Daemon {
    fx: Fx,
    addr: SocketAddr,
    rt: Option<tokio::runtime::Runtime>,
}

impl Daemon {
    /// The daemon of a fixture on `bind` (an address with port 0), with or without a token; the MCP
    /// endpoint is mounted as `runbook serve --mcp` mounts it.
    fn start(tag: &str, token: Option<&str>, bind: &str) -> Daemon {
        let fx = Fx::new(tag);
        fx.stub.script("book", Script::new(vec![Step::Emit { kind: "run.opened".into(), body: json!({"a": 1}) }, Step::Emit { kind: "run.closed".into(), body: json!({"b": 2}) }, Step::End("passed".into())]));
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        let listener = rt.block_on(tokio::net::TcpListener::bind(bind)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let (daemon, token) = (fx.d().clone(), token.map(String::from));
        rt.spawn(async move {
            let extra = mcp::router(daemon.clone());
            let _ = api::serve_extra(listener, daemon, token.as_deref(), api::Streams::default(), Some(extra), std::future::pending()).await;
        });
        Daemon { fx, addr: SocketAddr::from(([127, 0, 0, 1], port)), rt: Some(rt) }
    }

    fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    fn get(&self, path: &str) -> Resp {
        send(self.addr, "GET", path, &[], None)
    }

    fn command(&self, name: &str, body: &Value) -> Resp {
        send(self.addr, "POST", &format!("/daemon/commands/{name}"), &[("Content-Type", "application/json")], Some(body.to_string().as_bytes()))
    }

    fn mcp(&self, headers: &[(&str, &str)], body: &Value) -> Resp {
        let mut h = vec![("Content-Type", "application/json"), ("Accept", "application/json, text/event-stream")];
        h.extend_from_slice(headers);
        send(self.addr, "POST", "/mcp", &h, Some(body.to_string().as_bytes()))
    }

    /// A modern request with the headers of its revision.
    fn modern(&self, method: &str, params: Value) -> Resp {
        let name = params["name"].as_str().map(String::from);
        let mut h = vec![("MCP-Protocol-Version", MODERN), ("Mcp-Method", method)];
        if let Some(n) = &name {
            h.push(("Mcp-Name", n.as_str()));
        }
        self.mcp(&h, &modern_request(1, method, params))
    }

    fn wait_state(&self, run: &str, want: &str) {
        self.fx.wait_state(run, want);
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        if let Some(rt) = self.rt.take() {
            rt.shutdown_background();
        }
    }
}

fn modern_request(id: u64, method: &str, mut params: Value) -> Value {
    params["_meta"] = json!({
        "io.modelcontextprotocol/protocolVersion": MODERN,
        "io.modelcontextprotocol/clientInfo": {"name": "test", "version": "0"},
        "io.modelcontextprotocol/clientCapabilities": {},
    });
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

fn run_of(body: &Value) -> String {
    body["published"][0]["payload"]["run_id"].as_str().unwrap_or_else(|| panic!("no run id in {body}")).to_string()
}

fn names(tools: &Value) -> Vec<String> {
    tools.as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect()
}

/// The text of a body with the ids of runs replaced, to compare two answers about different runs.
fn without_ids(body: &Value, ids: &[&str]) -> String {
    ids.iter().fold(body.to_string(), |t, id| t.replace(id, "RUN"))
}

// ---- `runbook mcp` as a child process ----

struct Stdio1 {
    child: Child,
    stdin: Option<ChildStdin>,
    out: BufReader<ChildStdout>,
    next: u64,
}

impl Stdio1 {
    fn spawn(args: &[&str], env: &[(&str, &str)]) -> Stdio1 {
        let mut cmd = Command::new(BIN);
        cmd.arg("mcp").args(args).env_remove("RUNBOOK_DAEMON").env_remove("RUNBOOK_TOKEN").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        for (k, v) in env {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().unwrap();
        let (stdin, out) = (child.stdin.take(), BufReader::new(child.stdout.take().unwrap()));
        Stdio1 { child, stdin, out, next: 1 }
    }

    fn to(daemon: &Daemon) -> Stdio1 {
        Stdio1::spawn(&["--daemon", &daemon.url()], &[])
    }

    fn write(&mut self, msg: &Value) {
        let stdin = self.stdin.as_mut().unwrap();
        stdin.write_all(format!("{msg}\n").as_bytes()).unwrap();
        stdin.flush().unwrap();
    }

    fn read(&mut self) -> Value {
        let mut line = String::new();
        assert!(self.out.read_line(&mut line).unwrap() > 0, "the server closed its output");
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("not one JSON message ({e}): {line:?}"))
    }

    /// A request in its modern form; the whole answer.
    fn ask(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let id = self.next;
        self.write(&modern_request(id, method, params));
        let r = self.read();
        assert_eq!(r["id"], json!(id), "{r}");
        r
    }

    /// A tool call in its modern form; its `result`.
    fn tool(&mut self, name: &str, arguments: Value) -> Value {
        let r = self.ask("tools/call", json!({"name": name, "arguments": arguments}));
        assert!(r.get("error").is_none(), "{r}");
        r["result"].clone()
    }

    /// The legacy opening: `initialize`, then `notifications/initialized`.
    fn initialize(&mut self) -> Value {
        self.write(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "test", "version": "0"}}}));
        let r = self.read();
        self.write(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        r
    }
}

impl Drop for Stdio1 {
    fn drop(&mut self) {
        self.stdin.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let n = c.iter().enumerate().fold(0u32, |a, (i, b)| a | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..4 {
            if i <= c.len() {
                out.push(A[(n >> (18 - 6 * i)) as usize & 63] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

// ---- acceptance 1: initialize, server info, the six tools ----

#[test]
fn stdio_initialize_answers_server_info_and_the_seven_tools() {
    let d = Daemon::start("mcp-init", None, "127.0.0.1:0");
    let mut c = Stdio1::to(&d);
    let r = c.initialize();
    assert_eq!(r["id"], json!(1));
    assert_eq!(r["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(r["result"]["serverInfo"]["name"], "runbook");
    assert_eq!(r["result"]["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(r["result"]["capabilities"]["tools"].is_object(), "{r}");
    // `notifications/initialized` is not answered: the next message read is the answer to the next request.
    c.write(&json!({"jsonrpc": "2.0", "id": "p", "method": "ping"}));
    let p = c.read();
    assert_eq!((p["id"].clone(), p["result"].clone()), (json!("p"), json!({})));
    c.write(&json!({"jsonrpc": "2.0", "id": 7, "method": "tools/list"}));
    let l = c.read();
    assert_eq!(l["id"], json!(7));
    assert_eq!(names(&l["result"]["tools"]), SEVEN);
    assert!(l["result"].get("resultType").is_none(), "a legacy answer is bare: {l}");
}

#[test]
fn stdio_a_client_of_the_current_revision_discovers_and_lists_exactly_seven_tools() {
    let d = Daemon::start("mcp-modern", None, "127.0.0.1:0");
    let mut c = Stdio1::to(&d);
    let r = c.ask("server/discover", json!({}));
    assert_eq!(r["result"]["resultType"], "complete");
    assert_eq!(r["result"]["supportedVersions"], json!([MODERN, "2025-11-25", "2025-06-18"]));
    assert!(r["result"]["capabilities"]["tools"].is_object());
    assert_eq!(r["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["name"], "runbook");
    let l = c.ask("tools/list", json!({}));
    assert_eq!(l["result"]["resultType"], "complete");
    assert_eq!(names(&l["result"]["tools"]), SEVEN);
    // A request of an unsupported revision is the error that names the revisions; one without the
    // per-request metadata is invalid params, before any handshake.
    c.write(&json!({"jsonrpc": "2.0", "id": 5, "method": "tools/list", "params": {"_meta": {"io.modelcontextprotocol/protocolVersion": "1900-01-01", "io.modelcontextprotocol/clientCapabilities": {}}}}));
    let e = c.read();
    assert_eq!(e["error"]["code"], -32022);
    assert_eq!(e["error"]["data"]["requested"], "1900-01-01");
    assert_eq!(e["error"]["data"]["supported"], json!([MODERN, "2025-11-25", "2025-06-18"]));
    c.write(&json!({"jsonrpc": "2.0", "id": 6, "method": "tools/list"}));
    assert_eq!(c.read()["error"]["code"], -32602);
    c.write(&json!({"jsonrpc": "2.0", "id": 8, "method": "tools/nothing", "params": {"_meta": modern_request(0, "x", json!({}))["params"]["_meta"]}}));
    assert_eq!(c.read()["error"]["code"], -32601);
}

#[test]
fn stdio_answers_a_message_that_is_not_json_and_goes_on() {
    let d = Daemon::start("mcp-garbage", None, "127.0.0.1:0");
    let mut c = Stdio1::to(&d);
    c.stdin.as_mut().unwrap().write_all(b"this is not json\n[1,2]\n\n").unwrap();
    let a = c.read();
    assert_eq!((a["error"]["code"].clone(), a["id"].clone()), (json!(-32700), Value::Null));
    assert_eq!(c.read()["error"]["code"], -32600, "a batch is no message of this revision");
    let r = c.ask("ping", json!({}));
    assert_eq!(r["result"]["resultType"], "complete");
}

#[test]
fn stdio_without_a_named_daemon_refuses_to_start() {
    let out = Command::new(BIN).arg("mcp").env_remove("RUNBOOK_DAEMON").env_remove("RUNBOOK_TOKEN").stdin(Stdio::null()).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty(), "nothing but MCP messages on standard output");
    assert!(String::from_utf8_lossy(&out.stderr).contains("no daemon named"), "{}", String::from_utf8_lossy(&out.stderr));
    let out = Command::new(BIN).args(["mcp", "--daemon", "https://example.com"]).env_remove("RUNBOOK_TOKEN").stdin(Stdio::null()).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("only http://"));
}

// ---- acceptance 2: submit, list, cancel; equal to the HTTP routes ----

#[test]
fn stdio_submit_list_and_cancel_give_what_the_http_routes_give() {
    let d = Daemon::start("mcp-sub", None, "127.0.0.1:0");
    let mut c = Stdio1::to(&d);
    c.initialize();
    let sub = c.tool("submit_run", json!({"runbook_id": "hold", "intake": "x"}));
    assert_eq!(sub["isError"], false, "{sub}");
    let body = sub["structuredContent"].clone();
    assert_eq!(body["outcome"], "submitted");
    assert_eq!(serde_json::from_str::<Value>(sub["content"][0]["text"].as_str().unwrap()).unwrap(), body, "the text is the structured body");
    assert_eq!(sub["_meta"]["runbook/http-status"], 202);
    let run = run_of(&body);
    // The same call to the route gives the same body (for the run it makes).
    let http = d.command("submit-run", &json!({"runbook_id": "hold", "intake": "x"}));
    assert_eq!(http.status, 202);
    let other = run_of(&http.json());
    assert_eq!(without_ids(&body, &[&run]), without_ids(&http.json(), &[&other]));
    d.wait_state(&run, "Running");
    d.wait_state(&other, "Running");
    // list_runs lists it, and is the body of the route.
    let list = c.tool("list_runs", json!({}));
    let rows = list["structuredContent"]["rows"].as_array().unwrap();
    assert!(rows.iter().any(|r| r["run_id"] == run.as_str()), "{list}");
    assert_eq!(list["structuredContent"], d.get("/daemon/views/runs").json());
    let filtered = c.tool("list_runs", json!({"state": "Running"}));
    assert_eq!(filtered["structuredContent"], d.get("/daemon/views/runs-in-state?state=Running").json());
    // cancel_run moves it to cancelled; the body is the route's.
    let cancel = c.tool("cancel_run", json!({"run_id": run}));
    assert_eq!((cancel["isError"].clone(), cancel["structuredContent"]["outcome"].clone()), (json!(false), json!("cancelled")));
    let http = d.command("cancel-run", &json!({"run_id": other}));
    assert_eq!((http.status, cancel["_meta"]["runbook/http-status"].clone()), (202, json!(202)));
    assert_eq!(without_ids(&cancel["structuredContent"], &[&run]), without_ids(&http.json(), &[&other]));
    let read = c.tool("read_run", json!({"run_id": run}));
    assert_eq!(read["structuredContent"]["rows"][0]["state"], "Cancelled");
    assert_eq!(read["structuredContent"], d.get(&format!("/daemon/views/run?run_id={run}")).json());
    let again = c.tool("cancel_run", json!({"run_id": run}));
    assert_eq!((again["isError"].clone(), again["_meta"]["runbook/http-status"].clone()), (json!(true), json!(409)));
}

#[test]
fn stdio_answers_a_decision_as_the_http_route_does() {
    let d = Daemon::start("mcp-ans", None, "127.0.0.1:0");
    let mut c = Stdio1::to(&d);
    let run = run_of(&c.tool("submit_run", json!({"runbook_id": "ask", "intake": "x"}))["structuredContent"]);
    d.fx.wait_open_decisions(&run, 1);
    let open = d.get(&format!("/daemon/views/open-decisions?run_id={run}")).json();
    let request = open["rows"][0]["request_id"].as_str().unwrap().to_string();
    let a = c.tool("answer_decision", json!({"run_id": run, "request_id": request, "answer": "yes", "answered_by": "operator"}));
    assert_eq!((a["isError"].clone(), a["structuredContent"]["outcome"].clone()), (json!(false), json!("answered")));
    let again = c.tool("answer_decision", json!({"run_id": run, "request_id": request, "answer": "no", "answered_by": "operator"}));
    let http = d.command("answer-decision", &json!({"run_id": run, "request_id": request, "answer": "no", "answered_by": "operator"}));
    assert_eq!(again["structuredContent"], http.json());
    assert_eq!(again["_meta"]["runbook/http-status"], http.status);
    assert_eq!(http.status, 409);
    d.wait_state(&run, "Ended");
}

#[test]
fn stdio_reads_the_open_decisions_of_a_run_with_the_step_that_asks_as_the_view_has_them() {
    let d = Daemon::start("mcp-open-decisions", None, "127.0.0.1:0");
    let mut c = Stdio1::to(&d);
    let run = run_of(&c.tool("submit_run", json!({"runbook_id": "ask", "intake": "x"}))["structuredContent"]);
    d.fx.wait_open_decisions(&run, 1);
    let read = c.tool("read_open_decisions", json!({"run_id": run}));
    assert_eq!((read["isError"].clone(), read["_meta"]["runbook/http-status"].clone()), (json!(false), json!(200)), "{read}");
    let http = d.get(&format!("/daemon/views/open-decisions?run_id={run}")).json();
    assert_eq!(read["structuredContent"], http, "the body of the route");
    let row = &read["structuredContent"]["rows"][0];
    assert_eq!((row["request_id"].clone(), row["case_id"].clone(), row["action"].clone(), row["capability"].clone()), (json!("q1"), json!("case-1"), json!("intake.approve"), json!("approve")), "{row}");
    super::openapi::check_row("OpenDecisionsOfRun", row);
    let none = c.tool("read_open_decisions", json!({"run_id": "no-such-run"}));
    assert_eq!(none["structuredContent"]["rows"], json!([]), "a run with nothing open has no row");
    let refused = c.ask("tools/call", json!({"name": "read_open_decisions", "arguments": {}}));
    assert!(refused["error"]["code"] == -32602 || refused["result"]["isError"] == true, "run_id is required: {refused}");
    d.fx.answer(&run, "q1");
    d.wait_state(&run, "Ended");
}

#[test]
fn an_answer_through_mcp_reaches_the_runtime_with_the_daemons_principal_and_no_argument_sets_it() {
    let d = Daemon::start("mcp-principal", None, "127.0.0.1:0");
    let mut c = Stdio1::to(&d);
    let run = run_of(&c.tool("submit_run", json!({"runbook_id": "ask", "intake": "x"}))["structuredContent"]);
    d.fx.wait_open_decisions(&run, 1);
    let refused = c.ask("tools/call", json!({"name": "answer_decision", "arguments": {"run_id": run, "request_id": "q1", "answer": "A", "answered_by": "operator", "principal": "person:another-one"}}));
    assert_eq!(refused["error"]["code"], -32602, "a tool takes no principal: {refused}");
    assert!(d.fx.stub.answers_given().is_empty(), "nothing reached the runtime");
    let a = c.tool("answer_decision", json!({"run_id": run, "request_id": "q1", "answer": "A", "answered_by": "person:someone-else"}));
    assert_eq!((a["isError"].clone(), a["structuredContent"]["outcome"].clone()), (json!(false), json!("answered")));
    let payload = &a["structuredContent"]["published"][0]["payload"];
    assert_eq!((payload["answered_by"].clone(), payload["principal"].clone()), (json!("person:someone-else"), json!(super::PRINCIPAL)), "{a}");
    let given = d.fx.stub.answers_given();
    assert_eq!(given.len(), 1);
    assert_eq!(given[0].principal, super::PRINCIPAL, "the runtime was given the daemon's principal and not the label");
    let rows = d.get(&format!("/daemon/views/decisions?run_id={run}")).json();
    assert_eq!((rows["rows"][0]["answered_by"].clone(), rows["rows"][0]["principal"].clone()), (json!("person:someone-else"), json!(super::PRINCIPAL)));
    d.wait_state(&run, "Ended");
}

#[test]
fn a_tool_takes_no_path_the_http_command_does_not_take() {
    let d = Daemon::start("mcp-path", None, "127.0.0.1:0");
    let mut c = Stdio1::to(&d);
    let call = json!({"runbook_id": "quick", "intake": "x", "context_path": "/etc"});
    let r = c.tool("submit_run", call.clone());
    let http = d.command("submit-run", &call);
    assert_eq!((http.status, r["_meta"]["runbook/http-status"].clone()), (422, json!(422)));
    let (mine, theirs) = (r["structuredContent"]["payload"]["run_id"].as_str().unwrap().to_string(), http.json()["payload"]["run_id"].as_str().unwrap().to_string());
    assert_eq!(without_ids(&r["structuredContent"], &[&mine]), without_ids(&http.json(), &[&theirs]), "the body of the route, but for the id of the run it minted");
    assert_eq!(r["structuredContent"]["error"], "sdc.daemon.ContextOutsideRoots");
    // An argument the schema does not have is refused, not passed on.
    let r = c.ask("tools/call", json!({"name": "submit_run", "arguments": {"runbook_id": "quick", "intake": "x", "daemon_id": "d", "run_id": "mine"}}));
    assert_eq!(r["error"]["code"], -32602, "{r}");
    let r = c.ask("tools/call", json!({"name": "no_such_tool", "arguments": {}}));
    assert_eq!(r["error"]["code"], -32602);
    assert!(r["error"]["message"].as_str().unwrap().contains("no_such_tool"));
}

// ---- acceptance 3: events after a sequence number ----

#[test]
fn stdio_read_events_returns_the_events_after_a_sequence_in_order_unchanged() {
    let d = Daemon::start("mcp-events", None, "127.0.0.1:0");
    let mut c = Stdio1::to(&d);
    let run = run_of(&d.command("submit-run", &json!({"runbook_id": "book", "intake": "x"})).json());
    d.wait_state(&run, "Ended");
    let all_http = d.get(&format!("/daemon/views/events?run_id={run}&after_position=0")).json();
    let all = all_http["rows"].as_array().unwrap().clone();
    assert!(all.len() >= 3, "submitted, opened and closed at least: {all:?}");
    let whole = c.tool("read_events", json!({"run_id": run}));
    assert_eq!(whole["structuredContent"], all_http, "the whole record, as the route gives it");
    let after = c.tool("read_events", json!({"run_id": run, "after_sequence": 2}));
    let rows = after["structuredContent"]["rows"].as_array().unwrap().clone();
    let want: Vec<Value> = all.iter().filter(|r| r["sequence"].as_u64().unwrap() > 2).cloned().collect();
    assert!(!want.is_empty() && want.len() < all.len());
    assert_eq!(rows, want, "events after sequence 2, each as the runtime recorded it");
    let seqs: Vec<u64> = rows.iter().map(|r| r["sequence"].as_u64().unwrap()).collect();
    assert!(seqs.windows(2).all(|w| w[0] < w[1]) && seqs[0] == 3, "{seqs:?}");
    let opened = whole["structuredContent"]["rows"].as_array().unwrap().iter().find(|r| r["kind"] == "run.opened").unwrap();
    assert_eq!(opened["record"], json!({"a": 1}), "the runtime's record is passed on as it is");
    let last = all.last().unwrap()["sequence"].as_u64().unwrap();
    let none = c.tool("read_events", json!({"run_id": run, "after_sequence": last}));
    assert_eq!(none["structuredContent"]["rows"], json!([]));
    let by_position = c.tool("read_events", json!({"run_id": run, "after_position": all[0]["position"]}));
    assert_eq!(by_position["structuredContent"]["rows"].as_array().unwrap().len(), all.len() - 1);
}

// ---- acceptance 4: the endpoint of `runbook serve --mcp` ----

#[test]
fn http_endpoint_answers_the_same_tools_list_as_stdio() {
    let d = Daemon::start("mcp-http", None, "127.0.0.1:0");
    let r = d.modern("tools/list", json!({}));
    assert_eq!(r.status, 200, "{}", r.text());
    assert!(r.header("content-type").unwrap().starts_with("application/json"));
    let http = r.json();
    assert_eq!(http["id"], 1);
    assert_eq!(names(&http["result"]["tools"]), SEVEN);
    let mut c = Stdio1::to(&d);
    let stdio = c.ask("tools/list", json!({}));
    assert_eq!(http["result"]["tools"], stdio["result"]["tools"], "one list on both transports");
}

#[test]
fn http_endpoint_without_the_bearer_token_gets_401_on_a_non_loopback_listener() {
    let d = Daemon::start("mcp-401", Some("s3cret-token"), "0.0.0.0:0");
    let list = modern_request(1, "tools/list", json!({}));
    let modern = [("MCP-Protocol-Version", MODERN), ("Mcp-Method", "tools/list")];
    let r = d.mcp(&modern, &list);
    assert_eq!(r.status, 401, "{}", r.text());
    assert_eq!(r.header("www-authenticate"), Some("Bearer"));
    assert!(!r.text().contains("s3cret-token"));
    let mut wrong = modern.to_vec();
    wrong.push(("Authorization", "Bearer not-the-token"));
    assert_eq!(d.mcp(&wrong, &list).status, 401);
    let mut right = modern.to_vec();
    right.push(("Authorization", "Bearer s3cret-token"));
    let r = d.mcp(&right, &list);
    assert_eq!(r.status, 200, "{}", r.text());
    assert_eq!(names(&r.json()["result"]["tools"]), SEVEN);
    // The same check guards the other routes, so a request to the endpoint is not a way round it.
    assert_eq!(d.get("/daemon/views/runs").status, 401);
    // A GET to the endpoint without the token is 401 too, not 405: the check comes first.
    assert_eq!(d.get("/mcp").status, 401);
}

#[test]
fn serve_with_the_flag_serves_the_endpoint_and_without_it_there_is_none() {
    for (flag, want) in [(true, 200), (false, 404)] {
        let dir = super::state::scratch(if flag { "mcp-serve-on" } else { "mcp-serve-off" });
        let mut args = vec!["serve", "--principal", super::PRINCIPAL, "--listen", "127.0.0.1:0", "--state-dir", dir.to_str().unwrap()];
        if flag {
            args.push("--mcp");
        }
        let mut cmd = Command::new(BIN);
        cmd.args(&args).env_remove("RUNBOOK_TOKEN").stdout(Stdio::piped()).stderr(Stdio::null());
        crate::isolate(&mut cmd);
        let mut child = cmd.spawn().unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
        let addr: SocketAddr = line.trim().strip_prefix("runbook: listening on http://").unwrap_or_else(|| panic!("{line:?}")).parse().unwrap();
        let body = modern_request(1, "tools/list", json!({}));
        let r = send(addr, "POST", "/mcp", &[("MCP-Protocol-Version", MODERN), ("Mcp-Method", "tools/list"), ("Content-Type", "application/json")], Some(body.to_string().as_bytes()));
        let _ = child.kill();
        let _ = child.wait();
        assert_eq!(r.status, want, "serve flag {flag}: {}", r.text());
        if flag {
            assert_eq!(names(&r.json()["result"]["tools"]), SEVEN);
        }
    }
}

#[test]
fn http_checks_the_headers_against_the_body() {
    let d = Daemon::start("mcp-headers", None, "127.0.0.1:0");
    let call = modern_request(3, "tools/call", json!({"name": "list_runs", "arguments": {}}));
    let code = |headers: &[(&str, &str)]| {
        let r = d.mcp(headers, &call);
        (r.status, r.json()["error"]["code"].clone())
    };
    let good = [("MCP-Protocol-Version", MODERN), ("Mcp-Method", "tools/call"), ("Mcp-Name", "list_runs")];
    assert_eq!(d.mcp(&good, &call).status, 200);
    let encoded = format!("=?base64?{}?=", base64(b"list_runs"));
    assert_eq!(d.mcp(&[("MCP-Protocol-Version", MODERN), ("Mcp-Method", "tools/call"), ("Mcp-Name", &encoded)], &call).status, 200, "the base64 form of a name is decoded");
    assert_eq!(code(&[("MCP-Protocol-Version", "2025-11-25"), ("Mcp-Method", "tools/call"), ("Mcp-Name", "list_runs")]), (400, json!(-32020)));
    assert_eq!(code(&[("Mcp-Method", "tools/call"), ("Mcp-Name", "list_runs")]), (400, json!(-32020)));
    assert_eq!(code(&[("MCP-Protocol-Version", MODERN), ("Mcp-Name", "list_runs")]), (400, json!(-32020)));
    assert_eq!(code(&[("MCP-Protocol-Version", MODERN), ("Mcp-Method", "tools/list"), ("Mcp-Name", "list_runs")]), (400, json!(-32020)));
    assert_eq!(code(&[("MCP-Protocol-Version", MODERN), ("Mcp-Method", "tools/call"), ("Mcp-Name", "cancel_run")]), (400, json!(-32020)));
    assert_eq!(code(&[("MCP-Protocol-Version", MODERN), ("Mcp-Method", "tools/call")]), (400, json!(-32020)));
}

#[test]
fn http_answers_the_errors_of_the_protocol_with_their_statuses() {
    let d = Daemon::start("mcp-errors", None, "127.0.0.1:0");
    let bad = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {"_meta": {"io.modelcontextprotocol/protocolVersion": "1900-01-01", "io.modelcontextprotocol/clientCapabilities": {}}}});
    let r = d.mcp(&[("MCP-Protocol-Version", "1900-01-01"), ("Mcp-Method", "tools/list")], &bad);
    assert_eq!((r.status, r.json()["error"]["code"].clone()), (400, json!(-32022)));
    assert_eq!(r.json()["error"]["data"]["supported"], json!([MODERN, "2025-11-25", "2025-06-18"]));
    let r = d.mcp(&[("MCP-Protocol-Version", MODERN), ("Mcp-Method", "tools/nothing")], &modern_request(1, "tools/nothing", json!({})));
    assert_eq!((r.status, r.json()["error"]["code"].clone()), (404, json!(-32601)));
    let mut no_caps = modern_request(1, "tools/list", json!({}));
    no_caps["params"]["_meta"].as_object_mut().unwrap().remove("io.modelcontextprotocol/clientCapabilities");
    let r = d.mcp(&[("MCP-Protocol-Version", MODERN), ("Mcp-Method", "tools/list")], &no_caps);
    assert_eq!((r.status, r.json()["error"]["code"].clone()), (400, json!(-32602)));
    let r = d.mcp(&[], &json!({"jsonrpc": "2.0", "id": 1, "result": {}}));
    assert_eq!(r.status, 400, "a client's response is no message");
    let r = d.mcp(&[], &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    assert_eq!((r.status, r.body.len()), (202, 0), "a notification is accepted and has no answer");
    let r = send(d.addr, "POST", "/mcp", &[("Content-Type", "application/json")], Some(b"{not json"));
    assert_eq!((r.status, r.json()["error"]["code"].clone()), (400, json!(-32700)));
    for method in ["GET", "DELETE", "PUT"] {
        let r = d.mcp_method(method);
        assert_eq!(r.status, 405, "{method}");
        assert_eq!(r.header("allow"), Some("POST"));
    }
    let r = d.mcp(&[("Mcp-Session-Id", "abc"), ("Last-Event-ID", "9"), ("MCP-Protocol-Version", MODERN), ("Mcp-Method", "ping")], &modern_request(1, "ping", json!({})));
    assert_eq!(r.status, 200);
    assert!(r.header("mcp-session-id").is_none(), "no session id is minted or echoed");
}

impl Daemon {
    fn mcp_method(&self, method: &str) -> Resp {
        send(self.addr, method, "/mcp", &[], None)
    }
}

#[test]
fn http_refuses_an_origin_that_is_not_the_host_of_the_request() {
    let d = Daemon::start("mcp-origin", None, "127.0.0.1:0");
    let call = modern_request(1, "ping", json!({}));
    let ping = [("MCP-Protocol-Version", MODERN), ("Mcp-Method", "ping")];
    let with = |origin: &str| {
        let mut h = ping.to_vec();
        h.push(("Origin", origin));
        d.mcp(&h, &call).status
    };
    assert_eq!(with("https://evil.example"), 403);
    assert_eq!(with("null"), 403);
    assert_eq!(with("http://localhost:1"), 403);
    // The Host header the test client sends is the listen address: a page of that origin is the same host.
    assert_eq!(with(&format!("http://{}", d.addr)), 200);
    assert_eq!(d.mcp(&ping, &call).status, 200, "no Origin: not a browser");
}

#[test]
fn http_serves_a_legacy_client_without_a_session() {
    let d = Daemon::start("mcp-legacy", None, "127.0.0.1:0");
    let init = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}});
    let r = d.mcp(&[], &init);
    assert_eq!(r.status, 200, "{}", r.text());
    assert_eq!(r.json()["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(r.json()["result"]["serverInfo"]["name"], "runbook");
    assert!(r.header("mcp-session-id").is_none());
    let list = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"});
    let r = d.mcp(&[("MCP-Protocol-Version", "2025-06-18")], &list);
    assert_eq!(r.status, 200, "{}", r.text());
    assert_eq!(names(&r.json()["result"]["tools"]), SEVEN);
    assert!(r.json()["result"].get("resultType").is_none());
    let r = d.mcp(&[], &list);
    assert_eq!((r.status, r.json()["error"]["code"].clone()), (400, json!(-32602)), "neither era is named");
    let call = json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "list_runs", "arguments": {}}});
    let r = d.mcp(&[("MCP-Protocol-Version", "2025-11-25")], &call);
    assert_eq!(r.json()["result"]["isError"], false, "{}", r.text());
}

#[test]
fn http_tool_calls_give_what_the_http_routes_give() {
    let d = Daemon::start("mcp-http-calls", None, "127.0.0.1:0");
    let r = d.modern("tools/call", json!({"name": "submit_run", "arguments": {"runbook_id": "hold", "intake": "x"}}));
    assert_eq!(r.status, 200, "{}", r.text());
    let result = r.json()["result"].clone();
    assert_eq!(result["resultType"], "complete");
    assert_eq!(result["isError"], false);
    let body = result["structuredContent"].clone();
    let http = d.command("submit-run", &json!({"runbook_id": "hold", "intake": "x"}));
    assert_eq!(without_ids(&body, &[&run_of(&body)]), without_ids(&http.json(), &[&run_of(&http.json())]));
    let run = run_of(&body);
    d.wait_state(&run, "Running");
    d.wait_state(&run_of(&http.json()), "Running");
    let list = d.modern("tools/call", json!({"name": "list_runs", "arguments": {}})).json();
    assert_eq!(list["result"]["structuredContent"], d.get("/daemon/views/runs").json());
    let cancel = d.modern("tools/call", json!({"name": "cancel_run", "arguments": {"run_id": run}})).json();
    assert_eq!(cancel["result"]["structuredContent"]["outcome"], "cancelled");
    let unknown = d.modern("tools/call", json!({"name": "no_such_tool", "arguments": {}}));
    assert_eq!((unknown.status, unknown.json()["error"]["code"].clone()), (400, json!(-32602)));
}

// ---- acceptance 5: an unknown run id ----

#[test]
fn an_unknown_run_id_is_an_error_with_the_status_of_the_http_404() {
    let d = Daemon::start("mcp-404", None, "127.0.0.1:0");
    let http = d.command("cancel-run", &json!({"run_id": "no-such-run"}));
    assert_eq!(http.status, 404);
    let mut c = Stdio1::to(&d);
    for (tool, args) in [
        ("cancel_run", json!({"run_id": "no-such-run"})),
        ("read_run", json!({"run_id": "no-such-run"})),
        ("read_events", json!({"run_id": "no-such-run"})),
        ("read_events", json!({"run_id": "no-such-run", "after_sequence": 5})),
    ] {
        let r = c.tool(tool, args);
        assert_eq!(r["isError"], true, "{tool}: {r}");
        assert_eq!(r["_meta"]["runbook/http-status"], http.status, "{tool}: the code of the HTTP 404");
        assert!(r["content"][0]["text"].as_str().unwrap().contains("no-such-run") || r["structuredContent"]["payload"]["run_id"] == "no-such-run", "{tool}: {r}");
    }
    let r = c.tool("cancel_run", json!({"run_id": "no-such-run"}));
    assert_eq!(r["structuredContent"], http.json(), "the body of the route");
    // And over the HTTP endpoint.
    let r = d.modern("tools/call", json!({"name": "read_run", "arguments": {"run_id": "no-such-run"}})).json();
    assert_eq!((r["result"]["isError"].clone(), r["result"]["_meta"]["runbook/http-status"].clone()), (json!(true), json!(404)));
    // A missing argument is the route's own 400.
    let r = c.tool("read_run", json!({}));
    assert_eq!((r["isError"].clone(), r["_meta"]["runbook/http-status"].clone()), (json!(true), json!(400)));
}

// ---- the token, and a daemon that is gone ----

#[test]
fn stdio_sends_the_token_it_reads_from_the_environment_and_never_has_it_in_argv() {
    let d = Daemon::start("mcp-token", Some("s3cret-token"), "127.0.0.1:0");
    let url = d.url();
    let mut with = Stdio1::spawn(&["--daemon", &url], &[("RUNBOOK_TOKEN", "s3cret-token")]);
    let cmdline = std::fs::read(format!("/proc/{}/cmdline", with.child.id())).unwrap();
    assert!(!String::from_utf8_lossy(&cmdline).contains("s3cret-token"));
    let r = with.tool("list_runs", json!({}));
    assert_eq!((r["isError"].clone(), r["_meta"]["runbook/http-status"].clone()), (json!(false), json!(200)));
    let mut without = Stdio1::spawn(&["--daemon", &url], &[]);
    let r = without.tool("list_runs", json!({}));
    assert_eq!((r["isError"].clone(), r["_meta"]["runbook/http-status"].clone()), (json!(true), json!(401)));
    assert!(!r.to_string().contains("s3cret-token"));
    let dir = super::state::scratch("mcp-token-file");
    let file = dir.join("token");
    std::fs::write(&file, "s3cret-token\n").unwrap();
    std::fs::set_permissions(&file, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();
    let mut by_file = Stdio1::spawn(&["--daemon", &url, "--token-file", file.to_str().unwrap()], &[]);
    assert_eq!(by_file.tool("list_runs", json!({}))["_meta"]["runbook/http-status"], 200);
}

#[test]
fn stdio_with_the_daemon_gone_is_a_tool_error_that_does_not_carry_the_token() {
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let url = format!("http://127.0.0.1:{port}");
    let mut c = Stdio1::spawn(&["--daemon", &url], &[("RUNBOOK_TOKEN", "s3cret-token")]);
    c.initialize();
    let r = c.tool("list_runs", json!({}));
    assert_eq!((r["isError"].clone(), r["_meta"]["runbook/http-status"].clone()), (json!(true), json!(502)));
    assert!(!r.to_string().contains("s3cret-token"));
    assert!(r.to_string().contains("daemon unreachable"));
}

// ---- the schemas ----

/// Every `$ref` and `x-ess-` key is gone: a schema stands alone.
fn standalone(v: &Value) -> bool {
    match v {
        Value::Object(m) => m.iter().all(|(k, x)| k != "$ref" && !k.starts_with("x-ess-") && standalone(x)),
        Value::Array(a) => a.iter().all(standalone),
        _ => true,
    }
}

#[test]
fn input_schemas_are_derived_from_the_openapi_document() {
    let doc = super::openapi::doc();
    let tools = mcp::tools::list();
    assert_eq!(names(&Value::Array(tools.clone())), SEVEN);
    let by = |n: &str| tools.iter().find(|t| t["name"] == n).unwrap().clone();
    let input = |command: &str| doc["components"]["schemas"][format!("sdc.daemon.{}.Input", command)].clone();
    let keys = |v: &Value| -> Vec<String> { v.as_object().unwrap().keys().cloned().collect() };
    for t in tools {
        assert!(standalone(&t["inputSchema"]), "{}", t["name"]);
        assert_eq!(t["inputSchema"]["type"], "object");
        assert_eq!(t["inputSchema"]["additionalProperties"], false);
    }
    // submit_run: the document's members less the five the handler fills.
    let submit = by("submit_run");
    let mut want = keys(&input("SubmitRun")["properties"]);
    want.retain(|k| !mcp::tools::SUBMIT_FILLED.contains(&k.as_str()));
    assert_eq!(keys(&submit["inputSchema"]["properties"]), want);
    for k in ["run_id", "daemon_id", "runbook_known", "context_inside_roots", "at"] {
        assert!(submit["inputSchema"]["properties"].get(k).is_none(), "{k} is the handler's");
    }
    assert_eq!(submit["inputSchema"]["required"], json!(["runbook_id", "intake"]));
    assert_eq!(submit["inputSchema"]["properties"]["confinement"]["enum"], json!(["local", "linux"]), "the enum is the document's");
    assert_eq!(submit["inputSchema"]["properties"]["idempotency_key"]["maxLength"], 200);
    // The other two commands: the document's members less `at`.
    for (tool, command) in [("cancel_run", "CancelRun"), ("answer_decision", "AnswerDecision")] {
        let mut want = keys(&input(command)["properties"]);
        let filled: &[&str] = if tool == "answer_decision" { &mcp::tools::ANSWER_FILLED } else { &["at"] };
        want.retain(|k| !filled.contains(&k.as_str()));
        assert_eq!(keys(&by(tool)["inputSchema"]["properties"]), want, "{tool}");
        let mut req: Vec<Value> = input(command)["required"].as_array().unwrap().clone();
        req.retain(|k| !filled.contains(&k.as_str().unwrap_or_default()));
        assert_eq!(by(tool)["inputSchema"]["required"], Value::Array(req), "{tool}");
    }
    // The views: their query parameters.
    assert_eq!(by("read_run")["inputSchema"]["required"], json!(["run_id"]));
    assert_eq!(by("read_open_decisions")["inputSchema"]["required"], json!(["run_id"]));
    assert_eq!(keys(&by("read_open_decisions")["inputSchema"]["properties"]), ["run_id"]);
    assert_eq!(by("list_runs")["inputSchema"]["required"], json!([]));
    assert_eq!(by("list_runs")["inputSchema"]["properties"]["state"]["enum"], doc["components"]["schemas"]["sdc.daemon.Run.State"]["enum"]);
    assert_eq!(keys(&by("read_events")["inputSchema"]["properties"]), ["run_id", "after_sequence", "after_position"]);
}
