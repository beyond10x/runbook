//! A child `b10x-loom` that refuses a call prints `b10x-loom: tree refused: <Reason>: ...` on standard
//! error. `docs/loom-run.md` ("A refusal of Loom by a named reason"): the runner reads that line for
//! every call it makes, so the same cause and exit 12 arise with `--loom` as in a library run. The
//! child is the fake `b10x-loom` of the engine tests; no model, no network.

use super::core::{code, stderr, World};
use super::fake::{decision_json, tree_run_json};
use serde_json::Value;

fn stopped(w: &World, name: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(w.run_dir(name).join("run-stopped.json")).unwrap()).unwrap()
}

fn assert_refused(w: &World, o: &std::process::Output, cause: &str) {
    assert_eq!(code(o), 12, "{}", stderr(o));
    let outcome = w.outcome("r1");
    assert_eq!((outcome["outcome"].as_str(), outcome["cause"].as_str()), (Some("refused"), Some(cause)), "{outcome}");
    assert_eq!(outcome["exit_code"], 12, "{outcome}");
    assert!(outcome["loom_exit_code"].is_number(), "the child's exit code is kept: {outcome}");
    assert!(outcome["stderr_tail"].as_str().is_some_and(|t| t.contains("tree refused")), "the tail of the child's standard error is kept: {outcome}");
    assert_eq!(stopped(w, "r1")["cause"], cause);
}

#[test]
fn a_refused_tree_run_ends_the_run_refused_with_its_reason_as_the_cause() {
    let w = World::new("child-run");
    w.fake.answer("tree_run", "default", "", 2);
    w.fake.stderr("tree_run", "default", "b10x-loom: tree refused: TreeStopped: held by process 4: the tree is stopped\n");
    w.scripted_approve();
    let o = w.run(&w.args("r1", 1_000_000));
    assert_refused(&w, &o, "tree_stopped");
}

#[test]
fn a_refused_tree_approve_ends_the_run_refused_with_its_reason_as_the_cause() {
    let w = World::new("child-approve");
    let d = decision_json("owner.approve", "ApproveAction", "person:owner");
    w.fake.answer("tree_run", "default", &tree_run_json("AwaitingPerson", 0, None, &format!("[{d}]")), 3);
    w.fake.answer("tree_approve", "default", "", 2);
    w.fake.stderr("tree_approve", "default", "b10x-loom: tree refused: PersonProgramFailed: case \"c\": the program exited 3\n");
    w.scripted_approve();
    let o = w.run(&w.args("r1", 1_000_000));
    assert_refused(&w, &o, "person_program_failed");
}

#[test]
fn a_refused_tree_open_ends_the_run_refused_with_its_reason_as_the_cause() {
    let w = World::new("child-open");
    w.fake.answer("tree_open", "default", "", 2);
    w.fake.hook("tree_open", "default", "");
    w.fake.stderr("tree_open", "default", "b10x-loom: tree refused: BindingsRefused: bindings of engine-demo: an action the protocol does not have\n");
    w.scripted_approve();
    let o = w.run(&w.args("r1", 1_000_000));
    assert_refused(&w, &o, "bindings_refused");
}

#[test]
fn an_exit_with_no_reason_on_standard_error_stays_a_loom_failure() {
    let w = World::new("child-approve-plain");
    let d = decision_json("owner.approve", "ApproveAction", "person:owner");
    w.fake.answer("tree_run", "default", &tree_run_json("AwaitingPerson", 0, None, &format!("[{d}]")), 3);
    w.fake.answer("tree_approve", "default", "", 2);
    w.fake.stderr("tree_approve", "default", "b10x-loom: the store is gone\n");
    w.scripted_approve();
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 1, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "error", "{outcome}");
    assert!(outcome.get("cause").is_none_or(Value::is_null), "no cause is made up: {outcome}");
}
