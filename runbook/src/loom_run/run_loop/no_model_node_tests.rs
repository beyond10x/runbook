//! The model paths of a run whose opening decided "no model node". A file a bound program wrote
//! after the opening can reach the first two paths in a real run (`tests/loom_run/no_model_node.rs`
//! does it with a fake `b10x-loom`); the others cannot be reached from a file, because the decider
//! and the options are fixed before the run starts, so these tests set the decision of the opening
//! (`model_node::FORCED`, which exists in unit tests only) and check that each path reads it. The
//! model program is a stand-in that writes a marker file: the marker must not exist after the run.
//! No model call and no network request.

use super::fake;
use super::tests::{args, world};
use super::*;
use crate::loom_run::model_node::{ModelNode, FORCED};
use std::collections::VecDeque;
use std::os::unix::fs::PermissionsExt;

/// Sets the decisions the next calls of `model_node::decide` return, in order; dropped, it clears
/// them. A run asks twice: the refusal before the run directory (the first), the opening (the second).
struct Forced;

fn force(decisions: &[ModelNode]) -> Forced {
    FORCED.with(|f| *f.borrow_mut() = decisions.iter().cloned().collect::<VecDeque<_>>());
    Forced
}

impl Drop for Forced {
    fn drop(&mut self) {
        FORCED.with(|f| f.borrow_mut().clear());
    }
}

fn write_exe(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

/// A world that a model decider can run in: the instruction files and the table of who decides
/// beside the bindings, person-only bindings, and a decider whose adapter is the stand-in that writes
/// the returned marker.
fn decider_world(tag: &str) -> (PathBuf, fake::Fake, PathBuf) {
    let (root, f) = world(tag);
    let marker = root.join("marker-model-program");
    let adapter = root.join("stand-in-adapter.sh");
    write_exe(&adapter, &format!("#!/bin/sh\necho started >> '{}'\n", marker.display()));
    std::fs::create_dir_all(root.join("instructions/decider")).unwrap();
    for entry in std::fs::read_dir(repo().join("instructions/decider")).unwrap().filter_map(Result::ok) {
        std::fs::copy(entry.path(), root.join("instructions/decider").join(entry.file_name())).unwrap();
    }
    std::fs::create_dir_all(root.join("driver/examples/sdc")).unwrap();
    std::fs::create_dir_all(root.join("tools")).unwrap();
    std::fs::copy(repo().join("driver/examples/sdc/decision-classes.json"), root.join("driver/examples/sdc/decision-classes.json")).unwrap();
    std::fs::copy(repo().join("tools/autonomy-levels.json"), root.join("tools/autonomy-levels.json")).unwrap();
    std::fs::write(root.join("driver/examples/sdc/rule-ends.json"), "{}\n").unwrap();
    let settings = serde_json::json!({"autonomy": "A5", "decider": {"kind": "model", "model": "m", "adapter": adapter.display().to_string(), "without_claims": true}});
    std::fs::write(root.join("settings.toml"), toml::to_string(&settings).unwrap()).unwrap();
    (root, f, marker)
}

fn approval(action: &str, kind: &str) -> String {
    fake::decision_json(action, kind, "person:owner").replace("\"root\"", "\"IDEA-1.direction\"").replace(",\"path\":\"/work/x.yaml\"", "")
}

fn person_bindings(root: &Path, action: &str) {
    let text = format!("format: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: {action}\n    executor: {{kind: person, principal: person:owner}}\n    evidence:\n      - {{kind: draft_review, subject: draft, producer: person:owner, from: effect, results: {{performed: approved}}}}\n");
    std::fs::write(root.join("bindings/sdc.yaml"), text).unwrap();
}

fn run(root: &Path, f: &fake::Fake) -> Outcome {
    run_with(&args(root, f, 600), &SystemClock, &AtomicBool::new(false)).unwrap()
}

fn path_of(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::StoppedModelCall { path, .. } => path,
        other => panic!("not stopped for a model call: {other:?}"),
    }
}

#[test]
fn the_model_decider_is_not_asked_in_a_run_opened_without_a_model_node() {
    let (root, f, marker) = decider_world("p-decider");
    person_bindings(&root, "scope.approve");
    f.answer("tree_run", "1", &fake::tree_run_json("AwaitingPerson", 0, None, &format!("[{}]", approval("scope.approve", "ApproveAction"))), 3);
    let _forced = force(&[ModelNode::Without, ModelNode::Without]);
    let outcome = run(&root, &f);
    assert_eq!(path_of(&outcome), "decider", "{outcome:?}");
    assert!(!marker.exists(), "the model program started");
    assert!(!root.join("ledger.json").exists(), "no ledger was opened or written");
    assert!(f.calls_of(&["tree", "approve"]).is_empty());
    let stop = std::fs::read_to_string(root.join("runs/r1/outcome.json")).unwrap();
    assert!(stop.contains("model_call_without_model_node") && stop.contains("\"exit_code\": 13"), "{stop}");
}

#[test]
fn the_model_decider_answering_the_answers_file_is_not_asked_in_a_run_opened_without_a_model_node() {
    let (root, f, marker) = decider_world("p-answers");
    person_bindings(&root, "scope.answer");
    write_exe(&root.join("bindings/tools/q-requests.sh"), "#!/bin/sh\necho '{\"type\":\"question\",\"id\":\"q1\",\"question\":{\"id\":\"q1\",\"default\":\"A\"}}'\n");
    write_exe(&root.join("bindings/tools/q-apply.sh"), "#!/bin/sh\nexit 0\n");
    std::fs::write(
        root.join("bindings/tools/answers.yaml"),
        "format: runbook.loom-run-answers/1\nanswers:\n- action: scope.answer\n  on: ApproveAction\n  requests: [tools/q-requests.sh]\n  apply: [tools/q-apply.sh, '{id}', '{answer}', '{principal}']\n",
    )
    .unwrap();
    f.answer("tree_run", "1", &fake::tree_run_json("AwaitingPerson", 0, None, &format!("[{}]", approval("scope.answer", "ApproveAction"))), 3);
    let _forced = force(&[ModelNode::Without, ModelNode::Without]);
    let outcome = run(&root, &f);
    assert_eq!(path_of(&outcome), "decider answering the answers file", "{outcome:?}");
    assert!(!marker.exists(), "the model program started");
    assert!(!root.join("ledger.json").exists());
}

#[test]
fn a_decider_that_reports_a_cost_in_a_run_opened_without_a_model_node_ends_the_run() {
    // A decider that says it calls no model but answers with a call: the answer is not booked
    // (there is no ledger), it ends the run naming the path.
    let (root, f) = world("p-decider-cost");
    person_bindings(&root, "scope.approve");
    let settings = serde_json::json!({"decider": {"kind": "scripted", "max_call_micro_usd": 0, "answers": [{"approve": true, "principal": "person:owner", "cost_micro_usd": 7, "model_calls": 1}]}});
    std::fs::write(root.join("settings.toml"), toml::to_string(&settings).unwrap()).unwrap();
    f.answer("tree_run", "1", &fake::tree_run_json("AwaitingPerson", 0, None, &format!("[{}]", approval("scope.approve", "ApproveAction"))), 3);
    let _forced = force(&[ModelNode::Without, ModelNode::Without]);
    let outcome = run(&root, &f);
    assert_eq!(path_of(&outcome), "decider", "{outcome:?}");
    assert!(!root.join("ledger.json").exists());
    assert!(f.calls_of(&["tree", "approve"]).is_empty(), "the answer was not taken");
}

#[test]
fn a_run_that_decides_with_a_model_node_after_the_pre_check_found_none_fails_closed_without_a_ledger() {
    // The refusal before the run directory reads the source bindings; the opening reads the copy.
    // When they disagree the opening decides, and a run with a model node and no ledger ends in error.
    let (root, f) = world("p-drift");
    let mut a = args(&root, &f, 600);
    a.spend_ledger = None;
    let _forced = force(&[ModelNode::Without, ModelNode::With { because: "forced".into() }]);
    let outcome = run_with(&a, &SystemClock, &AtomicBool::new(false)).unwrap();
    match outcome {
        Outcome::Error { reason } => assert!(reason.contains("--spend-ledger is required"), "{reason}"),
        other => panic!("{other:?}"),
    }
    assert!(f.calls_of(&["tree", "run"]).is_empty(), "no step was started");
}

#[test]
fn a_re_drive_a_replay_setting_conformance_and_a_decider_that_may_call_a_model_count_at_the_opening() {
    use crate::loom_run::decide::{RefusingDecider, ScriptedDecider};
    use crate::loom_run::model_node::decide;
    let (root, f) = world("p-decide");
    let bindings = root.join("bindings");
    let mut plain = args(&root, &f, 600);
    plain.confinement = crate::loom_run::Confinement::Substrate;
    assert_eq!(decide(&plain, &serde_json::json!({}), &RefusingDecider, &bindings), ModelNode::Without, "nothing in this run names a model, and a bound program has no network under substrate");
    let mut local = args(&root, &f, 600);
    local.confinement = crate::loom_run::Confinement::Local;
    assert_eq!(decide(&local, &serde_json::json!({}), &RefusingDecider, &bindings), ModelNode::Capped { level: "local".into() }, "nothing names a model, but a bound program has the network under local: the caps apply");
    let mut redrive = args(&root, &f, 600);
    redrive.ports.replay.recorded = Some(root.join("recording"));
    assert!(decide(&redrive, &serde_json::json!({}), &RefusingDecider, &bindings).has_model_node(), "a re-drive can go live");
    assert!(decide(&plain, &serde_json::json!({"replay": {"live_adapter": "x"}}), &RefusingDecider, &bindings).has_model_node(), "a replay setting can go live");
    let mut observe = args(&root, &f, 600);
    observe.ports.conformance = crate::loom_run::ports::Conformance::Observe;
    assert!(decide(&observe, &serde_json::json!({}), &RefusingDecider, &bindings).has_model_node(), "the observation starts a model");
    let reserving = ScriptedDecider::new(vec![], 5);
    assert!(decide(&plain, &serde_json::json!({}), &reserving, &bindings).has_model_node(), "a decider that reserves an amount");
    assert!(decide(&plain, &serde_json::json!({}), &RefusingDecider, &root.join("absent")).has_model_node(), "bindings that cannot be read");
}

#[test]
fn only_the_levels_the_list_names_take_the_network_from_a_bound_program() {
    use crate::loom_run::model_node::{level_denies_network, NO_NETWORK_LEVELS};
    assert_eq!(NO_NETWORK_LEVELS, ["substrate"]);
    assert!(level_denies_network("substrate"));
    for other in ["local", "linux", "docker", "Substrate", " substrate", "substrate ", "", "none", "substrate2"] {
        assert!(!level_denies_network(other), "{other:?} is not a level the list names, so it is refused like local");
    }
    // every level the options can name is decided by the list: the one it names needs no caps, the other does
    for (level, denies) in [(crate::loom_run::Confinement::Substrate, true), (crate::loom_run::Confinement::Local, false)] {
        assert_eq!(level_denies_network(level.as_arg()), denies, "{}", level.as_arg());
    }
}

#[test]
fn the_conformance_observation_is_not_started_in_a_run_opened_without_a_model_node() {
    use crate::loom_run::conformance;
    assert!(conformance::permitted(&ModelNode::Without).is_err());
    assert!(conformance::permitted(&ModelNode::With { because: "x".into() }).is_ok());
    // after_close with the decision "without": nothing is written for the observation
    let dir = std::env::current_exe().unwrap().parent().unwrap().join(format!("loomrun-unit-conformance-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (root, f) = world("p-conformance");
    let mut a = args(&root, &f, 600);
    a.ports.conformance = crate::loom_run::ports::Conformance::Observe;
    conformance::after_close(&a, &dir, &Outcome::Ended { tree_outcome: None }, &ModelNode::Without);
    assert!(std::fs::read_dir(&dir).unwrap().next().is_none(), "the observation wrote into the run directory");
}
