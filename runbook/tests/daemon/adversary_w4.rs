//! Adversary cases of the wave "runs end green" (stories `a-recovered-call-does-not-fail-the-run` and
//! `a-failed-call-names-the-adapter-error`): `docs/model-routes.md` section 4, `docs/engine.md` "A gateway",
//! `docs/daemon-api.md` "The relay of a run's model calls".
//!
//! Each case drives the daemon over loopback only: the gateway is a stand-in on a port of the test's own, `claude` is a
//! stand-in in bash first on the daemon's `PATH`, and the bound programs of the case's own runbook call the run's relay
//! through its route file. No model is called.

use super::default_confinement::{rows, Live};
use super::delegate::{daemon_at, until_ended, ANSWER, ROUTE};
use super::gateway::{claude_cli, model_table, until_state, write_exe, Home};
use super::http::{send, try_send};
use super::relay::{gateway_answering, Gateway};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const OK: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nx-llm-gateway-call: gw-call-ok\r\nContent-Length: 56\r\nConnection: close\r\n\r\n{\"id\":\"m\",\"usage\":{\"input_tokens\":11,\"output_tokens\":5}}";

/// An answer of status `status` with the JSON `body` and the gateway's call id `call`.
fn answer(status: &str, call: &str, body: &str) -> Vec<u8> {
    format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nx-llm-gateway-call: {call}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).into_bytes()
}

/// The gateway's answer to a call whose upstream timed out: `upstream-failed` 502, call id `call`.
fn upstream_failed(call: &str) -> Vec<u8> {
    answer("502 Bad Gateway", call, &json!({"error": {"code": "upstream-failed", "message": "the upstream gave no answer: timeout"}}).to_string())
}

/// An upstream's refusal the gateway relayed: `invalid_request_error` 400, call id `call`.
fn upstream_refusal(call: &str) -> Vec<u8> {
    answer("400 Bad Request", call, &json!({"type": "error", "error": {"type": "invalid_request_error", "message": "max_tokens: 64000 > 32000"}}).to_string())
}

/// A streamed answer with status 200 that the upstream ended with an error event (the messages wire's `event: error`,
/// here `overloaded_error`) after it had started the message: no content, no `message_stop`.
fn streamed_error(call: &str) -> Vec<u8> {
    let body = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"m\",\"usage\":{\"input_tokens\":11,\"output_tokens\":1}}}\n\nevent: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n\n";
    format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nx-llm-gateway-call: {call}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).into_bytes()
}

/// The bash function `call` of a stand-in client: one Messages call to `$base` (with `$token`, model `$model`) over
/// its own connection, its status line appended to `<log>/statuses`. With `$1` `hang-up` the client sends the call,
/// waits a moment (the relay has taken it) and closes its connection without reading the answer, as a client does
/// that gave up on a call or exited with it open.
fn call_function(log: &Path) -> String {
    format!(
        r#"call() {{
  local body="{{\"model\":\"$model\",\"max_tokens\":8,\"messages\":[{{\"role\":\"user\",\"content\":\"hi\"}}]}}"
  local rest=${{base#http://}}
  local hostport=${{rest%%/*}}
  local path=''
  [ "$rest" = "$hostport" ] || path=/${{rest#*/}}
  exec 3<>"/dev/tcp/${{hostport%:*}}/${{hostport##*:}}" || return 7
  printf 'POST %s/v1/messages HTTP/1.0\r\nHost: %s\r\nAuthorization: Bearer %s\r\nContent-Type: application/json\r\nContent-Length: %s\r\n\r\n%s' "$path" "$hostport" "$token" "${{#body}}" "$body" >&3
  if [ "$1" = hang-up ]; then sleep 0.3; exec 3<&-; return 0; fi
  local answer
  answer=$(cat <&3)
  exec 3<&-
  status=${{answer%%$'\r'*}}
  printf '%s\n' "$status" >> '{log}/statuses'
}}
"#,
        log = log.display()
    )
}

/// The line of a stand-in client that sends a call and hangs up on it: the relay goes on with the call ("a caller that
/// hangs up drops this handler and not the call", `runbook/src/relay.rs`) and books it when the gateway answers.
const LATE_CALL: &str = "call hang-up\n";

/// A `claude` in bash for the free run's agent node: its first call is answered (200), it sends a second call and
/// hangs up on it, and while that one is in flight at the relay the agent writes `out/result.md` and exits 0, as a
/// client does that ends with a side call still open.
fn claude_leaving_a_call_in_flight(log: &Path) -> String {
    format!(
        r#"#!/bin/bash
model=''
prev=''
for a in "$@"; do [ "$prev" = --model ] && model=$a; prev=$a; done
base=$ANTHROPIC_BASE_URL
token=${{ANTHROPIC_AUTH_TOKEN-}}
{call}call ''
case $status in *" 200 "*) ;; *) exit 8 ;; esac
{late}mkdir -p out && printf '%s\n' '{ANSWER}' > out/result.md
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"num_turns":1,"total_cost_usd":0,"duration_ms":1,"usage":{{"input_tokens":0,"output_tokens":0}},"result":"ok","terminal_reason":"completed"}}'
"#,
        call = call_function(log),
        late = LATE_CALL
    )
}

/// A `claude` in bash for the free run's agent node that sends `calls` calls one after the other and, when the status
/// line of the last is 200, writes `out/result.md` and exits 0 (else exits 8): a client that retried.
fn claude_retrying(log: &Path, calls: u32) -> String {
    format!(
        r#"#!/bin/bash
model=''
prev=''
for a in "$@"; do [ "$prev" = --model ] && model=$a; prev=$a; done
base=$ANTHROPIC_BASE_URL
token=${{ANTHROPIC_AUTH_TOKEN-}}
{call}for i in $(seq 1 {calls}); do call ''; done
case $status in *" 200 "*) ;; *) exit 8 ;; esac
mkdir -p out && printf '%s\n' '{ANSWER}' > out/result.md
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"num_turns":1,"total_cost_usd":0,"duration_ms":1,"usage":{{"input_tokens":0,"output_tokens":0}},"result":"ok","terminal_reason":"completed"}}'
"#,
        call = call_function(log)
    )
}

/// A daemon with the route `default` on `gw` whose `claude` is `claude`.
fn daemon(tag: &str, gw: &Gateway, claude: impl Fn(&Path) -> String) -> (Home, Live) {
    let (h, l, _roots) = daemon_at(tag, &gw.base_url, &format!("{}{ROUTE}", model_table(Some(&claude_cli()))));
    write_exe(&h.log.join("bin/claude"), &claude(&h.log));
    (h, l)
}

/// Delegates a request on the route `default`; the run's id.
fn delegated(l: &Live) -> String {
    let body = json!({"request": "Write one sentence about tea.", "model_route": "default"}).to_string();
    let r = send(l.addr, "POST", "/daemon/commands/delegate", &[("Content-Type", "application/json")], Some(body.as_bytes()));
    assert_eq!(r.status, 202, "{}", r.text());
    r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned()
}

fn events(l: &Live, run: &str, kind: &str) -> Vec<Value> {
    rows(l, &format!("/daemon/views/events?run_id={run}&after_position=0")).into_iter().filter(|e| e["kind"] == kind).map(|e| e["record"].clone()).collect()
}

/// The `gateway_call` of every call the run's events `runbook.model-calls-failed` name.
fn named_failed(l: &Live, run: &str) -> Vec<Value> {
    events(l, run, "runbook.model-calls-failed").iter().flat_map(|e| e["calls"].as_array().cloned().unwrap_or_default()).map(|c| c["gateway_call"].clone()).collect()
}

/// The longest the stand-in gateway holds the call that is left in flight while it waits for its signal: a guard against
/// a hang only, under the test's 60 s.
const HOLD_BOUND: Duration = Duration::from_secs(30);

/// The signal the stand-in gateway holds the call left in flight for: the daemon's address and the run, set by the test
/// once it has them. The gateway answers the call at most one poll (20 ms) after the run's events hold the marker that
/// its step was booked, well inside the run's wait for its calls in flight when it ends (5 s,
/// `runbook/src/engine/mod.rs`), and at the latest after `HOLD_BOUND`.
#[derive(Clone, Default)]
struct Signal(Arc<Mutex<Option<(SocketAddr, String)>>>);

impl Signal {
    fn set(&self, addr: SocketAddr, run: &str) {
        *self.0.lock().unwrap() = Some((addr, run.to_owned()));
    }

    /// Holds until the run's events hold `nth` events of the kind `kind`, or `HOLD_BOUND` passed.
    fn wait(&self, kind: &str, nth: usize) {
        let until = Instant::now() + HOLD_BOUND;
        while Instant::now() < until {
            let target = self.0.lock().unwrap().clone();
            if let Some((addr, run)) = target {
                let seen = try_send(addr, "GET", &format!("/daemon/views/events?run_id={run}&after_position=0"), &[], None)
                    .ok()
                    .filter(|r| r.status == 200)
                    .and_then(|r| r.json()["rows"].as_array().map(|rows| rows.iter().filter(|e| e["kind"] == kind).count()));
                if seen.is_some_and(|n| n >= nth) {
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

/// The case's precondition, read from the order of the run's events: the event `runbook.model-call` of `call` comes
/// after the `nth` event of the kind `after`, one the run records after the call's own step was booked (a call booked
/// before that is recorded as its step is booked, before it). So the call was in flight when its step was booked.
fn booked_after_its_step(l: &Live, run: &str, call: &str, after: &str, nth: usize) {
    let all: Vec<(Value, Value)> = rows(l, &format!("/daemon/views/events?run_id={run}&after_position=0")).into_iter().map(|e| (e["kind"].clone(), e["record"]["gateway_call"].clone())).collect();
    let mark = all.iter().enumerate().filter(|(_, (kind, _))| kind == after).nth(nth - 1).map(|(i, _)| i);
    let late = all.iter().position(|(kind, id)| kind == "runbook.model-call" && id == call);
    assert!(matches!((mark, late), (Some(m), Some(c)) if m < c), "setup: the call {call} was not booked after its step (the case needs it in flight then): {all:?}");
}

/// `docs/engine.md` "A gateway": the run's record names the failed calls of a step that went on with one event
/// `runbook.model-calls-failed`; `docs/daemon-api.md`: "a call that was sent is booked once". A call the agent left in
/// flight when it exited 0 is booked by the relay after the step was booked (the run waits for it before it ends and
/// records its line as `runbook.model-call`); when the gateway answered it 502, the run's record must name it as a
/// failed call. The relay keeps it among the failed calls of the step, but nothing takes them after the last step.
#[test]
fn adversary_w4_a_failed_call_still_in_flight_when_the_last_step_was_booked_is_named_in_the_record() {
    let n = Arc::new(AtomicU64::new(0));
    let signal = Signal::default();
    let held = signal.clone();
    let gw = gateway_answering(
        move |_| match n.fetch_add(1, Ordering::SeqCst) {
            0 => OK.to_vec(),
            _ => {
                held.wait("Terminated", 1);
                upstream_failed("gw-call-late")
            }
        },
        &["a-model"],
    );
    let (_h, l) = daemon("adv-w4-late", &gw, claude_leaving_a_call_in_flight);
    let run = delegated(&l);
    signal.set(l.addr, &run);
    let end = until_ended(&l, &run);
    assert_eq!((end["run_state"].clone(), end["outcome"].clone()), (json!("Ended"), json!("delivered")), "{end}");
    booked_after_its_step(&l, &run, "gw-call-late", "Terminated", 1);
    let calls = events(&l, &run, "runbook.model-call");
    assert!(
        calls.iter().any(|c| c["gateway_call"] == "gw-call-late" && c["cause"] == "route-unavailable" && c["code"] == "upstream-failed"),
        "the late call was not booked as a failed call of the run: {calls:?}"
    );
    let named = named_failed(&l, &run);
    assert_eq!(named, vec![json!("gw-call-late")], "the run's record does not name the failed call that ended after its step was booked: model-calls-failed {:?}", events(&l, &run, "runbook.model-calls-failed"));
}

/// A runbook of two bound programs on the run's route. `notes.write` sends one call, hangs up on it and exits 0 with
/// `notes.md` while it is in flight at the relay; `result.write` (after it) sends one call and exits 1.
fn late_then_failing_runbook(h: &Home) {
    let protocol = "format: protocol/1\nprotocol:\n  id: late-demo\n  revision: 1\n  description: >-\n    An adversary case's runbook: a step that leaves a call in flight, then a step whose program fails.\nartifacts:\n  idea: {}\n  notes: {}\n  result: {}\nevidence_kinds:\n  notes_written: {}\n  result_written: {}\nclaims:\n  notes.written:\n    true_when:\n      evidence: {kind: notes_written, subject: notes, result: written}\n  result.written:\n    true_when:\n      evidence: {kind: result_written, subject: result, result: written}\nactions:\n  notes.write:\n    effect: write\n    precondition: {claim: notes.written, is: unknown}\n    may_produce:\n      - evidence: notes_written\n  result.write:\n    effect: write\n    precondition: {claim: notes.written}\n    may_produce:\n      - evidence: result_written\noutcomes:\n  answered:\n    requires: {claim: result.written}\n";
    let bindings = "format: loom.bindings/1\nprotocol: late-demo@1\nprogram_root: bindings\nartifacts:\n  idea: idea.yaml\n  notes: notes.md\n  result: result.md\nactions:\n  - action: notes.write\n    executor:\n      kind: program\n      argv: [tools/late-notes.sh, --max-budget-usd, \"0.5\"]\n    timeout_seconds: 20\n    evidence:\n      - kind: notes_written\n        subject: notes\n        producer: late\n        from: exit\n        results: {\"0\": written, timeout: failed, \"*\": failed}\n  - action: result.write\n    executor:\n      kind: program\n      argv: [tools/late-result.sh, --max-budget-usd, \"0.5\"]\n    timeout_seconds: 20\n    evidence:\n      - kind: result_written\n        subject: result\n        producer: late\n        from: exit\n        results: {\"0\": written, timeout: failed, \"*\": failed}\n";
    std::fs::write(h.home.join("protocol/late-demo.protocol.yaml"), protocol).unwrap();
    std::fs::write(h.home.join("bindings/late-demo.yaml"), bindings).unwrap();
    let head = format!(
        "#!/bin/bash\nf=$RUNBOOK_ROUTE_FILE\nbase=$(jq -r .base_url \"$f\") && token=$(jq -r .token \"$f\") || exit 9\nmodel=default\n{}",
        call_function(&h.log)
    );
    write_exe(&h.home.join("bindings/tools/late-notes.sh"), &format!("{head}{}printf 'notes\\n' > notes.md\n", LATE_CALL));
    write_exe(&h.home.join("bindings/tools/late-result.sh"), &format!("{head}call ''\nexit 1\n"));
}

/// Story `a-recovered-call-does-not-fail-the-run`, Acceptance: "A failed call in a step whose program exits non-zero
/// ends the run with that call as its first cause." The step `result.write` makes one call, the gateway answers it
/// 400 (`invalid_request_error`), and its program exits 1. A call `notes.write` left in flight before (answered 502
/// while `result.write` runs) stands first among the failed calls taken after `result.write`: the run is settled by the
/// step of that earlier call, which went on, so the refused call of the failed step is noted as one its step recovered
/// from and the run does not end with it as its cause.
#[test]
fn adversary_w4_a_late_failed_call_of_an_earlier_step_does_not_take_the_place_of_the_failed_steps_own_call() {
    let n = Arc::new(AtomicU64::new(0));
    let signal = Signal::default();
    let held = signal.clone();
    let gw = gateway_answering(
        move |_| match n.fetch_add(1, Ordering::SeqCst) {
            0 => {
                held.wait("runbook.action-started", 2);
                upstream_failed("gw-call-late")
            }
            _ => upstream_refusal("gw-call-refused"),
        },
        &["a-model"],
    );
    let (h, l, _roots) = daemon_at("adv-w4-order", &gw.base_url, &format!("{}{ROUTE}", model_table(Some(&claude_cli()))));
    late_then_failing_runbook(&h);
    let r = send(l.addr, "POST", "/daemon/commands/submit-run", &[("Content-Type", "application/json")], Some(json!({"runbook_id": "late-demo", "intake": "red or blue", "model_route": "default"}).to_string().as_bytes()));
    assert_eq!(r.status, 202, "{}", r.text());
    let run = r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned();
    signal.set(l.addr, &run);
    let end = until_state(&l, &run, &["Ended", "FailedToStart", "Cancelled"]);
    let calls = events(&l, &run, "runbook.model-call");
    let steps: Vec<(Value, Value, Value)> = calls.iter().map(|c| (c["step"].clone(), c["gateway_call"].clone(), c["cause"].clone())).collect();
    assert!(
        steps.contains(&(json!("notes.write"), json!("gw-call-late"), json!("route-unavailable"))) && steps.contains(&(json!("result.write"), json!("gw-call-refused"), json!("call-refused"))),
        "the two failed calls were not booked with their steps: {steps:?} {end}"
    );
    booked_after_its_step(&l, &run, "gw-call-late", "runbook.action-started", 2);
    let reasons: Vec<Value> = events(&l, &run, "runbook.runtime-failed").into_iter().map(|e| e["reason"].clone()).collect();
    let first = reasons.first().and_then(Value::as_str).unwrap_or_default();
    assert!(
        first.starts_with("call-refused: route default (chosen by run) refused the call: invalid_request_error"),
        "the failed step's refused call is not the run's first cause: outcome {} reasons {reasons:?} model-calls-failed {:?}",
        end["outcome"],
        events(&l, &run, "runbook.model-calls-failed")
    );
}

/// `docs/engine.md` "A gateway" and the story's Acceptance: the run's record names each failed call of a step that went
/// on. The gateway answers the first call 200 with a stream that the upstream ended with an error event (no content),
/// the agent retries, its second call is answered, it exits 0 with `result.md`. The first call failed; the relay books
/// it as an answered call (no `cause`), and the record does not name it.
#[test]
fn adversary_w4_a_streamed_answer_that_ended_in_an_error_event_is_a_failed_call() {
    let n = Arc::new(AtomicU64::new(0));
    let gw = gateway_answering(move |_| if n.fetch_add(1, Ordering::SeqCst) == 0 { streamed_error("gw-call-overloaded") } else { OK.to_vec() }, &["a-model"]);
    let (_h, l) = daemon("adv-w4-sse", &gw, |log| claude_retrying(log, 2));
    let run = delegated(&l);
    let end = until_ended(&l, &run);
    assert_eq!((end["run_state"].clone(), end["outcome"].clone()), (json!("Ended"), json!("delivered")), "{end}");
    let calls = events(&l, &run, "runbook.model-call");
    let first: Vec<&Value> = calls.iter().filter(|c| c["gateway_call"] == "gw-call-overloaded").collect();
    assert_eq!(first.len(), 1, "{calls:?}");
    assert!(first[0]["cause"].is_string() && first[0]["code"].is_string(), "the call whose stream ended in an error event was booked as answered: {}", first[0]);
    assert_eq!(named_failed(&l, &run), vec![json!("gw-call-overloaded")], "the run's record does not name the failed call");
}
