//! The model calls of a bound program belong to its step (story `failed-calls-of-a-bound-program-belong-to-its-step`,
//! `docs/model-routes.md` section 4): a call the relay books for a program a step runs carries the step's action, the
//! name the step's failure and `runbook.action-started` use, also when another runbook of the daemon's home binds the
//! same program with the same arguments. So a step whose program exits non-zero ends the run with its own first failed
//! call as the run's cause, and the run's record lists the calls of a step that went on under its action, never a
//! case id.
//!
//! No model is called and nothing leaves loopback: the gateway is a stand-in on a port of the test's own that answers
//! every call 502 (`upstream-failed`), and the bound program calls the run's relay through its route file.

use super::default_confinement::{rows, Live};
use super::delegate::{daemon_at, ROUTE};
use super::gateway::{claude_cli, model_table, until_state, write_exe, Home};
use super::http::send;
use super::relay::{gateway_answering, Gateway};
use serde_json::{json, Value};

/// The program both runbooks bind, with the same arguments.
const SHARED: &str = "[tools/shared-call.sh, --max-budget-usd, \"0.5\"]";

/// The gateway's answer to a call whose upstream timed out: `upstream-failed` 502, call id `gw-call-502`.
fn upstream_failed() -> Vec<u8> {
    let body = json!({"error": {"code": "upstream-failed", "message": "the upstream gave no answer: timeout"}}).to_string();
    format!("HTTP/1.1 502 Bad Gateway\r\nContent-Type: application/json\r\nx-llm-gateway-call: gw-call-502\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).into_bytes()
}

/// A runbook `<id>` of one step `action`, whose program is [`SHARED`]; it is delivered when the program exits 0.
fn runbook(h: &Home, id: &str, action: &str) {
    let protocol = format!(
        "format: protocol/1\nprotocol:\n  id: {id}\n  revision: 1\n  description: >-\n    One step whose bound program calls the model.\nartifacts:\n  idea: {{}}\n  answer: {{}}\nevidence_kinds:\n  answer_written: {{}}\nclaims:\n  answer.written:\n    true_when:\n      evidence: {{kind: answer_written, subject: answer, result: written}}\nactions:\n  {action}:\n    effect: write\n    may_produce:\n      - evidence: answer_written\noutcomes:\n  delivered:\n    requires: {{claim: answer.written}}\n"
    );
    let bindings = format!(
        "format: loom.bindings/1\nprotocol: {id}@1\nprogram_root: bindings\nartifacts:\n  idea: idea.yaml\n  answer: answer.md\nactions:\n  - action: {action}\n    executor:\n      kind: program\n      argv: {SHARED}\n    exits: {{error: [1, 75, 126, 127]}}\n    timeout_seconds: 20\n    evidence:\n      - kind: answer_written\n        subject: answer\n        producer: {id}\n        from: exit\n        results: {{\"0\": written, timeout: failed, \"*\": failed}}\n"
    );
    std::fs::write(h.home.join(format!("protocol/{id}.protocol.yaml")), protocol).unwrap();
    std::fs::write(h.home.join(format!("bindings/{id}.yaml")), bindings).unwrap();
}

/// A daemon whose home holds the runbooks `lib-demo` (step `lib.call`) and `strict-demo` (step `strict.call`), both
/// binding [`SHARED`]: one Messages call to the run's relay, then `answer.md` and exit `exit`.
fn daemon(tag: &str, gw: &Gateway, exit: u8) -> Live {
    let (h, l, _roots) = daemon_at(tag, &gw.base_url, &format!("{}{ROUTE}", model_table(Some(&claude_cli()))));
    runbook(&h, "lib-demo", "lib.call");
    runbook(&h, "strict-demo", "strict.call");
    let program = format!(
        r#"#!/bin/bash
f=$RUNBOOK_ROUTE_FILE
base=$(jq -r .base_url "$f") && token=$(jq -r .token "$f") || exit 9
body='{{"model":"default","max_tokens":8,"messages":[{{"role":"user","content":"hi"}}]}}'
rest=${{base#http://}}
hostport=${{rest%%/*}}
path=''
[ "$rest" = "$hostport" ] || path=/${{rest#*/}}
exec 3<>"/dev/tcp/${{hostport%:*}}/${{hostport##*:}}" || exit 7
printf 'POST %s/v1/messages HTTP/1.0\r\nHost: %s\r\nAuthorization: Bearer %s\r\nContent-Type: application/json\r\nContent-Length: %s\r\n\r\n%s' "$path" "$hostport" "$token" "${{#body}}" "$body" >&3
cat <&3 > /dev/null
exec 3<&-
printf 'an answer\n' > answer.md
exit {exit}
"#
    );
    write_exe(&h.home.join("bindings/tools/shared-call.sh"), &program);
    l
}

fn submitted(l: &Live, runbook: &str) -> String {
    let body = json!({"runbook_id": runbook, "intake": "red or blue", "model_route": "default"}).to_string();
    let r = send(l.addr, "POST", "/daemon/commands/submit-run", &[("Content-Type", "application/json")], Some(body.as_bytes()));
    assert_eq!(r.status, 202, "{}", r.text());
    r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned()
}

fn events(l: &Live, run: &str, kind: &str) -> Vec<Value> {
    rows(l, &format!("/daemon/views/events?run_id={run}&after_position=0")).into_iter().filter(|e| e["kind"] == kind).map(|e| e["record"].clone()).collect()
}

/// The step `strict.call` runs the program another runbook binds too; its one call is answered 502 and its program
/// exits 1. The call is booked under `strict.call` (the action `runbook.action-started` names), and the run fails with
/// that call as its first cause, not as `failed: step strict.call exit 1` with the call noted as one a step went on
/// from.
#[test]
fn a_failed_call_of_a_bound_program_whose_step_exits_1_is_the_first_cause_of_the_runs_failure() {
    let gw = gateway_answering(|_| upstream_failed(), &["a-model"]);
    let l = daemon("bs-exit", &gw, 1);
    let run = submitted(&l, "strict-demo");
    let end = until_state(&l, &run, &["Ended", "FailedToStart", "Cancelled"]);
    let started: Vec<Value> = events(&l, &run, "runbook.action-started").into_iter().map(|e| e["action"].clone()).collect();
    assert_eq!(started, vec![json!("strict.call")], "the step's start does not name its action: {started:?}");
    let steps: Vec<(Value, Value)> = events(&l, &run, "runbook.model-call").iter().map(|c| (c["step"].clone(), c["gateway_call"].clone())).collect();
    assert_eq!(steps, vec![(json!("strict.call"), json!("gw-call-502"))], "the call is not booked under the step's action: {end}");
    let noted = events(&l, &run, "runbook.model-calls-failed");
    assert!(noted.is_empty(), "the failed step's own call was noted as one its step went on from: {noted:?}");
    let reasons: Vec<Value> = events(&l, &run, "runbook.runtime-failed").into_iter().map(|e| e["reason"].clone()).collect();
    let first = reasons.first().and_then(Value::as_str).unwrap_or_default();
    assert!(first.starts_with("route-unavailable: route default (chosen by run) failed the call: upstream-failed"), "the failed step's call is not the run's first cause: {reasons:?} {end}");
}

/// The same step and call, its program exits 0 with `answer.md`: the run ends delivered, and its record names the
/// failed call under `strict.call`.
#[test]
fn a_failed_call_of_a_bound_program_whose_step_exits_0_is_named_under_the_steps_action() {
    let gw = gateway_answering(|_| upstream_failed(), &["a-model"]);
    let l = daemon("bs-zero", &gw, 0);
    let run = submitted(&l, "strict-demo");
    let end = until_state(&l, &run, &["Ended", "FailedToStart", "Cancelled"]);
    assert_eq!((end["state"].clone(), end["outcome"].clone()), (json!("Ended"), json!("delivered")), "{end}");
    let steps: Vec<Value> = events(&l, &run, "runbook.model-call").iter().map(|c| c["step"].clone()).collect();
    assert_eq!(steps, vec![json!("strict.call")], "the call is not booked under the step's action: {end}");
    let noted: Vec<(Value, Vec<Value>)> = events(&l, &run, "runbook.model-calls-failed")
        .iter()
        .map(|e| (e["step"].clone(), e["calls"].as_array().into_iter().flatten().map(|c| c["step"].clone()).collect()))
        .collect();
    assert_eq!(noted, vec![(json!("strict.call"), vec![json!("strict.call")])], "the run's record does not name the failed call under the step's action");
}
