//! Adversary cases of story `a-call-booked-after-its-step-ended-does-not-fail-the-run` (and the story it is a regression
//! of, `a-recovered-call-does-not-fail-the-run`): `docs/engine.md` "A gateway", `docs/model-routes.md` section 4.
//!
//! Each case drives the daemon over loopback only: the gateway is a stand-in on a port of the test's own, and the bound
//! programs of the case's own runbook call the run's relay through its route file. No model is called.

use super::default_confinement::{rows, Live};
use super::delegate::{daemon_at, ROUTE};
use super::gateway::{claude_cli, model_table, until_state, write_exe, Home};
use super::http::send;
use super::relay::gateway_answering;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const OK: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nx-llm-gateway-call: gw-call-ok\r\nContent-Length: 56\r\nConnection: close\r\n\r\n{\"id\":\"m\",\"usage\":{\"input_tokens\":11,\"output_tokens\":5}}";

fn answer(status: &str, call: &str, body: &str) -> Vec<u8> {
    format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nx-llm-gateway-call: {call}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).into_bytes()
}

fn upstream_failed(call: &str) -> Vec<u8> {
    answer("502 Bad Gateway", call, &json!({"error": {"code": "upstream-failed", "message": "the upstream gave no answer: timeout"}}).to_string())
}

fn model_unknown(call: &str) -> Vec<u8> {
    answer("404 Not Found", call, &json!({"error": {"code": "model-unknown", "message": "the gateway knows no such model"}}).to_string())
}

fn events(l: &Live, run: &str, kind: &str) -> Vec<Value> {
    rows(l, &format!("/daemon/views/events?run_id={run}&after_position=0")).into_iter().filter(|e| e["kind"] == kind).map(|e| e["record"].clone()).collect()
}

/// The child protocol `<prefix>.<name>`: one program step `<name>.write` that writes the artifact `<name>`.
fn child_protocol(h: &Home, prefix: &str, name: &str) {
    let protocol = format!(
        "format: protocol/1\nprotocol:\n  id: {prefix}.{name}\n  revision: 1\n  description: >-\n    One step that writes the {name}.\nartifacts:\n  idea: {{}}\n  {name}: {{}}\nevidence_kinds:\n  {name}_written: {{}}\nclaims:\n  {name}.written:\n    true_when:\n      evidence: {{kind: {name}_written, subject: {name}, result: written}}\nactions:\n  {name}.write:\n    effect: write\n    precondition: {{claim: {name}.written, is: unknown}}\n    may_produce:\n      - evidence: {name}_written\noutcomes:\n  passed:\n    requires: {{claim: {name}.written}}\n"
    );
    std::fs::write(h.home.join(format!("protocol/{prefix}-{name}.protocol.yaml")), protocol).unwrap();
}

/// Bindings `<file>.yaml` of the child protocol `<prefix>.<name>`: its step runs `tools/<file>.sh` and writes `<path>`;
/// exit 0 and exit 3 are taken as `written`.
fn child_bindings(h: &Home, prefix: &str, name: &str, file: &str, path: &str) {
    let bindings = format!(
        "format: loom.bindings/1\nprotocol: {prefix}-{name}@1\nprogram_root: bindings\nartifacts:\n  idea: idea.yaml\n  {name}: {path}\nactions:\n  - action: {name}.write\n    executor:\n      kind: program\n      argv: [tools/{file}.sh, --max-budget-usd, \"0.5\"]\n    exits: {{error: [1, 75, 126, 127]}}\n    timeout_seconds: 20\n    evidence:\n      - kind: {name}_written\n        subject: {name}\n        producer: late\n        from: exit\n        results: {{\"0\": written, \"3\": written, timeout: failed, \"*\": failed}}\n"
    );
    std::fs::write(h.home.join(format!("bindings/{file}.yaml")), bindings).unwrap();
}

/// The root `<id>`: it opens the child case `first` (protocol `first_protocol`, bindings `first_bindings`), then the
/// child case `second`, and is `delivered` once both passed (the shape of the story's own case, `late_tree_runbook`).
fn root(h: &Home, id: &str, first: (&str, &str), second: (&str, &str)) {
    let protocol = format!(
        "format: protocol/1\nprotocol:\n  id: {id}\n  revision: 1\n  description: >-\n    Two child cases one after the other.\nartifacts:\n  idea: {{}}\nevidence_kinds:\n  case_outcome: {{}}\n  first_opened: {{}}\n  second_opened: {{}}\nclaims:\n  first.opened:\n    true_when:\n      evidence: {{kind: first_opened, subject: idea, result: opened}}\n  second.opened:\n    true_when:\n      evidence: {{kind: second_opened, subject: idea, result: opened}}\n  children.passed:\n    true_when:\n      children: {{in: [passed]}}\nactions:\n  first.open:\n    effect: write\n    precondition: {{claim: first.opened, is: unknown}}\n    case_open: {{protocol: {}, subject: idea}}\n    may_produce:\n      - evidence: first_opened\n  second.open:\n    effect: write\n    precondition:\n      all:\n        - {{claim: first.opened}}\n        - {{claim: second.opened, is: unknown}}\n    case_open: {{protocol: {}, subject: idea}}\n    may_produce:\n      - evidence: second_opened\noutcomes:\n  delivered:\n    requires:\n      all:\n        - {{claim: first.opened}}\n        - {{claim: second.opened}}\n        - {{claim: children.passed}}\n",
        first.0, second.0
    );
    let bindings = format!(
        "format: loom.bindings/1\nprotocol: {id}@1\nprogram_root: bindings\nartifacts:\n  idea: idea.yaml\nactions:\n  - action: first.open\n    executor:\n      kind: open\n      name: first\n      bindings: {}.yaml\n    evidence:\n      - kind: first_opened\n        subject: idea\n        producer: loom\n        from: effect\n        results: {{performed: opened}}\n  - action: second.open\n    executor:\n      kind: open\n      name: second\n      bindings: {}.yaml\n    evidence:\n      - kind: second_opened\n        subject: idea\n        producer: loom\n        from: effect\n        results: {{performed: opened}}\n",
        first.1, second.1
    );
    std::fs::write(h.home.join(format!("protocol/{id}.protocol.yaml")), protocol).unwrap();
    std::fs::write(h.home.join(format!("bindings/{id}.yaml")), bindings).unwrap();
}

/// The head of a bound program that calls the run's relay: `call` of `adversary_w4`.
fn head(h: &Home) -> String {
    format!("#!/bin/bash\nf=$RUNBOOK_ROUTE_FILE\nbase=$(jq -r .base_url \"$f\") && token=$(jq -r .token \"$f\") || exit 9\nmodel=default\n{}", super::adversary_w4::call_function(&h.log))
}

/// Submits the runbook `id` on the route `default`; the run's id.
fn submitted(l: &Live, id: &str) -> String {
    let r = send(l.addr, "POST", "/daemon/commands/submit-run", &[("Content-Type", "application/json")], Some(json!({"runbook_id": id, "intake": "red or blue", "model_route": "default"}).to_string().as_bytes()));
    assert_eq!(r.status, 202, "{}", r.text());
    r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned()
}

/// Holds the calling thread until `file` exists, at most 30 s (a guard against a hang under the test's 60 s).
fn until_exists(file: &std::path::Path) {
    let until = Instant::now() + Duration::from_secs(30);
    while Instant::now() < until && !file.exists() {
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The calls a run's record names failed, by step.
type Named = Vec<(Value, Vec<Value>)>;

/// The run's failures, its end and the calls named failed, by step.
fn settled(l: &Live, run: &str) -> (Vec<Value>, Value, Named) {
    let end = until_state(l, run, &["Ended", "FailedToStart", "Cancelled"]);
    let failed: Vec<Value> = events(l, run, "runbook.runtime-failed").into_iter().map(|e| e["reason"].clone()).collect();
    let noted = events(l, run, "runbook.model-calls-failed").iter().map(|e| (e["step"].clone(), e["calls"].as_array().into_iter().flatten().map(|c| c["gateway_call"].clone()).collect())).collect();
    (failed, end, noted)
}

/// Story Acceptance 1 with the late call's step of the same action as the step that runs when the call is booked: two
/// child cases of one protocol (two bindings, one action `notes.write`), as an epic opens one story case per story. The
/// first case's `notes.write` leaves a call in flight and exits 3 (taken as `written`); the gateway answers that call 502
/// while the second case's `notes.write` runs, which then exits 0 with its product. The late call's step ended and went
/// on: the run follows its protocol to `delivered`, with no `runbook.runtime-failed`.
#[test]
fn adversary_late_call_a_late_call_of_an_earlier_run_of_the_same_action_does_not_fail_the_run() {
    let n = Arc::new(AtomicU64::new(0));
    let started = Arc::new(std::sync::Mutex::new(None::<std::path::PathBuf>));
    let seen = started.clone();
    let gw = gateway_answering(
        move |_| {
            if n.fetch_add(1, Ordering::SeqCst) > 0 {
                return OK.to_vec();
            }
            if let Some(f) = seen.lock().unwrap().clone() {
                until_exists(&f);
            }
            upstream_failed("gw-call-late")
        },
        &["a-model"],
    );
    let (h, l, _roots) = daemon_at("adv-late-same", &gw.base_url, &format!("{}{ROUTE}", model_table(Some(&claude_cli()))));
    *started.lock().unwrap() = Some(h.log.join("second-started"));
    root(&h, "adv-same", ("adv.notes", "adv-same-a"), ("adv.notes", "adv-same-b"));
    child_protocol(&h, "adv", "notes");
    child_bindings(&h, "adv", "notes", "adv-same-a", "a.md");
    child_bindings(&h, "adv", "notes", "adv-same-b", "b.md");
    let log = h.log.display();
    write_exe(&h.home.join("bindings/tools/adv-same-a.sh"), &format!("{}( call ''; : > '{log}/late-answered' ) </dev/null >/dev/null 2>&1 &\nsleep 0.3\nprintf 'a\\n' > a.md\nexit 3\n", head(&h)));
    write_exe(&h.home.join("bindings/tools/adv-same-b.sh"), &format!("#!/bin/bash\n: > '{log}/second-started'\nfor i in $(seq 1 200); do [ -e '{log}/late-answered' ] && break; sleep 0.05; done\nprintf 'b\\n' > b.md\nexit 0\n"));
    let run = submitted(&l, "adv-same");
    let (failed, end, noted) = settled(&l, &run);
    let all: Vec<(Value, Value, Value)> = rows(&l, &format!("/daemon/views/events?run_id={run}&after_position=0")).into_iter().map(|e| (e["kind"].clone(), e["record"]["action"].clone(), e["record"]["gateway_call"].clone())).collect();
    let second = all.iter().enumerate().filter(|(_, (k, a, _))| k == "runbook.action-started" && a == "notes.write").nth(1).map(|(i, _)| i);
    let late = all.iter().position(|(k, _, c)| k == "runbook.model-call" && c == "gw-call-late");
    assert!(matches!((second, late), (Some(s), Some(c)) if s < c), "setup: the late call was not booked while the second run of notes.write ran: {all:?} {end}");
    assert!(failed.is_empty(), "a call booked after its step ended failed the run, because the step running then has the same action: {failed:?} {end}");
    assert_eq!((end["state"].clone(), end["outcome"].clone()), (json!("Ended"), json!("delivered")), "{end}");
    assert_eq!(noted, vec![(json!("notes.write"), vec![json!("gw-call-late")])], "the late call is not named once under its step");
}

/// Story `a-recovered-call-does-not-fail-the-run`, Acceptance: a step whose gateway answers 502 to its first call and
/// 200 after, whose program retries and exits 0, does not fail the run. Here an earlier child case of the same protocol
/// ran the same action and exited 3, an exit its bindings take as `written` (the case passed). The second case's
/// `notes.write` recovers from a 502 and exits 0: the run is `delivered`, with no `runbook.runtime-failed`
/// (`refusal::failed_step` must not take the first case's exit 3 as this step's failure).
#[test]
fn adversary_late_call_a_recovered_call_is_not_failed_by_another_cases_exit_of_the_same_action() {
    let n = Arc::new(AtomicU64::new(0));
    let gw = gateway_answering(move |_| if n.fetch_add(1, Ordering::SeqCst) == 0 { upstream_failed("gw-call-recovered") } else { OK.to_vec() }, &["a-model"]);
    let (h, l, _roots) = daemon_at("adv-late-other", &gw.base_url, &format!("{}{ROUTE}", model_table(Some(&claude_cli()))));
    root(&h, "adv-other", ("adv.notes", "adv-other-a"), ("adv.notes", "adv-other-b"));
    child_protocol(&h, "adv", "notes");
    child_bindings(&h, "adv", "notes", "adv-other-a", "a.md");
    child_bindings(&h, "adv", "notes", "adv-other-b", "b.md");
    let log = h.log.display();
    write_exe(&h.home.join("bindings/tools/adv-other-a.sh"), "#!/bin/bash\nprintf 'a\\n' > a.md\nexit 3\n");
    write_exe(&h.home.join("bindings/tools/adv-other-b.sh"), &format!("{}call ''\ncall ''\ntail -n 1 '{log}/statuses' | grep -q ' 200' || exit 8\nprintf 'b\\n' > b.md\nexit 0\n", head(&h)));
    let run = submitted(&l, "adv-other");
    let (failed, end, noted) = settled(&l, &run);
    let calls: Vec<(Value, Value)> = events(&l, &run, "runbook.model-call").iter().map(|c| (c["step"].clone(), c["gateway_call"].clone())).collect();
    assert_eq!(calls.len(), 2, "setup: the second case's step did not make its two calls: {calls:?} {end}");
    assert!(failed.is_empty(), "a recovered call failed the run, taken for a failure of another case's step of the same action: {failed:?} {end}");
    assert_eq!((end["state"].clone(), end["outcome"].clone()), (json!("Ended"), json!("delivered")), "{end}");
    assert_eq!(noted, vec![(json!("notes.write"), vec![json!("gw-call-recovered")])], "the recovered call is not named once under its step");
}

/// The story's Outcome and `docs/engine.md` "A gateway" as this unit wrote it: "A failed call booked after its step ended
/// (its program had exited, and a process of it kept the call open) never fails the run: its step is over and went on."
/// The late call of `notes.write` is answered with the gateway's refusal of the route (`model-unknown` 404) while
/// `digest.write` runs in the next child case and exits 0 with its product.
#[test]
fn adversary_late_call_a_late_route_refusal_does_not_fail_the_step_that_runs_when_it_is_booked() {
    let n = Arc::new(AtomicU64::new(0));
    let started = Arc::new(std::sync::Mutex::new(None::<std::path::PathBuf>));
    let seen = started.clone();
    let gw = gateway_answering(
        move |_| {
            if n.fetch_add(1, Ordering::SeqCst) > 0 {
                return OK.to_vec();
            }
            if let Some(f) = seen.lock().unwrap().clone() {
                until_exists(&f);
            }
            model_unknown("gw-call-late-refused")
        },
        &["a-model"],
    );
    let (h, l, _roots) = daemon_at("adv-late-refused", &gw.base_url, &format!("{}{ROUTE}", model_table(Some(&claude_cli()))));
    *started.lock().unwrap() = Some(h.log.join("digest-started"));
    root(&h, "adv-refused", ("adv.notes", "adv-refused-notes"), ("adv.digest", "adv-refused-digest"));
    child_protocol(&h, "adv", "notes");
    child_protocol(&h, "adv", "digest");
    child_bindings(&h, "adv", "notes", "adv-refused-notes", "notes.md");
    child_bindings(&h, "adv", "digest", "adv-refused-digest", "digest.md");
    let log = h.log.display();
    write_exe(&h.home.join("bindings/tools/adv-refused-notes.sh"), &format!("{}( call ''; : > '{log}/late-answered' ) </dev/null >/dev/null 2>&1 &\nsleep 0.3\nprintf 'notes\\n' > notes.md\nexit 3\n", head(&h)));
    write_exe(&h.home.join("bindings/tools/adv-refused-digest.sh"), &format!("#!/bin/bash\n: > '{log}/digest-started'\nfor i in $(seq 1 200); do [ -e '{log}/late-answered' ] && break; sleep 0.05; done\nprintf 'digest\\n' > digest.md\nexit 0\n"));
    let run = submitted(&l, "adv-refused");
    let (failed, end, _) = settled(&l, &run);
    let all: Vec<(Value, Value, Value)> = rows(&l, &format!("/daemon/views/events?run_id={run}&after_position=0")).into_iter().map(|e| (e["kind"].clone(), e["record"]["action"].clone(), e["record"]["gateway_call"].clone())).collect();
    let digest = all.iter().position(|(k, a, _)| k == "runbook.action-started" && a == "digest.write");
    let late = all.iter().position(|(k, _, c)| k == "runbook.model-call" && c == "gw-call-late-refused");
    assert!(matches!((digest, late), (Some(d), Some(c)) if d < c), "setup: the late call was not booked while digest.write ran: {all:?} {end}");
    assert!(failed.is_empty(), "a route refusal booked after its step ended failed the step that ran then: {failed:?} {end}");
    assert_eq!((end["state"].clone(), end["outcome"].clone()), (json!("Ended"), json!("delivered")), "{end}");
}
