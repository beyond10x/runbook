//! Adversary cases of wave 2 of the model routes (the consumer's four gaps): which runs take a route, and the outcome
//! `failed: step <action> exit <n>`.
//!
//! No model is called and nothing leaves loopback: the gateways are stand-ins on ports of the test's own, and the
//! agent node's `claude` is absent.

use super::default_confinement::{post, rows};
use super::delegate::{daemon_at, until_ended, ROUTE};
use super::gateway::{claude_cli, gateway_table, home, model_table, serve, until_state, write_exe};
use super::http::send;
use super::relay::{gateway_listing, got};
use serde_json::json;

const OK: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nx-llm-gateway-call: gw-call-9\r\nContent-Length: 56\r\nConnection: close\r\n\r\n{\"id\":\"m\",\"usage\":{\"input_tokens\":11,\"output_tokens\":5}}";

// ---- point 3: an agent step that fails names the step and its exit ----

/// Point 3 of the wave: "A delegated run whose agent step fails ... must end with an outcome that names the failed
/// step and its exit, and `delegation-result`'s `failure` (and the MCP tool error) carry it." The ESS summary of
/// `runbook.step-failed` and `docs/daemon-api.md` say the same for "a program of the run [that] ended with an exit
/// other than 0". The agent node of a free delegation fails when the daemon's `PATH` has no `claude`: the claude-cli
/// adapter exits 127 ("claude is not on PATH"), `sdc-node` passes that exit on, and the tree has nothing else to do.
/// That failed agent step must end the run naming `delegation.free` and its exit, not `runtime-failure` with a stuck
/// tree.
#[test]
fn adversary_wave2_a_delegation_whose_agent_node_finds_no_claude_ends_naming_the_step_and_its_exit() {
    let gw = gateway_listing(OK, &["a-model", "b-model"]);
    let (h, l, _) = daemon_at("adv2-noclaude", &gw.base_url, &format!("{}{ROUTE}", model_table(Some(&claude_cli()))));
    // the daemon's PATH holds no `claude` now: the stand-in goes, and none is in the system directories of that PATH
    std::fs::remove_file(h.log.join("bin/claude")).unwrap();
    for dir in ["/usr/local/bin", "/usr/bin", "/bin"] {
        assert!(!std::path::Path::new(dir).join("claude").exists(), "{dir} holds a claude; this case needs none on the daemon's PATH");
    }
    let r = send(l.addr, "POST", "/daemon/commands/delegate", &[("Content-Type", "application/json")], Some(json!({"request": "Write one sentence about tea.", "model_route": "default"}).to_string().as_bytes()));
    assert_eq!(r.status, 202, "{}", r.text());
    let run = r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned();
    let end = until_ended(&l, &run);
    let outcome = end["outcome"].as_str().unwrap_or_default();
    let failure = end["failure"].as_str().unwrap_or_default();
    assert_eq!(end["run_state"], "Ended", "{end}");
    assert!(
        outcome.starts_with("failed: step delegation.free exit ") && outcome != "failed: step delegation.free exit 0",
        "the agent step failed and the outcome does not name it and its exit: outcome {outcome:?}, failure {failure:?}"
    );
    let exit = outcome.rsplit(' ').next().unwrap_or_default();
    assert!(failure.contains("delegation.free") && failure.contains(&format!("exit {exit}")) && !failure.contains("is stuck"), "{end}");
}

// ---- point 2: the interim route of a lone `[gateway]` ----

/// `docs/daemon-api.md`, "The route of a run", as this wave wrote it: "The interim route `default` of a lone
/// `[gateway]` goes to a run whose decider is `model` above `A0` only." (and the CHANGELOG: "the interim route of a lone
/// `[gateway]` goes to a model decider only"). A run whose decider is a rule and whose runbook runs programs, on a
/// daemon with only a `[gateway]` and a model decider, names that route: it must not take it (refused, or no route).
#[test]
fn adversary_wave2_a_rule_run_that_names_the_interim_route_of_a_lone_gateway_does_not_take_it() {
    let h = home("adv2-interim");
    let gw = gateway_listing(OK, &["a-model"]);
    let file = format!("{}{}[deciders.rule]\nverdict = \"approve\"\n", gateway_table(&gw.base_url, &h.token_file), model_table(Some(&claude_cli())));
    let l = serve("adv2-interim-d", &h, &file, true);
    let (status, body) = post(&l, &json!({"runbook_id": "autonomy-demo", "intake": "a note on tea", "confinement": "local", "autonomy": "A5", "decider": "rule", "model_route": "default"}));
    if status == 202 {
        let run = body["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned();
        until_state(&l, &run, &["Ended", "FailedToStart", "Cancelled"]);
        let row = rows(&l, &format!("/daemon/views/run?run_id={run}"))[0].clone();
        assert!(
            row.get("model_route").is_none(),
            "a run whose decider is a rule took the interim route of the lone [gateway], which goes to a model decider only: {row}"
        );
    } else {
        assert_eq!((status, body["outcome"].clone()), (422, json!("route-refused")), "{body}");
    }
    assert!(got(&gw).iter().all(|g| !g.head.starts_with("POST ")), "a rule run's call reached the lone gateway");
}

// ---- point 3: a step whose program is killed at its timeout ----

/// Point 3 again, for "a submitted run whose only action fails": the only action's program hangs and is killed at the
/// binding's `timeout_seconds` (the free delegation's agent node has one, 3600 s). The step failed and the tree has
/// nothing else to do: the run must end with an outcome that names the step, not `runtime-failure` with "the tree is
/// stuck".
#[test]
fn adversary_wave2_a_run_whose_only_step_is_killed_at_its_timeout_ends_naming_the_step() {
    let gw = gateway_listing(OK, &["a-model", "b-model"]);
    let (h, l, _) = daemon_at("adv2-timeout", &gw.base_url, &format!("{}{ROUTE}", model_table(Some(&claude_cli()))));
    let protocol = "format: protocol/1\nprotocol:\n  id: hang-demo\n  revision: 1\n  description: >-\n    An adversary case's runbook: one program that hangs past its timeout.\nartifacts:\n  idea: {}\n  result: {}\nevidence_kinds:\n  result_written: {}\nclaims:\n  result.written:\n    true_when:\n      evidence: {kind: result_written, subject: result, result: written}\nactions:\n  hang.write:\n    effect: write\n    may_produce:\n      - evidence: result_written\noutcomes:\n  answered:\n    requires: {claim: result.written}\n";
    let bindings = "format: loom.bindings/1\nprotocol: hang-demo@1\nprogram_root: bindings\nartifacts:\n  idea: idea.yaml\n  result: result.md\nactions:\n  - action: hang.write\n    executor:\n      kind: program\n      argv: [tools/hang.sh, --max-budget-usd, \"0.01\"]\n    timeout_seconds: 1\n    evidence:\n      - kind: result_written\n        subject: result\n        producer: hang\n        from: exit\n        results: {\"0\": written, timeout: failed, \"*\": failed}\n";
    std::fs::write(h.home.join("protocol/hang-demo.protocol.yaml"), protocol).unwrap();
    std::fs::write(h.home.join("bindings/hang-demo.yaml"), bindings).unwrap();
    write_exe(&h.home.join("bindings/tools/hang.sh"), "#!/bin/bash\nexec sleep 30\n");
    let r = send(l.addr, "POST", "/daemon/commands/submit-run", &[("Content-Type", "application/json")], Some(json!({"runbook_id": "hang-demo", "intake": "red or blue"}).to_string().as_bytes()));
    assert_eq!(r.status, 202, "{}", r.text());
    let run = r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned();
    let end = until_state(&l, &run, &["Ended", "FailedToStart", "Cancelled"]);
    let events = rows(&l, &format!("/daemon/views/events?run_id={run}&after_position=0"));
    let why: Vec<String> = events.iter().filter(|e| e["kind"] == "runbook.runtime-failed" || e["kind"] == "runbook.step-failed").map(|e| e["record"]["reason"].as_str().unwrap_or_default().to_owned()).collect();
    let outcome = end["outcome"].as_str().unwrap_or_default();
    assert!(
        end["state"] == "Ended" && outcome.starts_with("failed: step hang.write") && why.iter().all(|w| !w.contains("is stuck")),
        "the only step was killed at its timeout and the run does not end naming it: outcome {outcome:?}, reasons {why:?}"
    );
}

