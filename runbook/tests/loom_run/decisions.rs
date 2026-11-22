//! What the runner does with the decisions Loom prints: a named result and the reasons of a
//! rejection, and the printed command of a close and of a handoff, run as printed (Loom 5af3f9c2).

use super::core::{code, stderr, World};
use super::fake::{decision_json, tree_run_json};
use serde_json::json;

const PERSON_BINDINGS: &str = "format: loom.bindings/1
protocol: sdc@8
actions:
  - action: owner.approve
    executor:
      kind: person
      principal: person:owner
    evidence:
      - kind: draft_review
        subject: draft
        producer: person:owner
        from: effect
        results: {performed: approved, rejected: rejected}
";

const PERFORMED_ONLY_BINDINGS: &str = "format: loom.bindings/1
protocol: sdc@8
actions:
  - action: owner.approve
    executor:
      kind: person
      principal: person:owner
    evidence:
      - kind: draft_review
        subject: draft
        producer: person:owner
        from: effect
        results: {performed: approved}
";

fn bindings(w: &World, text: &str) {
    std::fs::write(w.root.join("bindings/sdc.yaml"), text).unwrap();
}

fn one_person_decision(w: &World) {
    let f = &w.fake;
    let d = decision_json("owner.approve", "ApproveAction", "person:owner");
    f.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 0, None, &format!("[{d}]")), 3);
    f.answer("tree_approve", "default", "{\"action\":\"owner.approve\"}\n", 0);
    f.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
}

fn arg_after<'a>(argv: &'a [String], flag: &str) -> &'a str {
    let at = argv.iter().position(|x| x == flag).unwrap_or_else(|| panic!("no {flag} in {argv:?}"));
    &argv[at + 1]
}

#[test]
fn a_rejection_is_a_named_result_with_the_reasons_in_a_file_of_the_run_directory() {
    let w = World::new("reject");
    bindings(&w, PERSON_BINDINGS);
    one_person_decision(&w);
    let reasons = format!("Add a second line.\nThe login {} and /marker-home-dir/notes are not for this text.", w.login_path().display());
    w.settings(&json!({"decider": {"kind": "scripted", "answers": [
        {"approve": false, "reasons": reasons, "cost_micro_usd": 0}]}}));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let calls = w.fake.calls_of(&["tree", "approve"]);
    assert_eq!(calls.len(), 1);
    let argv = &calls[0].argv;
    assert!(!argv.iter().any(|a| a == "--deny"), "{argv:?}");
    assert_eq!(arg_after(argv, "--action"), "owner.approve");
    assert_eq!(arg_after(argv, "--result"), "rejected");
    let file = std::path::PathBuf::from(arg_after(argv, "--reasons-file"));
    let workspace = w.run_dir("r1").join("work");
    assert!(file.starts_with(&workspace), "{} is not inside the workspace {}", file.display(), workspace.display());
    assert!(!file.strip_prefix(&workspace).unwrap().components().any(|c| !matches!(c, std::path::Component::Normal(_))));
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.starts_with("Add a second line.\n"), "{text}");
    assert!(!text.contains("LOGIN-FILE-MARKER") && !text.contains(&w.login_path().display().to_string()), "{text}");
    assert!(!text.contains("/marker-home-dir"), "{text}");
    assert!(text.contains("<login file>"), "{text}");
    assert!(!std::fs::symlink_metadata(&file).unwrap().file_type().is_symlink());
}

#[test]
fn a_rejection_without_reasons_gets_a_text_that_says_so() {
    let w = World::new("reject-bare");
    bindings(&w, PERSON_BINDINGS);
    one_person_decision(&w);
    w.settings(&json!({"decider": {"kind": "scripted", "answers": [{"approve": false, "cost_micro_usd": 0}]}}));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let argv = &w.fake.calls_of(&["tree", "approve"])[0].argv;
    let text = std::fs::read_to_string(arg_after(argv, "--reasons-file")).unwrap();
    assert!(text.contains("no reasons"), "{text}");
}

#[test]
fn an_approval_names_its_result_where_the_action_declares_results() {
    let w = World::new("approve-result");
    bindings(&w, PERSON_BINDINGS);
    one_person_decision(&w);
    w.scripted_approve();
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let argv = &w.fake.calls_of(&["tree", "approve"])[0].argv;
    assert_eq!(arg_after(argv, "--result"), "performed");
    assert!(!argv.iter().any(|a| a == "--reasons-file" || a == "--reasons" || a == "--deny"), "{argv:?}");
}

#[test]
fn a_result_the_action_does_not_declare_ends_the_run_with_a_named_error_and_runs_nothing() {
    let w = World::new("undeclared");
    bindings(&w, PERFORMED_ONLY_BINDINGS);
    one_person_decision(&w);
    w.settings(&json!({"decider": {"kind": "scripted", "answers": [{"approve": false, "reasons": "no", "cost_micro_usd": 0}]}}));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 1, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "error");
    let reason = outcome["reason"].as_str().unwrap();
    assert!(reason.contains("rejected") && reason.contains("owner.approve") && reason.contains("performed"), "{reason}");
    assert!(w.fake.calls_of(&["tree", "approve"]).is_empty(), "nothing was sent to Loom");
}

fn close_decision(outcome: &str) -> String {
    format!(
        "{{\"action\":null,\"case\":\"root\",\"command\":[\"b10x-loom\",\"tree\",\"close\",\"--store\",\"@STORE@\",\"--case\",\"root\",\"--outcome\",\"{outcome}\"],\"decides\":\"close case root through {outcome}\",\"kind\":\"Close\",\"outcome\":\"{outcome}\"}}"
    )
}

#[test]
fn a_close_runs_as_printed_and_the_other_outcomes_of_that_case_are_not_asked() {
    let w = World::new("close");
    let f = &w.fake;
    let both = format!("[{},{}]", close_decision("kept"), close_decision("left"));
    f.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 0, None, &both), 3);
    f.answer("tree_close", "default", "{\"case\":\"root\",\"outcome\":\"kept\"}\n", 0);
    f.answer("tree_run", "2", &tree_run_json("Terminal", 0, Some("kept"), "[]"), 0);
    w.settings(&json!({"decider": {"kind": "scripted", "answers": [{"approve": true, "cost_micro_usd": 0}]}}));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let calls = w.fake.calls_of(&["tree", "close"]);
    assert_eq!(calls.len(), 1);
    let store = w.run_dir("r1/store").display().to_string();
    assert_eq!(calls[0].argv, ["tree", "close", "--store", store.as_str(), "--case", "root", "--outcome", "kept"]);
    assert_eq!(w.outcome("r1")["reason"], "kept");
}

#[test]
fn a_handoff_runs_the_printed_tree_run_with_the_confinement_loom_printed() {
    let w = World::new("handoff");
    let f = &w.fake;
    let handoff = "{\"action\":\"amend.upstream\",\"case\":\"root\",\"command\":[\"b10x-loom\",\"tree\",\"run\",\"--store\",\"@STORE@\",\"--confinement\",\"local\"],\"decides\":\"Add a second stop, then run again.\",\"kind\":\"Handoff\"}";
    f.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 2, None, &format!("[{handoff}]")), 3);
    f.answer("tree_run", "2", &tree_run_json("BudgetSpent", 1, Some("the run took its steps"), "[]"), 5);
    f.answer("tree_run", "3", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.settings(&json!({"decider": {"kind": "scripted", "answers": [{"approve": true, "cost_micro_usd": 0}]}}));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let runs = w.fake.calls_of(&["tree", "run"]);
    assert_eq!(runs.len(), 3);
    let store = w.run_dir("r1/store").display().to_string();
    assert_eq!(runs[1].argv, ["tree", "run", "--store", store.as_str(), "--confinement", "local"], "the printed command, unchanged");
    assert_eq!(w.outcome("r1")["outcome"], "ended");
}

#[test]
fn a_handoff_that_comes_back_after_its_command_ran_ends_the_run_with_a_named_error() {
    let w = World::new("handoff-again");
    let f = &w.fake;
    let handoff = "{\"action\":\"amend.upstream\",\"case\":\"root\",\"command\":[\"b10x-loom\",\"tree\",\"run\",\"--store\",\"@STORE@\",\"--confinement\",\"local\"],\"decides\":\"Add a second stop, then run again.\",\"kind\":\"Handoff\"}";
    f.answer("tree_run", "default", &tree_run_json("AwaitingPerson", 0, None, &format!("[{handoff}]")), 3);
    w.settings(&json!({"decider": {"kind": "scripted", "answers": [{"approve": true, "cost_micro_usd": 0}]}}));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 1, "{}", stderr(&o));
    let reason = w.outcome("r1")["reason"].as_str().unwrap().to_owned();
    assert!(reason.contains("Handoff") && reason.contains("asked again"), "{reason}");
}
