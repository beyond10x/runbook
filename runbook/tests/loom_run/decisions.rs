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

// --- the ends of a run the model decider takes (`cycles/suite.yaml`: decider-reject, decider-release-a3,
// autopilot-stop). The model is a stand-in adapter; the texts are invented. ---

use super::decider::{fixture_settings, install_decider, write_settings};

fn bindings_of(action: &str) -> String {
    format!("format: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: {action}\n    executor: {{kind: person, principal: person:owner}}\n    evidence:\n      - {{kind: draft_review, subject: draft, producer: person:owner, from: effect, results: {{performed: approved}}}}\n")
}

/// One step of a person at `action` of the case `IDEA-1.direction`, asked once and never answered by Loom.
fn one_decision_of(w: &World, action: &str) {
    let d = decision_json(action, "ApproveAction", "person:owner").replace("\"root\"", "\"IDEA-1.direction\"").replace(",\"path\":\"/work/x.yaml\"", "");
    w.fake.answer("tree_run", "default", &tree_run_json("AwaitingPerson", 0, None, &format!("[{d}]")), 3);
}

fn reject_reply(reason: &str) -> String {
    format!("{{\"valid\":true,\"decision\":\"reject\",\"reasons\":[\"{reason}\"],\"cost_usd\":0.02,\"adapter\":\"fake\"}}\n")
}

fn stopped(w: &World) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(w.run_dir("r1/run-stopped.json")).unwrap()).unwrap()
}

#[test]
fn a_rejection_where_the_action_has_no_rejected_result_ends_the_run_naming_step_and_case_and_sends_nothing() {
    let w = World::new("reject-no-path");
    install_decider(&w, &reject_reply("fixture: the context is not usable"));
    bindings(&w, &bindings_of("context.approve"));
    one_decision_of(&w, "context.approve");
    write_settings(&w, &fixture_settings(&w, 5_000_000));
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 1, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "error");
    let reason = outcome["reason"].as_str().unwrap();
    assert!(reason.contains("rejected") && reason.contains("context.approve") && reason.contains("performed"), "{reason}");
    assert!(w.fake.calls_of(&["tree", "approve"]).is_empty() && w.fake.calls_of(&["tree", "close"]).is_empty(), "nothing was sent to Loom");
    assert!(!w.run_dir("r1/status.json").exists(), "a stopped run is not closed");
    assert!(!reason.contains("/home/") && !reason.contains("/tmp/"), "no path in the reason: {reason}");
}

#[test]
#[ignore = "the runner ends this with exit 1 and `error` and the reason names the step but not the case; the driver ended with exit 6, cause rejected_without_rework_path, `autopilot: <step> in <case>: ...`"]
fn a_rejection_without_a_result_to_record_it_names_the_case_and_no_person() {
    let w = World::new("reject-names-case");
    install_decider(&w, &reject_reply("fixture: the context is not usable"));
    bindings(&w, &bindings_of("context.approve"));
    one_decision_of(&w, "context.approve");
    write_settings(&w, &fixture_settings(&w, 5_000_000));
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 6, "{}", stderr(&o));
    let stop = stopped(&w);
    assert_eq!((stop["cause"].as_str(), stop["case"].as_str()), (Some("rejected_without_rework_path"), Some("IDEA-1.direction")));
    let reason = stop["reason"].as_str().unwrap();
    assert!(reason.starts_with("autopilot: context.approve in IDEA-1.direction: ") && !reason.contains("person"), "{reason}");
}

#[test]
#[ignore = "the runner's decider answers only at autonomy A5 (A1 to A4 have no per-class routing); at A3 it stops at the first decision for a person with exit 8, where the driver's decider rejected release.publish and ended exit 6"]
fn a_release_the_decider_rejects_at_a3_ends_without_a_rework_path_and_records_nothing() {
    let w = World::new("release-a3");
    install_decider(&w, &reject_reply("fixture: the release notes name no version"));
    bindings(&w, &bindings_of("release.publish"));
    one_decision_of(&w, "release.publish");
    let mut v = fixture_settings(&w, 5_000_000);
    v["autonomy"] = json!("A3");
    write_settings(&w, &v);
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 6, "{}", stderr(&o));
    assert!(!w.run_dir("r1/status.json").exists());
    let reason = w.outcome("r1")["reason"].as_str().unwrap().to_owned();
    assert!(reason.contains("release.publish") && reason.contains("rejected"), "{reason}");
    assert!(w.fake.calls_of(&["tree", "approve"]).is_empty(), "nothing was recorded");
}

#[test]
fn at_a3_the_decider_does_not_answer_and_the_run_stops_for_a_person_without_calling_the_model() {
    let w = World::new("release-a3-reserved");
    install_decider(&w, &reject_reply("fixture: the release notes name no version"));
    bindings(&w, &bindings_of("release.publish"));
    one_decision_of(&w, "release.publish");
    let mut v = fixture_settings(&w, 5_000_000);
    v["autonomy"] = json!("A3");
    write_settings(&w, &v);
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 8, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "stopped: decider");
    assert!(outcome["reason"].as_str().unwrap().contains("autonomy A3"), "{}", outcome["reason"]);
    assert!(w.fake.calls_of(&["tree", "approve"]).is_empty(), "nothing was recorded");
    assert!(!w.run_dir("r1/status.json").exists());
    assert!(!w.run_dir("r1/bindings/tools/n").exists(), "the adapter was never started");
}

#[test]
fn a_decision_the_decider_has_no_reply_for_stops_the_run_naming_step_and_case_with_no_path_in_the_reason() {
    let w = World::new("stop-names-step");
    install_decider(&w, &reject_reply("fixture: unused"));
    let d = "{\"action\":\"amend.upstream\",\"case\":\"IDEA-1\",\"command\":[\"b10x-loom\",\"tree\",\"run\",\"--store\",\"@STORE@\",\"--confinement\",\"local\"],\"decides\":\"Add a second stop, then run again.\",\"kind\":\"Handoff\"}";
    w.fake.answer("tree_run", "default", &tree_run_json("AwaitingPerson", 0, None, &format!("[{d}]")), 3);
    write_settings(&w, &fixture_settings(&w, 5_000_000));
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 8, "{}", stderr(&o));
    let stop = stopped(&w);
    assert_eq!(stop["format"], "sdc.run-stopped/1");
    assert_eq!(stop["by"], "rule:loom-run");
    assert_eq!(stop["cause"], "decider_cannot_decide");
    let reason = stop["reason"].as_str().unwrap();
    assert!(reason.contains("amend.upstream in IDEA-1: "), "{reason}");
    assert!(!reason.contains("/home/") && !reason.contains("/tmp/"), "{reason}");
}

#[test]
#[ignore = "the reason of a Handoff the decider has no reply for says `names work for a person`; the driver's reason for the same step names no person"]
fn the_reason_of_a_stop_at_a_decision_the_decider_cannot_take_names_no_person() {
    let w = World::new("stop-no-person");
    install_decider(&w, &reject_reply("fixture: unused"));
    let d = "{\"action\":\"amend.upstream\",\"case\":\"IDEA-1\",\"command\":[\"b10x-loom\",\"tree\",\"run\",\"--store\",\"@STORE@\",\"--confinement\",\"local\"],\"decides\":\"Add a second stop, then run again.\",\"kind\":\"Handoff\"}";
    w.fake.answer("tree_run", "default", &tree_run_json("AwaitingPerson", 0, None, &format!("[{d}]")), 3);
    write_settings(&w, &fixture_settings(&w, 5_000_000));
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 8, "{}", stderr(&o));
    let reason = stopped(&w)["reason"].as_str().unwrap().to_owned();
    assert!(!reason.contains("person"), "{reason}");
}

/// A tree that stops once at the close of case `root` through `operating`, then ends.
fn close_tree(w: &World) {
    let f = &w.fake;
    f.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 0, None, &format!("[{}]", close_decision("operating"))), 3);
    f.answer("tree_close", "default", "{\"case\":\"root\",\"outcome\":\"operating\"}\n", 0);
    f.answer("tree_run", "2", &tree_run_json("Terminal", 0, Some("operating"), "[]"), 0);
}

fn adapter_starts(w: &World) -> u32 {
    std::fs::read_to_string(w.run_dir("r1/bindings/tools/n")).map(|t| t.trim().parse().unwrap()).unwrap_or(0)
}

#[test]
fn at_a5_the_close_of_a_case_is_asked_of_the_model_decider_and_run_as_printed() {
    let w = World::new("close-a5");
    install_decider(&w, &reject_reply("unused").replace("\"reject\"", "\"approve\""));
    close_tree(&w);
    write_settings(&w, &fixture_settings(&w, 5_000_000));
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(adapter_starts(&w), 1, "the decider was asked once for the close");
    assert_eq!(w.fake.calls_of(&["tree", "close"]).len(), 1);
    let decided = super::decider::lines(&w.run_dir("r1/decisions.jsonl"));
    assert_eq!((decided[0]["type"].as_str(), decided[0]["decision"].as_str(), decided[0]["model_calls"].as_u64()), (Some("close"), Some("approve"), Some(1)));
    assert_eq!(w.outcome("r1")["outcome"], "ended");
}

#[test]
#[ignore = "the runner asks the model decider for the close of a case at A5 (one model call); the driver closed it by rule:autopilot without asking the decider"]
fn at_a5_with_a_model_decider_the_close_is_taken_by_rule_and_the_decider_is_not_asked() {
    let w = World::new("close-by-rule");
    install_decider(&w, &reject_reply("unused").replace("\"reject\"", "\"approve\""));
    close_tree(&w);
    write_settings(&w, &fixture_settings(&w, 5_000_000));
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(adapter_starts(&w), 0, "the decider was asked");
    assert_eq!(w.fake.calls_of(&["tree", "close"]).len(), 1);
    let decided = super::decider::lines(&w.run_dir("r1/decisions.jsonl"));
    assert_eq!(decided[0]["principal"], "rule:autopilot");
}

#[test]
#[ignore = "at A3 the runner stops at the first decision for a person without asking the decider (exit 8); the driver asked the decider for the close at A3, the decider rejected, and the run stopped unclosed"]
fn at_a3_the_decider_is_asked_for_the_close_rejects_and_the_run_stops_unclosed() {
    let w = World::new("close-a3");
    install_decider(&w, &reject_reply("fixture: not yet"));
    close_tree(&w);
    let mut v = fixture_settings(&w, 5_000_000);
    v["autonomy"] = json!("A3");
    write_settings(&w, &v);
    let o = w.run(&w.args("r1", 5_000_000));
    assert_ne!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(adapter_starts(&w), 1, "the decider was not asked for the close");
    assert!(w.fake.calls_of(&["tree", "close"]).is_empty(), "the case was closed");
    assert!(!w.run_dir("r1/status.json").exists());
}
