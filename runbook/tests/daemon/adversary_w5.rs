//! Adversary cases of the wave "bound calls" (story `failed-calls-of-a-bound-program-belong-to-its-step`):
//! `docs/engine.md` (the step in flight: "the same action is the step the relay books each model call of the program
//! under"), `docs/daemon-api.md` (`runbook.action-started`), `docs/model-routes.md` section 4.
//!
//! A run with a decider (a level above A0 and a decider kind) opens its tree with the decider's own copy of the
//! bindings, in which the argv of every agent node (`sdc-node`) carries `--tools-dir <the copy's tools>`. The step of
//! such a program is its action like any other's: `runbook.action-started` names it, the relay books the program's
//! calls under it, and a failed call of a failed step is the run's first cause.
//!
//! No model is called and nothing leaves loopback: the gateway is a stand-in on a port of the test's own that answers
//! every call 502 (`upstream-failed`), and the bound program (a stand-in `sdc-node` of the runbook's tools) calls the
//! run's relay through its route file.

use super::default_confinement::{rows, Live};
use super::delegate::{daemon_at, ROUTE};
use super::gateway::{claude_cli, model_table, until_state, write_exe, Home};
use super::http::send;
use super::relay::{gateway_answering, Gateway};
use serde_json::{json, Value};

/// The rule decider of the daemon: it approves; the runbook below asks it nothing.
const RULE: &str = "[deciders.rule]\nverdict = \"approve\"\n";

/// The gateway's answer to a call whose upstream timed out: `upstream-failed` 502, call id `gw-call-502`.
fn upstream_failed() -> Vec<u8> {
    let body = json!({"error": {"code": "upstream-failed", "message": "the upstream gave no answer: timeout"}}).to_string();
    format!("HTTP/1.1 502 Bad Gateway\r\nContent-Type: application/json\r\nx-llm-gateway-call: gw-call-502\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).into_bytes()
}

/// The runbook `node-demo` of one step `node.write`, an agent node: its program is the runbook's `tools/sdc-node`.
fn runbook(h: &Home) {
    let protocol = "format: protocol/1\nprotocol:\n  id: node-demo\n  revision: 1\n  description: >-\n    One step, an agent node that calls the model.\nartifacts:\n  idea: {}\n  answer: {}\nevidence_kinds:\n  answer_written: {}\nclaims:\n  answer.written:\n    true_when:\n      evidence: {kind: answer_written, subject: answer, result: written}\nactions:\n  node.write:\n    effect: write\n    may_produce:\n      - evidence: answer_written\noutcomes:\n  delivered:\n    requires: {claim: answer.written}\n";
    let bindings = "format: loom.bindings/1\nprotocol: node-demo@1\nprogram_root: bindings\nartifacts:\n  idea: idea.yaml\n  answer: answer.md\nactions:\n  - action: node.write\n    executor:\n      kind: program\n      argv: [tools/sdc-node, node.write, --max-budget-usd, \"0.5\"]\n    exits: {error: [1, 75, 126, 127]}\n    timeout_seconds: 20\n    evidence:\n      - kind: answer_written\n        subject: answer\n        producer: node-demo\n        from: exit\n        results: {\"0\": written, timeout: failed, \"*\": failed}\n";
    std::fs::write(h.home.join("protocol/node-demo.protocol.yaml"), protocol).unwrap();
    std::fs::write(h.home.join("bindings/node-demo.yaml"), bindings).unwrap();
}

/// A daemon with the rule decider whose home holds `node-demo`; its `tools/sdc-node` sends one Messages call to the
/// run's relay, writes `answer.md` and exits 1.
fn daemon(tag: &str, gw: &Gateway) -> Live {
    let (h, l, _roots) = daemon_at(tag, &gw.base_url, &format!("{}{ROUTE}{RULE}", model_table(Some(&claude_cli()))));
    runbook(&h);
    let program = r#"#!/bin/bash
f=$RUNBOOK_ROUTE_FILE
base=$(jq -r .base_url "$f") && token=$(jq -r .token "$f") || exit 9
body='{"model":"default","max_tokens":8,"messages":[{"role":"user","content":"hi"}]}'
rest=${base#http://}
hostport=${rest%%/*}
path=''
[ "$rest" = "$hostport" ] || path=/${rest#*/}
exec 3<>"/dev/tcp/${hostport%:*}/${hostport##*:}" || exit 7
printf 'POST %s/v1/messages HTTP/1.0\r\nHost: %s\r\nAuthorization: Bearer %s\r\nContent-Type: application/json\r\nContent-Length: %s\r\n\r\n%s' "$path" "$hostport" "$token" "${#body}" "$body" >&3
cat <&3 > /dev/null
exec 3<&-
printf 'an answer\n' > answer.md
exit 1
"#;
    write_exe(&h.home.join("bindings/tools/sdc-node"), program);
    l
}

fn events(l: &Live, run: &str, kind: &str) -> Vec<Value> {
    rows(l, &format!("/daemon/views/events?run_id={run}&after_position=0")).into_iter().filter(|e| e["kind"] == kind).map(|e| e["record"].clone()).collect()
}

/// The agent node `node.write` of a run with the rule decider at A5: its one call is answered 502 and its program
/// exits 1. Its start names `node.write`, the call is booked under `node.write`, and the run fails with that call as
/// its first cause.
#[test]
fn w5_the_agent_node_of_a_run_with_a_decider_is_named_and_its_failed_call_is_its_steps_first_cause() {
    let gw = gateway_answering(|_| upstream_failed(), &["a-model"]);
    let l = daemon("w5-decider", &gw);
    let body = json!({"runbook_id": "node-demo", "intake": "red or blue", "model_route": "default", "confinement": "local", "autonomy": "A5", "decider": "rule"}).to_string();
    let r = send(l.addr, "POST", "/daemon/commands/submit-run", &[("Content-Type", "application/json")], Some(body.as_bytes()));
    assert_eq!(r.status, 202, "{}", r.text());
    let run = r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned();
    let end = until_state(&l, &run, &["Ended", "FailedToStart", "Cancelled"]);
    let started: Vec<Value> = events(&l, &run, "runbook.action-started").into_iter().map(|e| e["action"].clone()).collect();
    let steps: Vec<Value> = events(&l, &run, "runbook.model-call").iter().map(|c| c["step"].clone()).collect();
    let reasons: Vec<Value> = events(&l, &run, "runbook.runtime-failed").into_iter().map(|e| e["reason"].clone()).collect();
    assert_eq!(
        (started, steps),
        (vec![json!("node.write")], vec![json!("node.write")]),
        "the agent node's start and its call do not name its action; failure: {reasons:?}; end: {end}"
    );
    let first = reasons.first().and_then(Value::as_str).unwrap_or_default();
    assert!(first.starts_with("route-unavailable: route default (chosen by run) failed the call: upstream-failed"), "the failed step's call is not the run's first cause: {reasons:?} {end}");
}
