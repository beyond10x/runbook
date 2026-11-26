//! Adversary B on the wave "model routes": the delegation (`docs/model-routes.md`, sections 8 and 9, and Part two).
//!
//! Each case states a line of the design or a story and fails while the code does not hold it. No model is called and
//! nothing leaves loopback: every gateway is a stand-in on a port of the test's own, and the runbook programs are the
//! stand-ins of `delegate.rs`.

use super::delegate::{daemon_at, until_ended, ROUTE};
use super::http::send;
use super::relay::{gateway_answering, got, Gateway, Got};
use runbook::clock::SetClock;
use runbook::commands::{Daemon, Delegate, Policy};
use runbook::routes::{RouteTable, Tables};
use runbook::runtime::Confinement;
use runbook::stub::{Script, Step, StubRuntime};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_runbook");
const OK: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 56\r\nConnection: close\r\n\r\n{\"id\":\"m\",\"usage\":{\"input_tokens\":11,\"output_tokens\":5}}";

/// The route `classify` (alias `c-model`) beside the route `default`: the classifier route of `[route_policy]`.
const CLASSIFY: &str = "[routes.classify]\ngateway = \"default\"\nmodel = \"c-model\"\nkind = \"hosted\"\n[route_policy]\nclassifier = \"classify\"\n";

/// A Messages answer of the gateway whose text is `text`.
fn messages(text: &str) -> Vec<u8> {
    let body = json!({"id": "m", "type": "message", "role": "assistant", "content": [{"type": "text", "text": text}], "usage": {"input_tokens": 7, "output_tokens": 3}}).to_string();
    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).into_bytes()
}

/// A builder's answer the loader refuses: nothing of it runs, and no agent node is needed.
const REFUSED_BUILD: &str = r#"{"protocol": "not a protocol", "bindings": "not bindings"}"#;

/// A gateway whose model `c-model` answers the texts of `scripted` in order (the last again once the others are spent),
/// and every other request [`OK`].
fn classifying(scripted: Vec<String>) -> Gateway {
    let left = std::sync::Mutex::new(std::collections::VecDeque::from(scripted));
    gateway_answering(
        move |g| {
            if g.body["model"] != "c-model" {
                return OK.to_vec();
            }
            let mut left = left.lock().unwrap();
            let text = if left.len() > 1 {
                left.pop_front().unwrap_or_default()
            } else {
                left.front().cloned().unwrap_or_default()
            };
            messages(&text)
        },
        &["a-model", "c-model"],
    )
}

fn classifier_calls(gw: &Gateway) -> Vec<Got> {
    got(gw)
        .into_iter()
        .filter(|g| g.head.starts_with("POST ") && g.body["model"] == "c-model")
        .collect()
}

fn delegate_http(addr: std::net::SocketAddr, body: &Value) -> (u16, Value) {
    let r = send(
        addr,
        "POST",
        "/daemon/commands/delegate",
        &[("Content-Type", "application/json")],
        Some(body.to_string().as_bytes()),
    );
    (r.status, r.json())
}

fn run_of(body: &Value) -> String {
    body["published"][0]["payload"]["run_id"]
        .as_str()
        .unwrap_or_else(|| panic!("the answer names no run: {body}"))
        .to_owned()
}

fn wait_for(what: &str, limit: Duration, mut done: impl FnMut() -> bool) {
    let until = Instant::now() + limit;
    while !done() {
        assert!(Instant::now() < until, "gave up waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

// ---- in process, on a test clock: a delegation whose managed route cannot answer ----

/// A stand-in gateway that is ready, lists `qwen` and `claude`, and says the phase `phase` of `qwen`. It is sent only reads.
fn phase_gateway(phase: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            let mut head = Vec::new();
            let mut byte = [0u8; 1];
            while !head.ends_with(b"\r\n\r\n") && head.len() < 64 * 1024 {
                match stream.read(&mut byte) {
                    Ok(1) => head.push(byte[0]),
                    _ => break,
                }
            }
            let line: String = String::from_utf8_lossy(&head)
                .lines()
                .next()
                .unwrap_or("")
                .rsplit_once(' ')
                .map(|(l, _)| l.to_owned())
                .unwrap_or_default();
            let (code, body) = match line.as_str() {
                "GET /ready" => (200, json!({"ready": true})),
                "GET /v1/models" => (
                    200,
                    json!({"object": "list", "data": [{"id": "qwen", "wires": ["messages"]}, {"id": "claude", "wires": ["messages"]}]}),
                ),
                "GET /v1/models/qwen/status" => (200, json!({"model": "qwen", "phase": phase})),
                _ => (404, json!({"error": {"code": "not-found"}})),
            };
            let body = body.to_string();
            let _ = stream.write_all(format!("HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
        }
    });
    base_url
}

/// A daemon on a test clock with the stub runtime (the runbook `quick` ends `done`) and the managed route `qwen` on the
/// gateway at `base_url`, which waits `wait` seconds for its model.
fn waiting_daemon(tag: &str, base_url: &str, wait: u64) -> (Daemon, Arc<SetClock>) {
    let dir = super::state::scratch(tag);
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    std::fs::create_dir_all(dir.join("runbooks")).unwrap();
    let gateways = json!({"local": {"base_url": base_url}});
    let routes = json!({
        "qwen": {"gateway": "local", "model": "qwen", "kind": "managed", "start_wait_seconds": wait},
        "claude": {"gateway": "local", "model": "claude", "kind": "hosted"},
    });
    let mut table = RouteTable::declare(&Tables {
        gateways: Some(&gateways),
        routes: Some(&routes),
        ..Tables::default()
    })
    .unwrap();
    table.read_tokens().unwrap();
    let stub = Arc::new(StubRuntime::new());
    stub.script("quick", Script::new(vec![Step::End("done".into())]));
    let clock = Arc::new(SetClock::at("2026-11-26T10:00:00Z").unwrap());
    let policy = Policy {
        default_confinement: Some(Confinement::Linux),
        kinds: vec!["model".into()],
        routes: Arc::new(table),
        ..Policy::default()
    };
    let d = Daemon::start_full(super::commands::config(&dir), policy, stub, clock.clone()).unwrap();
    (d, clock)
}

fn state_of(d: &Daemon, run: &str) -> Value {
    d.run_detail(run)
        .first()
        .map(|r| r["state"].clone())
        .unwrap_or(Value::Null)
}

/// Part two, "Failure": "A route that is down: `delegate` is refused at submit ..., or the run ends `stopped: route` or
/// `failed: route-unavailable`; the tool returns an error with that line." Story `claude-plugin-serves-delegation`:
/// "A down route gives a tool error naming the route". A managed route whose model never comes up ends the run
/// `stopped: route`, and the tool must say so as an error, or the session reads the empty result as the sub-agent's.
#[test]
fn adversary_b_mcp_delegate_of_a_delegation_stopped_on_its_route_is_a_tool_error_naming_the_route()
{
    let base = phase_gateway("stopped");
    let (d, clock) = waiting_daemon("advb-stopped", &base, 12);
    let backend = runbook::mcp::Local(d.clone());
    let call = std::thread::spawn(move || {
        runbook::mcp::tools::run(&backend, "delegate", &json!({"request": "x", "model_route": "qwen", "protocol": "quick", "wait_seconds": 30}), None).ok()
    });
    wait_for(
        "a run waiting for its route",
        Duration::from_secs(10),
        || d.runs().iter().any(|r| r["state"] == "WaitingForRoute"),
    );
    clock.set("2026-11-26T10:00:12Z");
    let r = call.join().unwrap().expect("delegate answered no result");
    let row = &r["structuredContent"]["rows"][0];
    assert_eq!(
        (row["run_state"].clone(), row["outcome"].clone()),
        (json!("Ended"), json!("stopped: route")),
        "{r}"
    );
    let text = r["content"][0]["text"].as_str().unwrap_or("");
    assert!(text.contains("qwen"), "{text}");
    assert_eq!(
        r["isError"],
        json!(true),
        "a delegation whose route never came up is answered as a success: {text}"
    );
}

/// The text of `delegation_result` for a run that has not ended says so ("The run has not ended"); a delegation that waits
/// for its route has not ended, and has not "wrote no result.md".
#[test]
fn adversary_b_the_result_of_a_delegation_waiting_for_its_route_does_not_say_it_wrote_no_result() {
    let base = phase_gateway("starting");
    let (d, _clock) = waiting_daemon("advb-waiting", &base, 600);
    let o = d
        .delegate(Delegate {
            request: "x".into(),
            model_route: Some("qwen".into()),
            protocol: Some("quick".into()),
            ..Delegate::default()
        })
        .unwrap();
    let run = o
        .run_id
        .clone()
        .unwrap_or_else(|| panic!("not delegated: {o:?}"));
    wait_for(
        "the run to wait for its route",
        Duration::from_secs(10),
        || state_of(&d, &run) == "WaitingForRoute",
    );
    let r = runbook::mcp::tools::run(
        &runbook::mcp::Local(d.clone()),
        "delegation_result",
        &json!({"run_id": run}),
        None,
    )
    .unwrap_or_else(|_| panic!("delegation_result answered no result"));
    let text = r["content"][0]["text"].as_str().unwrap_or("");
    assert_eq!(
        r["structuredContent"]["rows"][0]["run_state"],
        json!("WaitingForRoute"),
        "{r}"
    );
    assert!(
        !text.contains("wrote no result.md"),
        "a run that waits for its route is said to have written no result.md: {text}"
    );
}

// ---- the loom daemon: classify, else build ----

/// Section 8: "A delegation is a run: `cancel-run` ... work on it unchanged." A cancel that comes while the classifier
/// answers ends the run; no further model call is sent for it, and the build call of section 9 is one.
#[test]
fn adversary_b_a_cancel_while_the_classifier_answers_sends_no_build_call() {
    let asked = Arc::new(AtomicUsize::new(0));
    let answered = Arc::new(AtomicBool::new(false));
    let (a, done) = (asked.clone(), answered.clone());
    let gw = gateway_answering(
        move |g| {
            if g.body["model"] != "c-model" {
                return OK.to_vec();
            }
            if a.fetch_add(1, Ordering::SeqCst) == 0 {
                // the classify call: slow enough for the cancel to come while it is in flight; below the threshold
                std::thread::sleep(Duration::from_secs(2));
                done.store(true, Ordering::SeqCst);
                return messages(
                    &json!({"protocol": "answer-demo", "confidence": 0.5, "inputs": {}})
                        .to_string(),
                );
            }
            messages(r#"{"free": true}"#)
        },
        &["a-model", "c-model"],
    );
    let (_h, l, _roots) = daemon_at("advb-cancel", &gw.base_url, &format!("{ROUTE}{CLASSIFY}"));
    let (status, body) = delegate_http(
        l.addr,
        &json!({"request": "green or black", "model_route": "default"}),
    );
    assert_eq!(status, 202, "{body}");
    let run = run_of(&body);
    wait_for("the classify call", Duration::from_secs(20), || {
        asked.load(Ordering::SeqCst) >= 1
    });
    let c = send(
        l.addr,
        "POST",
        "/daemon/commands/cancel-run",
        &[("Content-Type", "application/json")],
        Some(json!({"run_id": run}).to_string().as_bytes()),
    );
    assert_eq!(c.status, 202, "{}", c.text());
    wait_for("the classifier's answer", Duration::from_secs(10), || {
        answered.load(Ordering::SeqCst)
    });
    std::thread::sleep(Duration::from_millis(1500));
    let end = until_ended(&l, &run);
    assert_eq!(end["run_state"], json!("Cancelled"), "{end}");
    assert_eq!(
        asked.load(Ordering::SeqCst),
        1,
        "the cancelled delegation sent the build call after its cancel: {end}"
    );
}

/// Section 9.1: "One model call on `route_policy.classifier` gets the request text and the catalog of runbooks the daemon
/// can run (id, title, description, inputs)". The catalog line of each runbook in the classify call names its inputs.
#[test]
fn adversary_b_the_classifier_is_told_the_inputs_of_each_runbook() {
    let gw = classifying(vec![
        json!({"protocol": "answer-demo", "confidence": 0.9, "inputs": {}}).to_string(),
    ]);
    let (_h, l, _roots) = daemon_at("advb-inputs", &gw.base_url, &format!("{ROUTE}{CLASSIFY}"));
    let (status, body) = delegate_http(
        l.addr,
        &json!({"request": "green or black", "model_route": "default"}),
    );
    assert_eq!(status, 202, "{body}");
    until_ended(&l, &run_of(&body));
    let calls = classifier_calls(&gw);
    let prompt = calls
        .first()
        .map(|c| {
            c.body["messages"][0]["content"]
                .as_str()
                .unwrap_or("")
                .to_owned()
        })
        .expect("no classify call");
    let entry: Value = prompt
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line.trim()).ok())
        .find(|v| v["id"] == "answer-demo")
        .unwrap_or_else(|| panic!("the classify call lists no runbook answer-demo: {prompt}"));
    assert!(
        entry
            .get("inputs")
            .is_some_and(|i| i.to_string().contains("idea")),
        "the classifier is not told the inputs of answer-demo (its bindings place idea): {entry}"
    );
}

/// Section 9.1: "With confidence at or above `delegation.classify_threshold` (default 0.8) and inputs the runbook
/// accepts, that runbook runs." An answer whose inputs the runbook does not take (answer-demo takes `idea` only) does
/// not run that runbook: the run goes on to build.
#[test]
fn adversary_b_a_classifier_answer_with_inputs_the_runbook_does_not_take_does_not_run_it() {
    let gw = classifying(vec![
        json!({"protocol": "answer-demo", "confidence": 0.9, "inputs": {"language": "French"}})
            .to_string(),
        REFUSED_BUILD.to_owned(),
    ]);
    let (h, l, _roots) = daemon_at(
        "advb-bad-inputs",
        &gw.base_url,
        &format!("{ROUTE}{CLASSIFY}"),
    );
    let (status, body) = delegate_http(
        l.addr,
        &json!({"request": "green or black", "model_route": "default"}),
    );
    assert_eq!(status, 202, "{body}");
    let end = until_ended(&l, &run_of(&body));
    assert!(
        !h.log.join("named-statuses").exists(),
        "answer-demo ran although it takes no input language: {end}"
    );
    assert_ne!(
        end["path"],
        json!("classified"),
        "the classifier's inputs were not checked against the runbook's: {end}"
    );
}

/// The text a `runbook serve` with the file `file` printed to its error stream when it did not start; none when it started.
fn refusal_of(tag: &str, file: &str) -> Option<String> {
    let dir = super::state::scratch(tag);
    let config = dir.join("runbook.toml");
    std::fs::write(&config, file).unwrap();
    let mut child = Command::new(BIN)
        .arg("serve")
        .args([
            "--principal",
            super::PRINCIPAL,
            "--listen",
            "127.0.0.1:0",
            "--runtime",
            "stub",
            "--min-confinement",
            "local",
            "--default-confinement",
            "local",
        ])
        .arg("--state-dir")
        .arg(dir.join("state"))
        .env_remove("RUNBOOK_TOKEN")
        .env("RUNBOOK_CONFIG", &config)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let out = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = BufReader::new(out).read_line(&mut line);
        let _ = tx.send(line);
    });
    let line = rx.recv_timeout(Duration::from_secs(10)).unwrap_or_default();
    if line.starts_with("runbook: listening on ") {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    }
    let _ = child.kill();
    let _ = child.wait();
    let mut err = String::new();
    let _ = child.stderr.take().unwrap().read_to_string(&mut err);
    Some(err)
}

/// Section 9.1: the threshold is `delegation.classify_threshold` (default 0.8), a setting of the daemon. With it at 0.95
/// an answer of 0.9 is below it, and the run builds instead of running the runbook the classifier named.
#[test]
fn adversary_b_the_classify_threshold_is_a_setting_of_the_daemon() {
    let gw = classifying(vec![
        json!({"protocol": "answer-demo", "confidence": 0.9, "inputs": {}}).to_string(),
        REFUSED_BUILD.to_owned(),
    ]);
    let threshold = "[delegation]\nclassify_threshold = 0.95\n";
    let pre = super::gateway::home("advb-threshold-pre");
    let file = format!(
        "{}{ROUTE}{CLASSIFY}{threshold}",
        super::gateway::gateway_table(&gw.base_url, &pre.token_file)
    );
    if let Some(err) = refusal_of("advb-threshold-pre", &file) {
        panic!(
            "a daemon whose runbook.toml sets delegation.classify_threshold does not start: {err}"
        );
    }
    let (h, l, _roots) = daemon_at(
        "advb-threshold",
        &gw.base_url,
        &format!("{ROUTE}{CLASSIFY}{threshold}"),
    );
    let (status, body) = delegate_http(
        l.addr,
        &json!({"request": "green or black", "model_route": "default"}),
    );
    assert_eq!(status, 202, "{body}");
    let end = until_ended(&l, &run_of(&body));
    assert!(
        !h.log.join("named-statuses").exists(),
        "answer-demo ran at 0.9 below the threshold 0.95: {end}"
    );
    assert_ne!(end["path"], json!("classified"), "{end}");
}

/// Part two, "What is recorded": "The run record (section 9): route, alias, the source of the route, the delegation path,
/// the protocol, the confidence, the caller (`claude-code`, and the label in `RUNBOOK_CALLER` of the session when set)."
/// A delegation through the plugin's MCP server (`runbook mcp`, started with the session's environment) records the label.
#[test]
fn adversary_b_a_delegation_through_runbook_mcp_records_the_callers_label() {
    const LABEL: &str = "advb-caller-label";
    let gw = super::relay::gateway_listing(OK, &["a-model", "b-model"]);
    let (_h, l, _roots) = daemon_at("advb-caller", &gw.base_url, ROUTE);
    let mut child = Command::new(BIN)
        .args(["mcp", "--daemon", &format!("http://{}", l.addr)])
        .env_remove("RUNBOOK_DAEMON")
        .env_remove("RUNBOOK_TOKEN")
        .env("RUNBOOK_CALLER", LABEL)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut out = BufReader::new(child.stdout.take().unwrap());
    let mut answer = |id: u64, message: Value| -> Value {
        writeln!(stdin, "{message}").unwrap();
        stdin.flush().unwrap();
        loop {
            let mut line = String::new();
            assert!(
                out.read_line(&mut line).unwrap() > 0,
                "runbook mcp ended before it answered {id}"
            );
            let v: Value = serde_json::from_str(&line).unwrap();
            if v["id"] == json!(id) {
                return v;
            }
        }
    };
    answer(
        0,
        json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "claude-code", "version": "0"}}}),
    );
    let r = answer(
        1,
        json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "delegate", "arguments": {"request": "green or black", "model_route": "default", "protocol": "answer-demo", "wait_seconds": 30}}}),
    );
    let _ = child.kill();
    let _ = child.wait();
    let run = r["result"]["structuredContent"]["rows"][0]["run_id"]
        .as_str()
        .unwrap_or_else(|| panic!("{r}"))
        .to_owned();
    let row = super::default_confinement::rows(&l, &format!("/daemon/views/run?run_id={run}"));
    let events = super::default_confinement::rows(
        &l,
        &format!("/daemon/views/events?run_id={run}&after_position=0"),
    );
    let record = format!("{row:?} {events:?}");
    assert!(
        record.contains(LABEL),
        "the run records no caller label of the session (RUNBOOK_CALLER): {}",
        json!(row)
    );
}
