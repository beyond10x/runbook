//! A delegation (`docs/model-routes.md`, section 8): `POST /daemon/commands/delegate` and the MCP tool `delegate` take a
//! request and a route, answer 202 with the run id at once, and the run's result is read by that id from the view
//! `delegation-result` (and the MCP tool `delegation_result`). A delegation that names a protocol runs that runbook with
//! the request as its intake; one that names none is the free run: one agent node (`sdc-node delegation.free` on the
//! committed claude-cli adapter) whose instruction is the request, on the delegation's route, writing `result.md`.
//!
//! The daemon is `runbook serve` with the Loom runtime; it declares the one route `default` (`[routes.default]` on its
//! lone `[gateway]`, the gateway `default`). No model is called and nothing leaves loopback: the gateway is a stand-in on
//! a port of the test's own and `claude` is a stand-in in bash first on the daemon's `PATH`.

use super::default_confinement::{live_env, rows, Live};
use super::gateway::{claude_cli, gateway_table, home, model_table, repo, write_exe, Home};
use super::http::send;
use super::relay::{gateway, gateway_listing, got, Gateway};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_runbook");
const OK: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nx-llm-gateway-call: gw-call-9\r\nContent-Length: 56\r\nConnection: close\r\n\r\n{\"id\":\"m\",\"usage\":{\"input_tokens\":11,\"output_tokens\":5}}";
const DOWN: &[u8] = b"HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json\r\nContent-Length: 58\r\nConnection: close\r\n\r\n{\"error\":{\"code\":\"unavailable\",\"message\":\"not ready yet\"}}";
const ANSWER: &str = "The answer of the free run.";

/// A `claude` in bash for the free run's agent node: it keeps the prompt it was given (`log/prompt`), sends one
/// Messages call to `ANTHROPIC_BASE_URL` with the model it was given, and when that call was answered 200 writes
/// `out/result.md` in its working directory, as an agent that did the work would.
fn claude_answering(log: &Path) -> String {
    format!(
        r#"#!/bin/bash
log='{log}'
prompt=''
model=''
prev=''
for a in "$@"; do [ "$prev" = -p ] && prompt=$a; [ "$prev" = --model ] && model=$a; prev=$a; done
printf '%s' "$prompt" > "$log/prompt"
rest=${{ANTHROPIC_BASE_URL#http://}}
hostport=${{rest%%/*}}
base=''
[ "$rest" = "$hostport" ] || base=/${{rest#*/}}
body="{{\"model\":\"$model\",\"max_tokens\":8,\"messages\":[{{\"role\":\"user\",\"content\":\"hi\"}}]}}"
exec 3<>"/dev/tcp/${{hostport%:*}}/${{hostport##*:}}" || exit 7
printf 'POST %s/v1/messages HTTP/1.0\r\nHost: %s\r\nAuthorization: Bearer %s\r\nContent-Type: application/json\r\nContent-Length: %s\r\n\r\n%s' "$base" "$hostport" "${{ANTHROPIC_AUTH_TOKEN-}}" "${{#body}}" "$body" >&3
answer=$(cat <&3)
exec 3<&-
status=${{answer%%$'\r'*}}
printf '%s\n' "$status" >> "$log/statuses"
case $status in *" 200 "*) ;; *) exit 8 ;; esac
mkdir -p out && printf '%s\n' '{ANSWER}' > out/result.md
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"num_turns":1,"total_cost_usd":0,"duration_ms":1,"usage":{{"input_tokens":0,"output_tokens":0}},"result":"ok","terminal_reason":"completed"}}'
"#,
        log = log.display()
    )
}

/// A runbook of the test's home that a delegation names: one program that calls the relay through the run's route
/// file (role `default`) and writes `result.md` with the text of the intake.
fn answer_runbook(h: &Home) {
    let protocol = "format: protocol/1\nprotocol:\n  id: answer-demo\n  revision: 1\n  description: >-\n    A delegation test's runbook: one program answers the idea in result.md.\nartifacts:\n  idea: {}\n  result: {}\nevidence_kinds:\n  result_written: {}\nclaims:\n  result.written:\n    true_when:\n      evidence: {kind: result_written, subject: result, result: written}\nactions:\n  answer.write:\n    effect: write\n    may_produce:\n      - evidence: result_written\noutcomes:\n  answered:\n    requires: {claim: result.written}\n";
    let bindings = "format: loom.bindings/1\nprotocol: answer-demo@1\nprogram_root: bindings\nartifacts:\n  idea: idea.yaml\n  result: result.md\nactions:\n  - action: answer.write\n    executor:\n      kind: program\n      argv: [tools/answer.sh, --max-budget-usd, \"0.5\"]\n    evidence:\n      - kind: result_written\n        subject: result\n        producer: answer\n        from: exit\n        results: {\"0\": written, timeout: failed, \"*\": failed}\n";
    std::fs::write(h.home.join("protocol/answer-demo.protocol.yaml"), protocol).unwrap();
    std::fs::write(h.home.join("bindings/answer-demo.yaml"), bindings).unwrap();
    let program = format!(
        r#"#!/bin/bash
log='{log}'
f=$RUNBOOK_ROUTE_FILE
base=$(jq -r .base_url "$f") && token=$(jq -r .token "$f") || exit 9
rest=${{base#http://}}
hostport=${{rest%%/*}}
path=/${{rest#*/}}
body='{{"model":"default","max_tokens":8,"messages":[{{"role":"user","content":"hi"}}]}}'
exec 3<>"/dev/tcp/${{hostport%:*}}/${{hostport##*:}}" || exit 7
printf 'POST %s/v1/messages HTTP/1.0\r\nHost: %s\r\nAuthorization: Bearer %s\r\nContent-Type: application/json\r\nContent-Length: %s\r\n\r\n%s' "$path" "$hostport" "$token" "${{#body}}" "$body" >&3
answer=$(cat <&3)
exec 3<&-
printf '%s\n' "${{answer%%$'\r'*}}" >> "$log/named-statuses"
printf 'answered: %s\n' "$(sed -n 's/^text: //p' idea.yaml)" > result.md
"#,
        log = h.log.display()
    );
    write_exe(&h.home.join("bindings/tools/answer.sh"), &program);
}

struct Delegating {
    h: Home,
    gw: Gateway,
    l: Live,
    roots: std::path::PathBuf,
}

/// The table that declares the route `default` on the lone `[gateway]`.
const ROUTE: &str = "[routes.default]\ngateway = \"default\"\nmodel = \"a-model\"\nkind = \"hosted\"\n";

/// `runbook serve` with the Loom runtime, the route `default` on a stand-in gateway answering `answer`, the stand-in
/// `claude` of [`claude_answering`] and the built `sdc-node` first on its `PATH`, the level `local` as its default and a
/// context root of its own.
fn daemon(tag: &str, answer: &'static [u8]) -> Delegating {
    daemon_with(tag, answer, &format!("{}{ROUTE}", model_table(Some(&claude_cli()))))
}

/// [`daemon`] with `tables` after the lone `[gateway]` of its `runbook.toml`.
fn daemon_with(tag: &str, answer: &'static [u8], tables: &str) -> Delegating {
    let h = home(tag);
    answer_runbook(&h);
    let bin = h.log.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    write_exe(&bin.join("claude"), &claude_answering(&h.log));
    let roots = h.log.join("roots");
    std::fs::create_dir_all(roots.join("project")).unwrap();
    std::fs::write(roots.join("project/notes.txt"), "a file of the workspace\n").unwrap();
    // a gateway that answers DOWN is not ready; any other is ready and lists the aliases of the routes these cases declare
    let gw = if answer == DOWN { gateway(answer) } else { gateway_listing(answer, &["a-model", "b-model"]) };
    let file = format!("{}{tables}", gateway_table(&gw.base_url, &h.token_file));
    let path = format!(
        "{}:{}:/usr/local/bin:/usr/bin:/bin",
        bin.display(),
        repo().join("driver/target/debug").display()
    );
    let args = [
        "--runtime",
        "loom",
        "--runbook-home",
        h.home.to_str().unwrap(),
        "--min-confinement",
        "local",
        "--default-confinement",
        "local",
        "--context-root",
        roots.to_str().unwrap(),
        "--max-wall-seconds",
        "3600",
        "--max-llm-calls",
        "100",
        "--cap-micro-usd",
        "1000000000",
    ];
    let l = live_env(
        &format!("{tag}-d"),
        &args,
        Some(&file),
        &[
            ("XDG_RUNTIME_DIR", Some(h.xdg.to_str().unwrap())),
            ("PATH", Some(&path)),
        ],
    );
    Delegating { h, gw, l, roots }
}

fn delegate(l: &Live, body: &Value) -> (u16, Value, Option<String>) {
    let r = send(
        l.addr,
        "POST",
        "/daemon/commands/delegate",
        &[("Content-Type", "application/json")],
        Some(body.to_string().as_bytes()),
    );
    let location = r.header("location").map(str::to_owned);
    (r.status, r.json(), location)
}

/// The run an accepted delegation names: the `run_id` of the event it published first (`RunSubmitted`, or
/// `SubmitReplayed` for a replay).
fn run_of(body: &Value) -> String {
    body["published"][0]["payload"]["run_id"]
        .as_str()
        .unwrap_or_else(|| panic!("the answer names no run: {body}"))
        .to_owned()
}

/// The member `key` of the event `RunDelegated` an accepted delegation published.
fn delegated(body: &Value, key: &str) -> Value {
    assert_eq!(
        body["published"][1]["event"], "sdc.daemon.RunDelegated",
        "{body}"
    );
    body["published"][1]["payload"][key].clone()
}

fn result(l: &Live, run: &str) -> Value {
    let rows = rows(l, &format!("/daemon/views/delegation-result?run_id={run}"));
    assert_eq!(rows.len(), 1, "{rows:?}");
    rows[0].clone()
}

fn until_ended(l: &Live, run: &str) -> Value {
    let limit = Instant::now() + Duration::from_secs(40);
    loop {
        let r = result(l, run);
        if ["Ended", "FailedToStart", "Cancelled"]
            .iter()
            .any(|s| r["run_state"] == *s)
        {
            return r;
        }
        assert!(
            Instant::now() < limit,
            "gave up waiting for the end of the delegation: {r}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn posted(gw: &Gateway) -> Vec<super::relay::Got> {
    got(gw)
        .into_iter()
        .filter(|g| g.head.starts_with("POST "))
        .collect()
}

#[test]
fn a_free_run_writes_result_md_through_the_relay_and_the_result_view_returns_it() {
    let d = daemon("dlg-free", OK);
    let request = "Write one sentence about tea.";
    let (status, body, location) =
        delegate(&d.l, &json!({"request": request, "model_route": "default"}));
    assert_eq!(status, 202, "{body}");
    super::openapi::check_response("delegate", "delegated", &body);
    assert_eq!(
        (
            delegated(&body, "path"),
            delegated(&body, "route"),
            delegated(&body, "protocol")
        ),
        (json!("free"), json!("default"), json!("delegation-free")),
        "{body}"
    );
    let run = run_of(&body);
    assert_eq!(
        location.as_deref(),
        Some(format!("/daemon/views/delegation-result?run_id={run}").as_str())
    );
    let end = until_ended(&d.l, &run);
    assert_eq!(
        (end["run_state"].clone(), end["outcome"].clone()),
        (json!("Ended"), json!("delivered")),
        "{end}"
    );
    assert_eq!(end["result_text"], json!(format!("{ANSWER}\n")), "{end}");
    assert_eq!(
        (
            end["path"].clone(),
            end["protocol"].clone(),
            end["route"].clone(),
            end["confidence"].clone()
        ),
        (
            json!("free"),
            json!("delegation-free"),
            json!("default"),
            Value::Null
        ),
        "{end}"
    );
    assert_eq!(
        (
            end["calls"].clone(),
            end["input_tokens"].clone(),
            end["output_tokens"].clone(),
            end["cost_known"].clone()
        ),
        (json!(1), json!(11), json!(5), json!(false)),
        "{end}"
    );
    assert!(
        end["products"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p == "result.md"),
        "{end}"
    );
    super::openapi::check_row("DelegationResult", &end);
    // the agent node's instruction is the request, and its one call reached the gateway with the route's alias
    let prompt = std::fs::read_to_string(d.h.log.join("prompt")).unwrap();
    assert!(
        prompt.contains(request),
        "the agent node was not given the request: {prompt}"
    );
    let calls = posted(&d.gw);
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert_eq!(calls[0].body["model"], "a-model");
    // a delegation is a run: the run views know it
    let run_row = rows(&d.l, &format!("/daemon/views/run?run_id={run}"));
    assert_eq!(run_row[0]["runbook_id"], "delegation-free");
}

#[test]
fn a_delegation_that_names_no_route_takes_the_daemons_default_route() {
    let d = daemon_with("dlg-default", OK, &format!("{}{ROUTE}[route_policy]\ndefault = \"default\"\n", model_table(Some(&claude_cli()))));
    let (status, body, _) = delegate(&d.l, &json!({"request": "red or blue", "protocol": "answer-demo"}));
    assert_eq!(status, 202, "{body}");
    assert_eq!(delegated(&body, "route"), json!("default"), "{body}");
    let run = rows(&d.l, &format!("/daemon/views/run?run_id={}", run_of(&body)));
    assert_eq!((run[0]["model_route"].clone(), run[0]["model_route_alias"].clone(), run[0]["model_route_source"].clone()), (json!("default"), json!("a-model"), json!("default")), "the run records the route it took as a run does: {run:?}");
    let end = until_ended(&d.l, &run_of(&body));
    assert_eq!((end["outcome"].clone(), end["route"].clone(), end["calls"].clone()), (json!("answered"), json!("default"), json!(1)), "{end}");
    assert_eq!(posted(&d.gw).len(), 1);
}

#[test]
fn a_daemon_without_a_model_decider_runs_a_free_delegation_on_the_route_it_declares() {
    let other = "[routes.other]\ngateway = \"default\"\nmodel = \"b-model\"\nkind = \"hosted\"\n";
    let d = daemon_with("dlg-declared", OK, &format!("{ROUTE}{other}"));
    let (status, body, _) = delegate(&d.l, &json!({"request": "x", "model_route": "qwen"}));
    assert_eq!((status, body["payload"]["declared"].clone()), (422, json!(["default", "other"])), "{body}");
    let (status, body, _) = delegate(&d.l, &json!({"request": "Write one sentence about tea.", "model_route": "other"}));
    assert_eq!(status, 202, "{body}");
    let run = rows(&d.l, &format!("/daemon/views/run?run_id={}", run_of(&body)));
    assert_eq!((run[0]["model_route"].clone(), run[0]["model_route_alias"].clone(), run[0]["model_route_source"].clone()), (json!("other"), json!("b-model"), json!("run")), "the run records the route it named: {run:?}");
    let end = until_ended(&d.l, &run_of(&body));
    assert_eq!((end["outcome"].clone(), end["route"].clone(), end["calls"].clone()), (json!("delivered"), json!("other"), json!(1)), "{end}");
    assert_eq!(end["result_text"], json!(format!("{ANSWER}\n")), "{end}");
    assert_eq!(posted(&d.gw).len(), 1);
}

#[test]
fn a_delegation_naming_a_protocol_runs_it_with_the_request_as_intake_on_the_route() {
    let d = daemon("dlg-named", OK);
    let (status, body, _) = delegate(
        &d.l,
        &json!({"request": "green or black", "model_route": "default", "protocol": "answer-demo", "workspace": d.roots.join("project").to_str().unwrap()}),
    );
    assert_eq!(status, 202, "{body}");
    assert_eq!(delegated(&body, "path"), json!("named"), "{body}");
    let run = run_of(&body);
    let end = until_ended(&d.l, &run);
    assert_eq!(
        (
            end["run_state"].clone(),
            end["outcome"].clone(),
            end["protocol"].clone(),
            end["path"].clone()
        ),
        (
            json!("Ended"),
            json!("answered"),
            json!("answer-demo"),
            json!("named")
        ),
        "{end}"
    );
    assert_eq!(
        end["result_text"],
        json!("answered: green or black\n"),
        "{end}"
    );
    assert!(
        std::fs::read_to_string(d.h.log.join("named-statuses"))
            .unwrap()
            .contains(" 200 "),
        "the program's call through the route was not answered"
    );
    assert_eq!(posted(&d.gw).len(), 1);
    assert_eq!(end["calls"], json!(1), "{end}");
    // the workspace was copied into the run's own
    assert!(
        end["products"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p == "notes.txt"),
        "{end}"
    );
}

#[test]
fn an_undeclared_route_is_refused_route_refused_naming_the_declared_routes_and_creates_no_run() {
    let d = daemon("dlg-unknown-route", OK);
    let (status, body, _) = delegate(&d.l, &json!({"request": "x", "model_route": "qwen"}));
    assert_eq!(
        (status, body["outcome"].clone(), body["error"].clone()),
        (
            422,
            json!("route-refused"),
            json!("sdc.daemon.RouteRefused")
        ),
        "{body}"
    );
    assert_eq!(body["payload"]["declared"], json!(["default"]), "{body}");
    assert!(body["payload"]["reason"].as_str().is_some_and(|r| r.starts_with("model_route ") && !r.contains("qwen")), "the reason names the member and never the value: {body}");
    super::openapi::check_response("delegate", "route-refused", &body);
    let (status, body, _) = delegate(&d.l, &json!({"request": "x"}));
    assert_eq!(
        (
            status,
            body["outcome"].clone(),
            body["payload"]["declared"].clone()
        ),
        (422, json!("route-refused"), json!(["default"])),
        "a delegation that names no route and a daemon with no default: {body}"
    );
    assert!(
        rows(&d.l, "/daemon/views/runs").is_empty(),
        "a refused delegation created a run"
    );
    assert!(
        got(&d.gw).is_empty(),
        "a refused delegation reached the gateway"
    );
}

#[test]
fn a_gateway_that_is_not_ready_refuses_the_delegation_route_unavailable_and_creates_no_run() {
    let d = daemon("dlg-down", DOWN);
    let (status, body, _) = delegate(&d.l, &json!({"request": "x", "model_route": "default"}));
    assert_eq!(
        (status, body["refused"].clone()),
        (503, json!("route-unavailable")),
        "{body}"
    );
    let message = body["message"].as_str().unwrap();
    assert!(
        message.contains("route default")
            && message.contains("not ready")
            && !message.contains(&d.gw.base_url),
        "{message}"
    );
    assert!(
        rows(&d.l, "/daemon/views/runs").is_empty(),
        "a refused delegation created a run"
    );
    assert!(posted(&d.gw).is_empty(), "a model call was sent");
}

#[test]
fn the_refusals_of_submit_run_apply_to_a_delegation() {
    let d = daemon("dlg-refusals", OK);
    let base = json!({"request": "x", "model_route": "default"});
    let with = |k: &str, v: Value| {
        let mut b = base.clone();
        b[k] = v;
        b
    };
    let (status, body, _) = delegate(&d.l, &with("protocol", json!("no-such-runbook")));
    assert_eq!(
        (status, body["outcome"].clone()),
        (422, json!("no-runbook")),
        "{body}"
    );
    let (status, body, _) = delegate(
        &d.l,
        &with(
            "request",
            json!("x".repeat(runbook::commands::MAX_INTAKE_BYTES + 1)),
        ),
    );
    assert_eq!(
        (status, body["outcome"].clone()),
        (422, json!("intake-too-large")),
        "{body}"
    );
    let outside = d.h.log.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    let (status, body, _) = delegate(&d.l, &with("workspace", json!(outside.to_str().unwrap())));
    assert_eq!(
        (status, body["outcome"].clone()),
        (422, json!("context-outside-roots")),
        "{body}"
    );
    let (status, body, _) = delegate(&d.l, &with("session_id", json!("no-such-session")));
    assert_eq!(
        (status, body["outcome"].clone()),
        (409, json!("no-session")),
        "{body}"
    );
    let (status, body, _) = delegate(&d.l, &with("token", json!("t-0123456789")));
    assert_eq!(status, 400, "{body}");
    assert!(!body.to_string().contains("t-0123456789"), "{body}");
    assert!(
        rows(&d.l, "/daemon/views/runs").is_empty(),
        "a refused delegation created a run"
    );
    // a key used twice answers the first run
    let (s1, first, _) = delegate(&d.l, &with("idempotency_key", json!("k1")));
    let (s2, again, _) = delegate(&d.l, &with("idempotency_key", json!("k1")));
    assert_eq!(
        (s1, s2, again["outcome"].clone(), run_of(&again)),
        (202, 202, json!("already-submitted"), run_of(&first)),
        "{again}"
    );
    super::openapi::check_response("delegate", "already-submitted", &again);
    until_ended(&d.l, &run_of(&first));
    // a run the daemon does not know
    let r = send(
        d.l.addr,
        "GET",
        "/daemon/views/delegation-result?run_id=no-such-run",
        &[],
        None,
    );
    assert_eq!(r.status, 404, "{}", r.text());
}

// ---- MCP and the command line ----

struct Mcp {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    out: BufReader<std::process::ChildStdout>,
}

impl Mcp {
    fn to(l: &Live) -> Mcp {
        let mut child = Command::new(BIN)
            .args(["mcp", "--daemon", &format!("http://{}", l.addr)])
            .env_remove("RUNBOOK_DAEMON")
            .env_remove("RUNBOOK_TOKEN")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let (stdin, out) = (
            child.stdin.take().unwrap(),
            BufReader::new(child.stdout.take().unwrap()),
        );
        let mut m = Mcp { child, stdin, out };
        m.send(&json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}}));
        m.until(0);
        m
    }

    fn send(&mut self, v: &Value) {
        writeln!(self.stdin, "{v}").unwrap();
        self.stdin.flush().unwrap();
    }

    /// The messages up to and with the answer to `id`.
    fn until(&mut self, id: u64) -> Vec<Value> {
        let mut seen = Vec::new();
        loop {
            let mut line = String::new();
            assert!(
                self.out.read_line(&mut line).unwrap() > 0,
                "runbook mcp ended before it answered {id}: {seen:?}"
            );
            let v: Value = serde_json::from_str(&line).unwrap();
            let done = v["id"] == json!(id);
            seen.push(v);
            if done {
                return seen;
            }
        }
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn mcp_delegate_waits_for_the_end_sends_progress_per_step_and_returns_the_result() {
    let d = daemon("dlg-mcp", OK);
    let mut m = Mcp::to(&d.l);
    let names: Vec<String> = {
        m.send(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}}));
        let list = m.until(1).pop().unwrap();
        list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_owned())
            .collect()
    };
    assert!(
        names.contains(&"delegate".to_owned()) && names.contains(&"delegation_result".to_owned()),
        "{names:?}"
    );
    m.send(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "delegate", "arguments": {"request": "Write one sentence about tea.", "model_route": "default"}, "_meta": {"progressToken": "p-1"}}}));
    let seen = m.until(2);
    let (answer, notes) = seen.split_last().unwrap();
    let progress: Vec<&Value> = notes
        .iter()
        .filter(|n| n["method"] == "notifications/progress")
        .collect();
    assert!(!progress.is_empty(), "no progress notification: {notes:?}");
    assert!(
        progress
            .iter()
            .all(|p| p["params"]["progressToken"] == "p-1"),
        "{progress:?}"
    );
    assert!(
        progress.iter().any(|p| p["params"]["message"]
            .as_str()
            .is_some_and(|t| t.contains("delegation.free"))),
        "no step was named: {progress:?}"
    );
    let r = &answer["result"];
    assert_eq!(r["isError"], json!(false), "{answer}");
    let row = &r["structuredContent"]["rows"][0];
    assert_eq!(
        (
            row["run_state"].clone(),
            row["result_text"].clone(),
            row["route"].clone()
        ),
        (
            json!("Ended"),
            json!(format!("{ANSWER}\n")),
            json!("default")
        ),
        "{answer}"
    );
    let text = r["content"][0]["text"].as_str().unwrap();
    assert!(
        text.lines()
            .next()
            .unwrap()
            .contains(row["run_id"].as_str().unwrap())
            && text.contains(ANSWER),
        "the text starts with one line naming the run, then the result: {text}"
    );
    // the same result by run id
    let run = row["run_id"].as_str().unwrap().to_owned();
    m.send(&json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "delegation_result", "arguments": {"run_id": run}}}));
    let again = m.until(3).pop().unwrap();
    assert_eq!(
        again["result"]["structuredContent"]["rows"][0]["result_text"],
        json!(format!("{ANSWER}\n")),
        "{again}"
    );
    // a refusal is a tool error with the route's answer
    m.send(&json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "delegate", "arguments": {"request": "x", "model_route": "qwen"}}}));
    let refused = m.until(4).pop().unwrap();
    assert_eq!(
        (
            refused["result"]["isError"].clone(),
            refused["result"]["structuredContent"]["outcome"].clone()
        ),
        (json!(true), json!("route-refused")),
        "{refused}"
    );
}

#[test]
fn mcp_delegate_past_its_wait_returns_the_run_id_and_state() {
    let d = daemon("dlg-mcp-wait", OK);
    // the agent node waits before it answers: the tool's wait of 0 seconds ends first
    write_exe(
        &d.h.log.join("bin/claude"),
        &format!(
            "#!/bin/bash\nsleep 3\nexec '{}' \"$@\"\n",
            d.h.log.join("bin/claude-real").display()
        ),
    );
    write_exe(
        &d.h.log.join("bin/claude-real"),
        &claude_answering(&d.h.log),
    );
    let mut m = Mcp::to(&d.l);
    m.send(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "delegate", "arguments": {"request": "x", "model_route": "default", "wait_seconds": 0}}}));
    let answer = m.until(2).pop().unwrap();
    let row = &answer["result"]["structuredContent"]["rows"][0];
    assert_eq!(answer["result"]["isError"], json!(false), "{answer}");
    assert!(
        row["run_id"].is_string()
            && ["Submitted", "Running"]
                .iter()
                .any(|s| row["run_state"] == *s),
        "{answer}"
    );
    assert!(row["result_text"].is_null(), "{answer}");
}

#[test]
fn runbook_delegate_is_a_client_of_the_endpoint() {
    let d = daemon("dlg-cli", OK);
    let request = d.h.log.join("request.txt");
    std::fs::write(&request, "Write one sentence about tea.\n").unwrap();
    let out = Command::new(BIN)
        .args([
            "delegate",
            "--daemon",
            &format!("http://{}", d.l.addr),
            "--route",
            "default",
        ])
        .arg(&request)
        .env("RUNBOOK_NO_CONFIG", "1")
        .env_remove("RUNBOOK_DAEMON")
        .env_remove("RUNBOOK_TOKEN_FILE")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let run = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    assert_eq!(
        until_ended(&d.l, &run)["result_text"],
        json!(format!("{ANSWER}\n"))
    );
    let refused = Command::new(BIN)
        .args([
            "delegate",
            "--daemon",
            &format!("http://{}", d.l.addr),
            "--route",
            "qwen",
            "--protocol",
            "answer-demo",
        ])
        .arg(&request)
        .env("RUNBOOK_NO_CONFIG", "1")
        .output()
        .unwrap();
    assert!(!refused.status.success());
    let err = String::from_utf8_lossy(&refused.stderr);
    assert!(
        err.contains("route-refused") && err.contains("default"),
        "{err}"
    );
}

// ---- adversary cases ----

/// The free run's instruction is the request as the session wrote it: a request that happens to read as a YAML mapping
/// with a `text` member is still the whole request, not that one member.
#[test]
fn adversary_the_free_runs_agent_node_gets_the_whole_request_when_it_reads_as_yaml_with_text() {
    let d = daemon("dlg-adv-yaml", OK);
    let request = "text: Write one sentence about tea.\nlanguage: answer in French and never in English\n";
    let (status, body, _) = delegate(&d.l, &json!({"request": request, "model_route": "default"}));
    assert_eq!(status, 202, "{body}");
    let end = until_ended(&d.l, &run_of(&body));
    let prompt = std::fs::read_to_string(d.h.log.join("prompt")).unwrap_or_default();
    assert!(
        prompt.contains("language: answer in French and never in English"),
        "the agent node was given only a part of the request (end {end}): {prompt}"
    );
}

/// `result_text` is "absent while the run has written none" (ess/domains/daemon.yaml, Delegation): a `result.md` that came
/// with the workspace and that the run never wrote is not the run's result.
#[test]
fn adversary_a_free_run_that_writes_no_result_answers_no_result_text_from_the_workspace_copy() {
    let d = daemon("dlg-adv-stale", OK);
    // the agent node answers success and writes nothing
    write_exe(
        &d.h.log.join("bin/claude"),
        "#!/bin/bash\nprintf '%s\\n' '{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"num_turns\":1,\"total_cost_usd\":0,\"duration_ms\":1,\"usage\":{\"input_tokens\":0,\"output_tokens\":0},\"result\":\"ok\",\"terminal_reason\":\"completed\"}'\n",
    );
    let workspace = d.roots.join("stale");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::write(workspace.join("result.md"), "an old answer the run never wrote\n").unwrap();
    let (status, body, _) = delegate(
        &d.l,
        &json!({"request": "Write one sentence about tea.", "model_route": "default", "workspace": workspace.to_str().unwrap()}),
    );
    assert_eq!(status, 202, "{body}");
    let end = until_ended(&d.l, &run_of(&body));
    assert!(
        end["result_text"].is_null(),
        "a run that wrote no result.md answers the workspace's copy as its result: {end}"
    );
}

/// A session delegates side by side (parallel sub-agents): a second `delegate` on the same `runbook mcp` is taken while
/// the first one waits for its run, so the second run is submitted before the first run ends.
#[test]
fn adversary_mcp_delegate_does_not_hold_the_server_while_it_waits() {
    let d = daemon("dlg-adv-mcp-par", OK);
    write_exe(
        &d.h.log.join("bin/claude"),
        &format!(
            "#!/bin/bash\nsleep 3\nexec '{}' \"$@\"\n",
            d.h.log.join("bin/claude-real").display()
        ),
    );
    write_exe(&d.h.log.join("bin/claude-real"), &claude_answering(&d.h.log));
    let mut m = Mcp::to(&d.l);
    for id in [2, 3] {
        m.send(&json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": "delegate", "arguments": {"request": format!("task {id}"), "model_route": "default", "wait_seconds": 30}}}));
    }
    let mut answers = std::collections::BTreeMap::new();
    while answers.len() < 2 {
        let mut line = String::new();
        assert!(m.out.read_line(&mut line).unwrap() > 0, "runbook mcp ended: {answers:?}");
        let v: Value = serde_json::from_str(&line).unwrap();
        if let Some(id) = v["id"].as_u64() {
            answers.insert(id, v);
        }
    }
    let run = |id: u64| {
        let r = answers[&id]["result"]["structuredContent"]["rows"][0]["run_id"]
            .as_str()
            .unwrap_or_else(|| panic!("{}", answers[&id]))
            .to_owned();
        rows(&d.l, &format!("/daemon/views/run?run_id={r}"))[0].clone()
    };
    let (first, second) = (run(2), run(3));
    let at = |row: &Value, key: &str| runbook::clock::parse_instant(row[key].as_str().unwrap_or_else(|| panic!("{key}: {row}"))).unwrap();
    assert!(
        at(&second, "submitted_at") < at(&first, "ended_at"),
        "the second delegate was taken only after the first run ended: first {first}, second {second}"
    );
}

/// The answer of an accepted delegation names its run in the `Location` of the view `delegation-result`; a replay of an
/// idempotency key a submit-run used names a run that view refuses.
#[test]
fn adversary_a_delegate_replaying_a_submit_runs_key_names_a_result_its_location_answers() {
    let d = daemon("dlg-adv-key", OK);
    let (status, body) = super::default_confinement::post(
        &d.l,
        &json!({"runbook_id": "answer-demo", "intake": "x", "idempotency_key": "k-shared"}),
    );
    assert_eq!(status, 202, "{body}");
    let (status, body, location) = delegate(
        &d.l,
        &json!({"request": "x", "model_route": "default", "protocol": "answer-demo", "idempotency_key": "k-shared"}),
    );
    assert_eq!(status, 202, "{body}");
    let location = location.unwrap_or_else(|| panic!("no Location: {body}"));
    let r = send(d.l.addr, "GET", &location, &[], None);
    assert_eq!(
        r.status, 200,
        "delegate answered 202 {} and its Location {location} answers: {}",
        body["outcome"], r.text()
    );
}
