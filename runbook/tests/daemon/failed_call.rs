//! Story `a-failed-call-names-the-adapter-error`: a run on a route whose model call left no cost record ends naming the
//! call's adapter, its exit and the client's own error in words; a value the call's record does not hold is "not
//! recorded", never `?`.
//!
//! No model is called and nothing leaves loopback: the gateway is a stand-in on a port of the test's own, and the bound
//! program writes the record a model program leaves when its client refused the call before it sent it.

use super::default_confinement::rows;
use super::delegate::{daemon_at, ROUTE};
use super::gateway::{claude_cli, model_table, until_state, write_exe};
use super::http::send;
use super::relay::gateway_answering;
use serde_json::{json, Value};

const OK: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 56\r\nConnection: close\r\n\r\n{\"id\":\"m\",\"usage\":{\"input_tokens\":11,\"output_tokens\":5}}";

/// A runbook of one bound program, `proposals.write`: it writes the call directory of a model call whose client refused
/// the model it asked for before it sent anything (`request.json` naming the adapter, an empty `response.json`, the
/// client's error on `stderr.txt`), writes no cost line and exits 1.
fn refused_model_runbook(home: &std::path::Path) {
    let protocol = "format: protocol/1\nprotocol:\n  id: refused-demo\n  revision: 1\n  description: >-\n    A failed-call case's runbook: one model program whose client refused the model it asked for.\nartifacts:\n  idea: {}\n  result: {}\nevidence_kinds:\n  result_written: {}\nclaims:\n  result.written:\n    true_when:\n      evidence: {kind: result_written, subject: result, result: written}\nactions:\n  proposals.write:\n    effect: write\n    may_produce:\n      - evidence: result_written\noutcomes:\n  answered:\n    requires: {claim: result.written}\n";
    let bindings = "format: loom.bindings/1\nprotocol: refused-demo@1\nprogram_root: bindings\nartifacts:\n  idea: idea.yaml\n  result: result.md\nactions:\n  - action: proposals.write\n    executor:\n      kind: program\n      argv: [tools/refused.sh, --max-budget-usd, \"0.5\"]\n    timeout_seconds: 20\n    evidence:\n      - kind: result_written\n        subject: result\n        producer: refused\n        from: exit\n        results: {\"0\": written, timeout: failed, \"*\": failed}\n";
    std::fs::write(home.join("protocol/refused-demo.protocol.yaml"), protocol).unwrap();
    std::fs::write(home.join("bindings/refused-demo.yaml"), bindings).unwrap();
    let program = r#"#!/bin/sh
d=.sdc/llm/001-proposals.write
mkdir -p "$d" || exit 9
printf '%s\n' '{"action":"proposals.write","unit":"candidates","adapter":"claude-cli","model":"default","route":"relay"}' > "$d/request.json"
: > "$d/response.json"
printf '%s\n' '[claude-code:unrecognized_model] {"model":"default[1m]","query_source":"sdk"}' > "$d/stderr.txt"
exit 1
"#;
    write_exe(&home.join("bindings/tools/refused.sh"), program);
}

/// The run on the route `default` (its route file holds the one role `default`, model `runbook-default`) ends
/// `runtime-failure`, and its reason names the adapter of the call's `request.json`, says the exit is not recorded (no
/// `response.json` holds it), and says Claude Code's `unrecognized_model` in words: the model it asked for and the roles
/// of the run's route file.
#[test]
fn failed_call_a_run_on_a_route_whose_call_left_no_cost_record_names_adapter_exit_and_the_clients_error() {
    let gw = gateway_answering(|_| OK.to_vec(), &["a-model"]);
    let (h, l, _) = daemon_at("failed-call", &gw.base_url, &format!("{}{ROUTE}", model_table(Some(&claude_cli()))));
    refused_model_runbook(&h.home);
    let r = send(l.addr, "POST", "/daemon/commands/submit-run", &[("Content-Type", "application/json")], Some(json!({"runbook_id": "refused-demo", "intake": "red or blue", "model_route": "default"}).to_string().as_bytes()));
    assert_eq!(r.status, 202, "{}", r.text());
    let run = r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned();
    let end = until_state(&l, &run, &["Ended", "FailedToStart", "Cancelled"]);
    let events = rows(&l, &format!("/daemon/views/events?run_id={run}&after_position=0"));
    let reasons: Vec<Value> = events.iter().filter(|e| e["kind"] == "runbook.runtime-failed").map(|e| e["record"]["reason"].clone()).collect();
    assert_eq!((end["model_route"].clone(), end["outcome"].clone()), (json!("default"), json!("runtime-failure")), "{end} {reasons:?}");
    assert_eq!(
        reasons,
        vec![json!(
            "model step proposals.write: model call 001-proposals.write has no cost record (adapter claude-cli, exit not recorded): Claude Code does not know the model it asked for, \"default[1m]\"; the roles of the run's route file: default (model runbook-default) [claude-code:unrecognized_model]"
        )],
        "the failure does not name the adapter, the exit and the client's error in words"
    );
}
