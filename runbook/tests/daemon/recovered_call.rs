//! A model call the gateway refuses with a 4xx answer the client may recover from (story
//! `a-recovered-call-does-not-fail-the-run`, `docs/model-routes.md` section 4): the relay passes the refusal to the
//! client and books it with the cause `call-refused`, the gateway's code and its error text; the run ends as its step
//! ended. A step that exits 0 with its product delivered ends the run delivered, and the run's record names the refused
//! calls (`runbook.model-calls-refused`); a step that exits non-zero ends the run with the refused call as its first
//! cause. A refusal of the route by the gateway itself still fails the step `route-unavailable`.
//!
//! No model is called and nothing leaves loopback: the gateway is a stand-in on a port of the test's own that answers
//! the first call with a refusal and every later one 200, and `claude` is a stand-in in bash first on the daemon's
//! `PATH`.

use super::default_confinement::{rows, Live};
use super::delegate::{daemon_at, until_ended, ANSWER, ROUTE};
use super::gateway::{claude_cli, model_table, write_exe, Home, TOKEN};
use super::http::send;
use super::relay::{gateway_answering, got, Gateway};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

const OK: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nx-llm-gateway-call: gw-call-ok\r\nContent-Length: 56\r\nConnection: close\r\n\r\n{\"id\":\"m\",\"usage\":{\"input_tokens\":11,\"output_tokens\":5}}";

/// The message of the upstream's refusal; it names the gateway's token, which no record may hold.
fn upstream_message() -> String {
    format!("max_tokens: 64000 > 32000, the most this model allows (key {TOKEN})")
}

/// An answer of status `status` with the JSON `body` and the gateway's call id `call`.
fn answer(status: &str, call: &str, body: &str) -> Vec<u8> {
    format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nx-llm-gateway-call: {call}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).into_bytes()
}

/// The refusal an upstream gave and the gateway relayed: the messages wire's error shape.
fn upstream_refusal() -> Vec<u8> {
    let body = json!({"type": "error", "error": {"type": "invalid_request_error", "message": upstream_message()}}).to_string();
    answer("400 Bad Request", "gw-call-refused", &body)
}

/// The gateway's own refusal of the route: `wire-not-served` 400.
fn route_refusal() -> Vec<u8> {
    let body = json!({"error": {"code": "wire-not-served", "message": "the route does not serve this wire"}}).to_string();
    answer("400 Bad Request", "gw-call-wire", &body)
}

/// A gateway, ready and listing `a-model`, that answers the first call with `first` and every later one 200.
fn first_refused(first: fn() -> Vec<u8>) -> Gateway {
    let n = Arc::new(AtomicU64::new(0));
    gateway_answering(move |_| if n.fetch_add(1, Ordering::SeqCst) == 0 { first() } else { OK.to_vec() }, &["a-model"])
}

/// A `claude` in bash for the free run's agent node that goes on after a refused call, as Claude Code does: it sends
/// `calls` Messages calls to `ANTHROPIC_BASE_URL` one after the other, keeps each status (`log/statuses`), and when its
/// last call was answered 200 writes `out/result.md` and exits 0; otherwise it exits 8.
fn claude_recovering(log: &Path, calls: u32) -> String {
    format!(
        r#"#!/bin/bash
log='{log}'
model=''
prev=''
for a in "$@"; do [ "$prev" = --model ] && model=$a; prev=$a; done
rest=${{ANTHROPIC_BASE_URL#http://}}
hostport=${{rest%%/*}}
base=''
[ "$rest" = "$hostport" ] || base=/${{rest#*/}}
body="{{\"model\":\"$model\",\"max_tokens\":8,\"messages\":[{{\"role\":\"user\",\"content\":\"hi\"}}]}}"
status=''
for i in $(seq 1 {calls}); do
  exec 3<>"/dev/tcp/${{hostport%:*}}/${{hostport##*:}}" || exit 7
  printf 'POST %s/v1/messages HTTP/1.0\r\nHost: %s\r\nAuthorization: Bearer %s\r\nContent-Type: application/json\r\nContent-Length: %s\r\n\r\n%s' "$base" "$hostport" "${{ANTHROPIC_AUTH_TOKEN-}}" "${{#body}}" "$body" >&3
  answer=$(cat <&3)
  exec 3<&-
  status=${{answer%%$'\r'*}}
  printf '%s\n' "$status" >> "$log/statuses"
done
case $status in *" 200 "*) ;; *) exit 8 ;; esac
mkdir -p out && printf '%s\n' '{ANSWER}' > out/result.md
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"num_turns":1,"total_cost_usd":0,"duration_ms":1,"usage":{{"input_tokens":0,"output_tokens":0}},"result":"ok","terminal_reason":"completed"}}'
"#,
        log = log.display()
    )
}

/// A daemon with the route `default` on `gw` whose free run's `claude` makes `calls` calls.
fn daemon(tag: &str, gw: &Gateway, calls: u32) -> (Home, Live) {
    let (h, l, _roots) = daemon_at(tag, &gw.base_url, &format!("{}{ROUTE}", model_table(Some(&claude_cli()))));
    write_exe(&h.log.join("bin/claude"), &claude_recovering(&h.log, calls));
    (h, l)
}

/// Delegates `request` on the route `default`; the run's id.
fn delegated(l: &Live) -> String {
    let body = json!({"request": "Write one sentence about tea.", "model_route": "default"}).to_string();
    let r = send(l.addr, "POST", "/daemon/commands/delegate", &[("Content-Type", "application/json")], Some(body.as_bytes()));
    assert_eq!(r.status, 202, "{}", r.text());
    r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned()
}

fn events(l: &Live, run: &str, kind: &str) -> Vec<Value> {
    rows(l, &format!("/daemon/views/events?run_id={run}&after_position=0")).into_iter().filter(|e| e["kind"] == kind).map(|e| e["record"].clone()).collect()
}

/// The spend ledger's lines of the run `run` whose cause is `cause`.
fn ledger_lines(l: &Live, run: &str, cause: &str) -> Vec<Value> {
    let text = std::fs::read_to_string(l.dir.join("state/ledgers/delegation-free.json")).unwrap_or_default();
    let mut found = Vec::new();
    let mut stack = vec![serde_json::from_str::<Value>(&text).unwrap_or(Value::Null)];
    while let Some(v) = stack.pop() {
        match v {
            Value::Object(ref o) if o.get("run") == Some(&json!(run)) && o.get("cause") == Some(&json!(cause)) => found.push(v.clone()),
            Value::Object(o) => stack.extend(o.into_iter().map(|(_, v)| v)),
            Value::Array(a) => stack.extend(a),
            _ => {}
        }
    }
    found
}

/// The stand-in gateway answers 400 (`invalid_request_error`, an upstream's refusal it relayed) to the first call and
/// 200 after; the agent goes on, its next call is answered, it exits 0 and writes `result.md`. The run ends delivered
/// with its result text, the refused call is booked `call-refused` with the gateway's code and its error text (no
/// token), and the run's record names the refused call.
#[test]
fn a_call_refused_400_that_the_agent_recovered_from_leaves_the_run_delivered_and_is_named_in_its_record() {
    let gw = first_refused(upstream_refusal);
    let (h, l) = daemon("rc-recovered", &gw, 2);
    let run = delegated(&l);
    let end = until_ended(&l, &run);
    let statuses = std::fs::read_to_string(h.log.join("statuses")).unwrap_or_default();
    assert!(statuses.lines().next().is_some_and(|s| s.contains(" 400 ")) && statuses.lines().nth(1).is_some_and(|s| s.contains(" 200 ")), "the client was not passed the refusal and then the answer: {statuses}");
    assert_eq!((end["run_state"].clone(), end["outcome"].clone(), end["failure"].clone()), (json!("Ended"), json!("delivered"), Value::Null), "{end}");
    assert_eq!(end["result_text"], json!(format!("{ANSWER}\n")), "{end}");
    assert_eq!(got(&gw).iter().filter(|g| g.head.starts_with("POST ")).count(), 2);

    let calls = events(&l, &run, "runbook.model-call");
    let refused: Vec<&Value> = calls.iter().filter(|c| c["cause"] == "call-refused").collect();
    assert_eq!(refused.len(), 1, "{calls:?}");
    let error = refused[0]["error"].as_str().unwrap_or_default();
    assert_eq!((refused[0]["code"].clone(), refused[0]["gateway_call"].clone()), (json!("invalid_request_error"), json!("gw-call-refused")), "{calls:?}");
    assert!(error.starts_with("invalid_request_error: max_tokens: 64000 > 32000") && error.len() <= 300 && !error.contains(TOKEN), "{error}");
    let booked = ledger_lines(&l, &run, "call-refused");
    assert_eq!(booked.len(), 1, "the spend ledger holds no line of the refused call");
    assert_eq!((booked[0]["code"].clone(), booked[0]["error"].clone()), (json!("invalid_request_error"), json!(error)), "{booked:?}");
    assert!(!booked[0].to_string().contains(TOKEN), "{booked:?}");

    let noted = events(&l, &run, "runbook.model-calls-refused");
    assert_eq!(noted.len(), 1, "the run's record does not name the refused calls: {noted:?}");
    assert_eq!(noted[0]["step"], "delegation.free", "{noted:?}");
    let named = noted[0]["calls"].as_array().cloned().unwrap_or_default();
    assert_eq!(named.len(), 1, "{noted:?}");
    assert_eq!((named[0]["gateway_call"].clone(), named[0]["code"].clone(), named[0]["route"].clone()), (json!("gw-call-refused"), json!("invalid_request_error"), json!("default")), "{noted:?}");
}

/// The same refusal in a step whose program exits non-zero: the run fails, and its failure names the refused call
/// first, with the route, the gateway's code and the setting that chose the route.
#[test]
fn a_call_refused_400_in_a_step_that_exits_non_zero_is_the_first_cause_of_the_runs_failure() {
    let gw = first_refused(upstream_refusal);
    let (_h, l) = daemon("rc-exit", &gw, 1);
    let run = delegated(&l);
    let end = until_ended(&l, &run);
    let failure = end["failure"].as_str().unwrap_or_default();
    assert_ne!(end["outcome"], json!("delivered"), "{end}");
    assert!(failure.starts_with("call-refused: route default (chosen by run)") && failure.contains("invalid_request_error") && failure.contains("400"), "{end}");
    assert!(events(&l, &run, "runbook.model-calls-refused").is_empty(), "a failed step's refused call was noted as recovered");
}

/// The gateway's own refusal of the route (`wire-not-served` 400) is no refusal a client recovers from: the step that
/// made the call fails `route-unavailable` though its program went on and exited 0.
#[test]
fn a_refusal_of_the_route_by_the_gateway_still_fails_the_step_route_unavailable() {
    let gw = first_refused(route_refusal);
    let (_h, l) = daemon("rc-route", &gw, 2);
    let run = delegated(&l);
    let end = until_ended(&l, &run);
    let failure = end["failure"].as_str().unwrap_or_default();
    assert_ne!(end["outcome"], json!("delivered"), "{end}");
    assert!(failure.starts_with("route-unavailable: route default") && failure.contains("wire-not-served"), "{end}");
    let calls = events(&l, &run, "runbook.model-call");
    assert!(calls.iter().any(|c| c["cause"] == "route-unavailable" && c["code"] == "wire-not-served"), "{calls:?}");
}
