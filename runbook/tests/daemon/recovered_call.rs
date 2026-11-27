//! A model call that failed does not end the run by itself (story `a-recovered-call-does-not-fail-the-run`,
//! `docs/model-routes.md` section 4): a 4xx answer the client may recover from (booked `call-refused`), a 5xx answer,
//! a gateway timeout and a connection that fails (booked `route-unavailable`) are passed to the client, booked with the
//! gateway's code and its error text, and the run ends as its step ended. A step that exits 0 with its product
//! delivered ends the run delivered, and the run's record names the failed calls (`runbook.model-calls-failed`); a step
//! that exits non-zero ends the run with the first failed call as its first cause. The gateway's own refusals of the
//! route and its spend ceiling still fail the step `route-unavailable` at once.
//!
//! No model is called and nothing leaves loopback: the gateway is a stand-in on a port of the test's own that answers
//! the first call with a refusal and every later one 200, and `claude` is a stand-in in bash first on the daemon's
//! `PATH`.

use super::default_confinement::{rows, Live};
use super::delegate::{daemon_at, until_ended, ANSWER, ROUTE};
use super::gateway::{claude_cli, model_table, until_state, write_exe, Home, TOKEN};
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

/// The gateway's answer to a call whose upstream timed out: `upstream-failed` 502.
fn upstream_failed() -> Vec<u8> {
    let body = json!({"error": {"code": "upstream-failed", "message": "the upstream gave no answer: timeout"}}).to_string();
    answer("502 Bad Gateway", "gw-call-timeout", &body)
}

/// The gateway's spend ceiling: `spend-ceiling` 503.
fn spend_ceiling() -> Vec<u8> {
    let body = json!({"error": {"code": "spend-ceiling", "message": "a pod start would pass a cap"}}).to_string();
    answer("503 Service Unavailable", "gw-call-ceiling", &body)
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

    let noted = events(&l, &run, "runbook.model-calls-failed");
    assert_eq!(noted.len(), 1, "the run's record does not name the refused calls: {noted:?}");
    assert_eq!(noted[0]["step"], "delegation.free", "{noted:?}");
    let named = noted[0]["calls"].as_array().cloned().unwrap_or_default();
    assert_eq!(named.len(), 1, "{noted:?}");
    assert_eq!((named[0]["gateway_call"].clone(), named[0]["code"].clone(), named[0]["route"].clone()), (json!("gw-call-refused"), json!("invalid_request_error"), json!("default")), "{noted:?}");
    assert_eq!(named[0]["cause"], "call-refused", "{noted:?}");
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
    assert!(events(&l, &run, "runbook.model-calls-failed").is_empty(), "a failed step's refused call was noted as recovered");
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

/// The stand-in gateway answers 502 (`upstream-failed`, the upstream timed out) to the first call and 200 after; the
/// agent retries, its next call is answered, it exits 0 and writes `result.md`. The run ends delivered, the failed call
/// is booked `route-unavailable` with the gateway's code and its error text, and the run's record names it with its
/// cause.
#[test]
fn a_call_answered_502_that_the_agent_retried_leaves_the_run_delivered_and_is_named_in_its_record() {
    let gw = first_refused(upstream_failed);
    let (h, l) = daemon("rc-502", &gw, 2);
    let run = delegated(&l);
    let end = until_ended(&l, &run);
    let statuses = std::fs::read_to_string(h.log.join("statuses")).unwrap_or_default();
    assert!(statuses.lines().next().is_some_and(|s| s.contains(" 502 ")) && statuses.lines().nth(1).is_some_and(|s| s.contains(" 200 ")), "the client was not passed the 502 and then the answer: {statuses}");
    assert_eq!((end["run_state"].clone(), end["outcome"].clone(), end["failure"].clone()), (json!("Ended"), json!("delivered"), Value::Null), "{end}");
    assert_eq!(end["result_text"], json!(format!("{ANSWER}\n")), "{end}");
    let calls = events(&l, &run, "runbook.model-call");
    let failed: Vec<&Value> = calls.iter().filter(|c| c["cause"] == "route-unavailable").collect();
    assert_eq!(failed.len(), 1, "{calls:?}");
    assert_eq!((failed[0]["code"].clone(), failed[0]["error"].clone()), (json!("upstream-failed"), json!("upstream-failed: the upstream gave no answer: timeout")), "{calls:?}");
    let noted = events(&l, &run, "runbook.model-calls-failed");
    assert_eq!(noted.len(), 1, "the run's record does not name the failed call: {noted:?}");
    let named = noted[0]["calls"].as_array().cloned().unwrap_or_default();
    assert_eq!(named.len(), 1, "{noted:?}");
    assert_eq!((named[0]["gateway_call"].clone(), named[0]["cause"].clone(), named[0]["code"].clone()), (json!("gw-call-timeout"), json!("route-unavailable"), json!("upstream-failed")), "{noted:?}");
}

/// A 502 in a step whose program exits non-zero: the run fails naming that call first.
#[test]
fn a_call_answered_502_in_a_step_that_exits_non_zero_is_the_first_cause_of_the_runs_failure() {
    let gw = first_refused(upstream_failed);
    let (_h, l) = daemon("rc-502-exit", &gw, 1);
    let run = delegated(&l);
    let end = until_ended(&l, &run);
    let failure = end["failure"].as_str().unwrap_or_default();
    assert_ne!(end["outcome"], json!("delivered"), "{end}");
    assert!(failure.starts_with("route-unavailable: route default (chosen by run) failed the call: upstream-failed"), "{end}");
    assert!(events(&l, &run, "runbook.model-calls-failed").is_empty(), "a failed step's failed call was noted as recovered");
}

/// A child protocol `late.<name>` with its bindings `late-<name>.yaml`: one program step `<name>.write` that runs
/// `tools/late-<name>.sh` and writes the artifact `<name>` (`<name>.md`); exit 0, 2 and 3 are taken as `written`, and
/// the case passes once it is written.
fn late_child(h: &Home, name: &str) {
    let protocol = format!(
        "format: protocol/1\nprotocol:\n  id: late.{name}\n  revision: 1\n  description: >-\n    One step that writes the {name}.\nartifacts:\n  idea: {{}}\n  {name}: {{}}\nevidence_kinds:\n  {name}_written: {{}}\nclaims:\n  {name}.written:\n    true_when:\n      evidence: {{kind: {name}_written, subject: {name}, result: written}}\nactions:\n  {name}.write:\n    effect: write\n    precondition: {{claim: {name}.written, is: unknown}}\n    may_produce:\n      - evidence: {name}_written\noutcomes:\n  passed:\n    requires: {{claim: {name}.written}}\n"
    );
    let bindings = format!(
        "format: loom.bindings/1\nprotocol: late-{name}@1\nprogram_root: bindings\nartifacts:\n  idea: idea.yaml\n  {name}: {name}.md\nactions:\n  - action: {name}.write\n    executor:\n      kind: program\n      argv: [tools/late-{name}.sh, --max-budget-usd, \"0.5\"]\n    exits: {{error: [1, 75, 126, 127]}}\n    timeout_seconds: 20\n    evidence:\n      - kind: {name}_written\n        subject: {name}\n        producer: late\n        from: exit\n        results: {{\"0\": written, \"2\": written, \"3\": written, timeout: failed, \"*\": failed}}\n"
    );
    std::fs::write(h.home.join(format!("protocol/late-{name}.protocol.yaml")), protocol).unwrap();
    std::fs::write(h.home.join(format!("bindings/late-{name}.yaml")), bindings).unwrap();
}

/// A runbook `late-tree`, the shape of a report whose first model step timed out and whose next one wrote its product:
/// the root opens the child case `late.notes`, then, once that one ended, the child case `late.digest`, and is
/// delivered once both passed. `notes.write` sends one call through the run's relay, leaves it open in a process of its
/// own (the call is in flight when its step ends) and exits 3, an exit its bindings take as `written`, so its case
/// passes and the tree goes on. `digest.write` waits until that call has its answer and exits 0 with `digest.md`.
fn late_tree_runbook(h: &Home) {
    let root = "format: protocol/1\nprotocol:\n  id: late-tree\n  revision: 1\n  description: >-\n    A child case whose step leaves a call in flight and exits 3, then a child case whose step exits 0.\nartifacts:\n  idea: {}\nevidence_kinds:\n  case_outcome: {}\n  notes_opened: {}\n  digest_opened: {}\nclaims:\n  notes.opened:\n    true_when:\n      evidence: {kind: notes_opened, subject: idea, result: opened}\n  digest.opened:\n    true_when:\n      evidence: {kind: digest_opened, subject: idea, result: opened}\n  children.passed:\n    true_when:\n      children: {in: [passed]}\nactions:\n  notes.open:\n    effect: write\n    precondition: {claim: notes.opened, is: unknown}\n    case_open: {protocol: late.notes, subject: idea}\n    may_produce:\n      - evidence: notes_opened\n  digest.open:\n    effect: write\n    precondition:\n      all:\n        - {claim: notes.opened}\n        - {claim: digest.opened, is: unknown}\n    case_open: {protocol: late.digest, subject: idea}\n    may_produce:\n      - evidence: digest_opened\noutcomes:\n  delivered:\n    requires:\n      all:\n        - {claim: notes.opened}\n        - {claim: digest.opened}\n        - {claim: children.passed}\n";
    let root_bindings = "format: loom.bindings/1\nprotocol: late-tree@1\nprogram_root: bindings\nartifacts:\n  idea: idea.yaml\nactions:\n  - action: notes.open\n    executor:\n      kind: open\n      name: notes\n      bindings: late-notes.yaml\n    evidence:\n      - kind: notes_opened\n        subject: idea\n        producer: loom\n        from: effect\n        results: {performed: opened}\n  - action: digest.open\n    executor:\n      kind: open\n      name: digest\n      bindings: late-digest.yaml\n    evidence:\n      - kind: digest_opened\n        subject: idea\n        producer: loom\n        from: effect\n        results: {performed: opened}\n";
    std::fs::write(h.home.join("protocol/late-tree.protocol.yaml"), root).unwrap();
    std::fs::write(h.home.join("bindings/late-tree.yaml"), root_bindings).unwrap();
    late_child(h, "notes");
    late_child(h, "digest");
    let head = format!(
        "#!/bin/bash\nf=$RUNBOOK_ROUTE_FILE\nbase=$(jq -r .base_url \"$f\") && token=$(jq -r .token \"$f\") || exit 9\nmodel=default\n{}",
        super::adversary_w4::call_function(&h.log)
    );
    let log = h.log.display();
    write_exe(
        &h.home.join("bindings/tools/late-notes.sh"),
        &format!("{head}( call ''; : > '{log}/late-answered' ) </dev/null >/dev/null 2>&1 &\nsleep 0.3\nprintf 'notes\\n' > notes.md\nexit 3\n"),
    );
    write_exe(
        &h.home.join("bindings/tools/late-digest.sh"),
        &format!("#!/bin/bash\n: > '{log}/digest-started'\nfor i in $(seq 1 200); do [ -e '{log}/late-answered' ] && break; sleep 0.05; done\nprintf 'digest\\n' > digest.md\nexit 0\n"),
    );
}

/// Story `a-call-booked-after-its-step-ended-does-not-fail-the-run` (a regression of
/// `a-recovered-call-does-not-fail-the-run`). The call `notes.write` left in flight is answered 502 (`upstream-failed`)
/// while `digest.write` runs in the next child case, so the relay books it after its own step ended; `notes.write` went on (exit 3,
/// taken as `written`) and `digest.write` exits 0 with its product. The run follows its protocol to `delivered`, its
/// record names the late call once under `notes.write` with one event `runbook.model-calls-failed`, and it carries no
/// `runbook.runtime-failed`.
#[test]
fn a_failed_call_booked_after_its_step_ended_does_not_fail_the_run_whose_steps_went_on() {
    let n = Arc::new(AtomicU64::new(0));
    let started = Arc::new(std::sync::Mutex::new(None::<std::path::PathBuf>));
    let seen = started.clone();
    let gw = gateway_answering(
        move |_| {
            if n.fetch_add(1, Ordering::SeqCst) > 0 {
                return OK.to_vec();
            }
            // the call of `notes.write` is held until the child's step has started (bounded under the test's 60 s)
            let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
            while std::time::Instant::now() < until && !seen.lock().unwrap().as_ref().is_some_and(|f: &std::path::PathBuf| f.exists()) {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            let body = json!({"error": {"code": "upstream-failed", "message": "the upstream gave no answer: timeout"}}).to_string();
            answer("502 Bad Gateway", "gw-call-late", &body)
        },
        &["a-model"],
    );
    let (h, l, _roots) = daemon_at("rc-late-tree", &gw.base_url, &format!("{}{ROUTE}", model_table(Some(&claude_cli()))));
    *started.lock().unwrap() = Some(h.log.join("digest-started"));
    late_tree_runbook(&h);
    let r = send(l.addr, "POST", "/daemon/commands/submit-run", &[("Content-Type", "application/json")], Some(json!({"runbook_id": "late-tree", "intake": "red or blue", "model_route": "default"}).to_string().as_bytes()));
    assert_eq!(r.status, 202, "{}", r.text());
    let run = r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned();
    let end = until_state(&l, &run, &["Ended", "FailedToStart", "Cancelled"]);
    // the case's precondition: the late call's line comes after the start of the child's step
    let all: Vec<(Value, Value, Value)> = rows(&l, &format!("/daemon/views/events?run_id={run}&after_position=0")).into_iter().map(|e| (e["kind"].clone(), e["record"]["action"].clone(), e["record"]["gateway_call"].clone())).collect();
    let digest_started = all.iter().position(|(k, a, _)| k == "runbook.action-started" && a == "digest.write");
    let late = all.iter().position(|(k, _, c)| k == "runbook.model-call" && c == "gw-call-late");
    assert!(matches!((digest_started, late), (Some(d), Some(c)) if d < c), "setup: the call of notes.write was not booked after its step ended: {all:?} {end} {:?}", events(&l, &run, "runbook.start-failed"));
    let failed: Vec<Value> = events(&l, &run, "runbook.runtime-failed").into_iter().map(|e| e["reason"].clone()).collect();
    assert!(failed.is_empty(), "a call booked after its step ended failed the run: {failed:?} {end}");
    assert_eq!((end["state"].clone(), end["outcome"].clone()), (json!("Ended"), json!("delivered")), "the run did not follow its protocol to its outcome: {end}");
    let noted: Vec<(Value, Vec<Value>)> = events(&l, &run, "runbook.model-calls-failed")
        .iter()
        .map(|e| (e["step"].clone(), e["calls"].as_array().into_iter().flatten().map(|c| c["gateway_call"].clone()).collect()))
        .collect();
    assert_eq!(noted, vec![(json!("notes.write"), vec![json!("gw-call-late")])], "the run's record does not name the late call once under its step");
}

/// Story `a-call-booked-after-its-step-ended-does-not-fail-the-run`, the consumer's gap 4: a step whose program exits 2,
/// an exit its bindings take as a result (`written`; 2 is not in its `exits.error`), is no failed step. Its one call is
/// answered 502 (`upstream-failed`) and its program then exits 2 with its product: Loom took the result and the
/// protocol goes on to `passed`. The run's record names the call under the step with one `runbook.model-calls-failed`
/// and holds no `runbook.runtime-failed`. (An exit its `exits.error` lists still fails the run with the call as its
/// first cause: `bound_step::a_failed_call_of_a_bound_program_whose_step_exits_1_is_the_first_cause_of_the_runs_failure`.)
#[test]
fn a_failed_call_of_a_step_whose_exit_the_bindings_take_as_a_result_does_not_fail_the_run() {
    let gw = gateway_answering(|_| answer("502 Bad Gateway", "gw-call-mapped", &json!({"error": {"code": "upstream-failed", "message": "the upstream gave no answer: timeout"}}).to_string()), &["a-model"]);
    let (h, l, _roots) = daemon_at("rc-mapped-exit", &gw.base_url, &format!("{}{ROUTE}", model_table(Some(&claude_cli()))));
    late_child(&h, "gap");
    let head = format!(
        "#!/bin/bash\nf=$RUNBOOK_ROUTE_FILE\nbase=$(jq -r .base_url \"$f\") && token=$(jq -r .token \"$f\") || exit 9\nmodel=default\n{}",
        super::adversary_w4::call_function(&h.log)
    );
    write_exe(&h.home.join("bindings/tools/late-gap.sh"), &format!("{head}call ''\nprintf 'gap\\n' > gap.md\nexit 2\n"));
    let r = send(l.addr, "POST", "/daemon/commands/submit-run", &[("Content-Type", "application/json")], Some(json!({"runbook_id": "late-gap", "intake": "red or blue", "model_route": "default"}).to_string().as_bytes()));
    assert_eq!(r.status, 202, "{}", r.text());
    let run = r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned();
    let end = until_state(&l, &run, &["Ended", "FailedToStart", "Cancelled"]);
    let calls: Vec<Value> = events(&l, &run, "runbook.model-call").iter().map(|c| c["gateway_call"].clone()).collect();
    assert_eq!(calls, vec![json!("gw-call-mapped")], "setup: the step's one call was not booked: {end}");
    let failed: Vec<Value> = events(&l, &run, "runbook.runtime-failed").into_iter().map(|e| e["reason"].clone()).collect();
    assert!(failed.is_empty(), "a failed call of a step whose exit the bindings take as a result failed the run: {failed:?} {end}");
    assert_eq!((end["state"].clone(), end["outcome"].clone()), (json!("Ended"), json!("passed")), "the run did not follow its protocol to its outcome: {end}");
    let noted: Vec<(Value, Vec<Value>)> = events(&l, &run, "runbook.model-calls-failed")
        .iter()
        .map(|e| (e["step"].clone(), e["calls"].as_array().into_iter().flatten().map(|c| c["gateway_call"].clone()).collect()))
        .collect();
    assert_eq!(noted, vec![(json!("gap.write"), vec![json!("gw-call-mapped")])], "the run's record does not name the call under its step");
}

/// The gateway's spend ceiling (`spend-ceiling` 503) still fails the step at once, though the program went on and
/// exited 0.
#[test]
fn the_gateways_spend_ceiling_still_fails_the_step_route_unavailable() {
    let gw = first_refused(spend_ceiling);
    let (_h, l) = daemon("rc-ceiling", &gw, 2);
    let run = delegated(&l);
    let end = until_ended(&l, &run);
    let failure = end["failure"].as_str().unwrap_or_default();
    assert_ne!(end["outcome"], json!("delivered"), "{end}");
    assert!(failure.starts_with("route-unavailable: route default") && failure.contains("spend-ceiling"), "{end}");
}
