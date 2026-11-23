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

fn handoff() -> String {
    "{\"action\":\"amend.upstream\",\"case\":\"root\",\"command\":[\"b10x-loom\",\"tree\",\"run\",\"--store\",\"@STORE@\",\"--confinement\",\"local\"],\"decides\":\"Add a second stop, then run again.\",\"kind\":\"Handoff\"}".to_owned()
}

fn no_cause(w: &World) {
    let outcome = w.outcome("r1");
    assert!(outcome.get("cause").is_none_or(Value::is_null), "no cause is made up: {outcome}");
}

/// A handoff's printed command is `tree run`; its refusal is read like any other (a library run runs no
/// handoff, so this is a `--loom` case only).
#[test]
fn a_refused_handoff_ends_the_run_refused_with_its_reason_as_the_cause() {
    let w = World::new("child-handoff");
    w.fake.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 0, None, &format!("[{}]", handoff())), 3);
    w.fake.answer("tree_run", "2", "", 2);
    w.fake.stderr("tree_run", "2", "b10x-loom: tree refused: TreeStopped: held by process 4: the tree is stopped\n");
    w.scripted_approve();
    let o = w.run(&w.args("r1", 1_000_000));
    assert_refused(&w, &o, "tree_stopped");
}

#[test]
fn a_reason_inside_another_message_or_on_an_indented_line_stays_a_loom_failure() {
    let w = World::new("child-inside");
    w.fake.answer("tree_run", "default", "", 2);
    w.fake.stderr("tree_run", "default", "b10x-loom: tree run: Stuck: case c: the log said b10x-loom: tree refused: TreeStopped: x\n  b10x-loom: tree refused: TreeStopped: indented\n");
    w.scripted_approve();
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 1, "{}", stderr(&o));
    no_cause(&w);
}

#[test]
fn a_line_that_names_no_reason_stays_a_loom_failure() {
    let w = World::new("child-noreason");
    w.fake.answer("tree_run", "default", "", 2);
    w.fake.stderr("tree_run", "default", "b10x-loom: tree refused: Whatever: x\nb10x-loom: tree refused: \nb10x-loom: tree refused: treestopped: x\n");
    w.scripted_approve();
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 1, "{}", stderr(&o));
    no_cause(&w);
}

#[test]
fn the_first_of_two_refusal_lines_is_the_cause() {
    let w = World::new("child-two");
    w.fake.answer("tree_run", "default", "", 2);
    w.fake.stderr("tree_run", "default", "b10x-loom: tree refused: TreeStopped: a\nb10x-loom: tree refused: BindingsRefused: b\n");
    w.scripted_approve();
    let o = w.run(&w.args("r1", 1_000_000));
    assert_refused(&w, &o, "tree_stopped");
}

#[test]
fn a_refusal_line_beside_an_exit_code_tree_run_knows_is_not_a_refusal() {
    let w = World::new("child-known");
    w.fake.answer("tree_run", "default", &tree_run_json("Terminal", 0, Some("operating"), "[]"), 0);
    w.fake.stderr("tree_run", "default", "b10x-loom: tree refused: TreeStopped: x\n");
    w.scripted_approve();
    let o = w.run(&w.args("r1", 1_000_000));
    assert_ne!(code(&o), 12, "{}", stderr(&o));
    no_cause(&w);
}

/// The tail of a child's standard error that goes to the files of the run holds neither the runner's
/// login directory nor a token of the runner's environment (the child's environment is built).
#[test]
fn the_tail_of_a_refusal_holds_no_login_path_and_no_environment_value() {
    let w = World::new("child-tail");
    let home = std::env::var("HOME").unwrap();
    w.fake.answer("tree_run", "default", "", 2);
    w.fake.hook("tree_run", "default", "printf 'note: HOME=%s TOKEN=%s\\n' \"$HOME\" \"$RUNBOOK_TEST_TOKEN\" >&2\nprintf 'b10x-loom: tree refused: TreeStopped: held\\n' >&2\n");
    w.scripted_approve();
    let mut cmd = w.command(&w.args("r1", 1_000_000));
    cmd.env("RUNBOOK_TEST_TOKEN", "token-of-the-runner-0123456789");
    let o = cmd.output().unwrap();
    assert_eq!(code(&o), 12, "{}", stderr(&o));
    let mut texts = vec![("console".to_owned(), String::from_utf8_lossy(&o.stderr).into_owned() + &String::from_utf8_lossy(&o.stdout))];
    for f in ["outcome.json", "run-stopped.json", "events.jsonl"] {
        texts.push((f.to_owned(), std::fs::read_to_string(w.run_dir("r1").join(f)).unwrap_or_default()));
    }
    for (name, t) in texts {
        assert!(!t.contains("token-of-the-runner-0123456789"), "{name} carries the token: {t}");
        assert!(!t.contains(&home), "{name} carries the login directory: {t}");
    }
}
