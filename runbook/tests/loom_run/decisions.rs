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
fn a_rejection_the_action_has_no_result_for_stops_the_run_with_a_named_cause_and_runs_nothing() {
    let w = World::new("undeclared");
    bindings(&w, PERFORMED_ONLY_BINDINGS);
    one_person_decision(&w);
    w.settings(&json!({"decider": {"kind": "scripted", "answers": [{"approve": false, "reasons": "no", "cost_micro_usd": 0}]}}));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 10, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!((outcome["outcome"].as_str(), outcome["exit_code"].as_u64()), (Some("stopped: rejected"), Some(10)));
    let reason = outcome["reason"].as_str().unwrap();
    assert!(reason.contains("rejected") && reason.contains("owner.approve") && reason.contains("performed"), "{reason}");
    assert!(w.fake.calls_of(&["tree", "approve"]).is_empty(), "nothing was sent to Loom");
    assert_eq!(stopped(&w)["cause"], "rejected_without_rework_path");
}

#[test]
fn an_approval_of_a_result_the_action_does_not_declare_is_still_an_error() {
    let w = World::new("undeclared-approve");
    bindings(&w, &PERFORMED_ONLY_BINDINGS.replace("performed: approved", "kept: approved"));
    one_person_decision(&w);
    w.scripted_approve();
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 1, "{}", stderr(&o));
    assert_eq!(w.outcome("r1")["outcome"], "error");
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
fn a_rejection_where_the_action_has_no_rejected_result_stops_the_run_naming_step_and_case_and_sends_nothing() {
    let w = World::new("reject-no-path");
    install_decider(&w, &reject_reply("fixture: the context is not usable"));
    bindings(&w, &bindings_of("context.approve"));
    one_decision_of(&w, "context.approve");
    write_settings(&w, &fixture_settings(&w, 5_000_000));
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 10, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "stopped: rejected");
    let reason = outcome["reason"].as_str().unwrap();
    assert!(reason.contains("rejected") && reason.contains("context.approve") && reason.contains("performed"), "{reason}");
    assert!(w.fake.calls_of(&["tree", "approve"]).is_empty() && w.fake.calls_of(&["tree", "close"]).is_empty(), "nothing was sent to Loom");
    assert!(!w.run_dir("r1/status.json").exists(), "a stopped run is not closed");
    assert!(!reason.contains("/home/") && !reason.contains("/tmp/"), "no path in the reason: {reason}");
}

#[test]
fn a_rejection_without_a_result_to_record_it_names_the_case_and_no_person() {
    let w = World::new("reject-names-case");
    install_decider(&w, &reject_reply("fixture: the context is not usable"));
    bindings(&w, &bindings_of("context.approve"));
    one_decision_of(&w, "context.approve");
    write_settings(&w, &fixture_settings(&w, 5_000_000));
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 10, "{}", stderr(&o));
    let stop = stopped(&w);
    assert_eq!((stop["cause"].as_str(), stop["case"].as_str()), (Some("rejected_without_rework_path"), Some("IDEA-1")), "the stop file names the root case; the reason the case of the step");
    let reason = stop["reason"].as_str().unwrap();
    assert!(reason.starts_with("context.approve in IDEA-1.direction: decider:model rejected ") && !reason.contains("person"), "{reason}");
    assert!(reason.contains("fixture: the context is not usable"), "{reason}");
}

/// A tree that asks `action` once, runs the printed `tree approve`, then ends.
fn approve_tree(w: &World, action: &str) {
    one_decision_of(w, action);
    w.fake.answer("tree_approve", "default", &format!("{{\"action\":\"{action}\"}}\n"), 0);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
}

fn at_level(w: &World, level: &str) {
    let mut v = fixture_settings(w, 5_000_000);
    v["autonomy"] = json!(level);
    write_settings(w, &v);
}

#[test]
fn a_context_the_decider_rejects_at_a3_ends_without_a_rework_path_and_records_nothing() {
    let w = World::new("context-a3");
    install_decider(&w, &reject_reply("fixture: the context names no platform"));
    bindings(&w, &bindings_of("context.approve"));
    one_decision_of(&w, "context.approve");
    at_level(&w, "A3");
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 10, "{}", stderr(&o));
    assert_eq!(adapter_starts(&w), 1, "the table gives the context to the decider at A3: it was asked");
    assert!(!w.run_dir("r1/status.json").exists());
    let reason = w.outcome("r1")["reason"].as_str().unwrap().to_owned();
    assert!(reason.contains("context.approve") && reason.contains("rejected"), "{reason}");
    assert!(w.fake.calls_of(&["tree", "approve"]).is_empty(), "nothing was recorded");
    assert_eq!(stopped(&w)["cause"], "rejected_without_rework_path");
}

#[test]
fn at_a3_the_decider_does_not_answer_and_the_run_stops_for_a_person_without_calling_the_model() {
    let w = World::new("release-a3-reserved");
    install_decider(&w, &reject_reply("fixture: the release notes name no version"));
    bindings(&w, &bindings_of("release.publish"));
    one_decision_of(&w, "release.publish");
    at_level(&w, "A3");
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 8, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "stopped: decider");
    let reason = outcome["reason"].as_str().unwrap();
    assert!(reason.contains("autonomy A3") && reason.contains("class release") && reason.contains("person"), "the table gives the release to a person at A3: {reason}");
    assert!(w.fake.calls_of(&["tree", "approve"]).is_empty(), "nothing was recorded");
    assert!(!w.run_dir("r1/status.json").exists());
    assert!(!w.run_dir("r1/bindings/tools/n").exists(), "the adapter was never started");
}

#[test]
fn at_a4_the_decider_takes_what_the_table_gives_it_as_its_own_principal_and_a_person_keeps_the_rest() {
    let approve = reject_reply("unused").replace("\"reject\"", "\"approve\"");
    let w = World::new("code-review-a4");
    install_decider(&w, &approve);
    bindings(&w, &bindings_of("change.review"));
    approve_tree(&w, "change.review");
    at_level(&w, "A4");
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(adapter_starts(&w), 1);
    let calls = w.fake.calls_of(&["tree", "approve"]);
    assert_eq!(calls.len(), 1);
    assert_eq!(arg_after(&calls[0].argv, "--principal"), "decider:model", "never a person's name");
    let decided = super::decider::lines(&w.run_dir("r1/decisions.jsonl"));
    assert_eq!(decided[0]["principal"], "decider:model");

    let w = World::new("release-a4");
    install_decider(&w, &approve);
    bindings(&w, &bindings_of("release.publish"));
    approve_tree(&w, "release.publish");
    at_level(&w, "A4");
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 8, "{}", stderr(&o));
    assert_eq!(adapter_starts(&w), 0, "the release stays a person's below A5");
    assert!(w.fake.calls_of(&["tree", "approve"]).is_empty());
}

#[test]
fn a_level_below_a5_never_widens_for_a_holder_that_is_not_the_decider_or_a_class_the_table_lacks() {
    // A1 advises (a person decides), A2 gives roadmap to a person, A4 gives merge to a rule with the
    // decider (no rule is built here), and `scope.approve` is in no class.
    for (level, action) in [("A1", "context.approve"), ("A2", "roadmap.approve"), ("A4", "merge.approve"), ("A4", "scope.approve"), ("A0", "context.approve")] {
        let w = World::new(&format!("never-widens-{level}-{action}"));
        install_decider(&w, &reject_reply("unused").replace("\"reject\"", "\"approve\""));
        bindings(&w, &bindings_of(action));
        approve_tree(&w, action);
        at_level(&w, level);
        let o = w.run(&w.args("r1", 5_000_000));
        assert_eq!(code(&o), 8, "{level} {action}: {}", stderr(&o));
        assert_eq!(adapter_starts(&w), 0, "{level} {action}: the decider was asked");
        assert!(w.fake.calls_of(&["tree", "approve"]).is_empty(), "{level} {action}");
        assert!(w.outcome("r1")["reason"].as_str().unwrap().contains("reserved for a person"), "{level} {action}");
    }
}

#[test]
fn an_escalate_below_a5_stops_for_a_person_whatever_the_settings_say() {
    let w = World::new("escalate-a3");
    install_decider(&w, "{\"valid\":true,\"decision\":\"escalate\",\"best\":\"approve\",\"reasons\":[\"fixture: unsure\"],\"cost_usd\":0.02,\"adapter\":\"fake\"}\n");
    bindings(&w, &bindings_of("context.approve"));
    approve_tree(&w, "context.approve");
    let mut v = fixture_settings(&w, 5_000_000);
    v["autonomy"] = json!("A3");
    v["decider"]["escalate"] = json!("final");
    write_settings(&w, &v);
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 8, "{}", stderr(&o));
    assert!(w.outcome("r1")["reason"].as_str().unwrap().contains("the decider escalated"), "{}", w.outcome("r1")["reason"]);
    assert!(w.fake.calls_of(&["tree", "approve"]).is_empty(), "the best answer was not taken");
}

#[test]
fn a_missing_or_unreadable_table_stops_for_a_person_at_a3_and_calls_no_model() {
    for broken in ["missing", "not json", "unknown holder", "level missing"] {
        let w = World::new(&format!("table-{}", broken.replace(' ', "-")));
        install_decider(&w, &reject_reply("unused").replace("\"reject\"", "\"approve\""));
        let table = w.root.join("tools/autonomy-levels.json");
        match broken {
            "missing" => std::fs::remove_file(&table).unwrap(),
            "not json" => std::fs::write(&table, "{").unwrap(),
            "unknown holder" => {
                let text = std::fs::read_to_string(&table).unwrap().replace("\"decider\"", "\"anyone\"");
                std::fs::write(&table, text).unwrap();
            }
            _ => {
                let mut doc: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&table).unwrap()).unwrap();
                doc["levels"].as_object_mut().unwrap().remove("A3");
                std::fs::write(&table, doc.to_string()).unwrap();
            }
        }
        bindings(&w, &bindings_of("context.approve"));
        approve_tree(&w, "context.approve");
        at_level(&w, "A3");
        let o = w.run(&w.args("r1", 5_000_000));
        assert_eq!(code(&o), 8, "{broken}: {}", stderr(&o));
        let reason = w.outcome("r1")["reason"].as_str().unwrap().to_owned();
        assert!(reason.contains("autonomy-levels.json") && reason.contains("a person decides"), "{broken}: {reason}");
        assert_eq!(adapter_starts(&w), 0, "{broken}: the decider was asked");
        assert!(w.fake.calls_of(&["tree", "approve"]).is_empty(), "{broken}");
    }
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

/// Like `close_tree`, through an outcome the close class of the table does not list.
fn close_tree_through(w: &World, outcome: &str) {
    let f = &w.fake;
    f.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 0, None, &format!("[{}]", close_decision(outcome))), 3);
    f.answer("tree_close", "default", &format!("{{\"case\":\"root\",\"outcome\":\"{outcome}\"}}\n"), 0);
    f.answer("tree_run", "2", &tree_run_json("Terminal", 0, Some(outcome), "[]"), 0);
}

#[test]
fn at_a5_the_close_through_an_outcome_the_class_does_not_list_is_asked_of_the_model_decider_and_run_as_printed() {
    let w = World::new("close-a5");
    install_decider(&w, &reject_reply("unused").replace("\"reject\"", "\"approve\""));
    close_tree_through(&w, "kept");
    write_settings(&w, &fixture_settings(&w, 5_000_000));
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(adapter_starts(&w), 1, "the decider was asked once for the close");
    assert_eq!(w.fake.calls_of(&["tree", "close"]).len(), 1);
    let decided = super::decider::lines(&w.run_dir("r1/decisions.jsonl"));
    assert_eq!((decided[0]["type"].as_str(), decided[0]["decision"].as_str(), decided[0]["model_calls"].as_u64()), (Some("close"), Some("approve"), Some(1)));
    assert_eq!(decided[0]["principal"], "decider:model");
    assert_eq!(w.outcome("r1")["outcome"], "ended");
}

#[test]
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
    assert_eq!(decided[0]["model_calls"].as_u64(), Some(0));
}

#[test]
fn at_a3_the_close_is_a_persons_and_the_decider_is_not_asked() {
    let w = World::new("close-a3");
    install_decider(&w, &reject_reply("fixture: not yet"));
    close_tree(&w);
    at_level(&w, "A3");
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 8, "{}", stderr(&o));
    assert_eq!(adapter_starts(&w), 0, "the table gives the close to a person at A3");
    assert!(w.fake.calls_of(&["tree", "close"]).is_empty(), "the case was closed");
    assert!(!w.run_dir("r1/status.json").exists());
    let reason = w.outcome("r1")["reason"].as_str().unwrap().to_owned();
    assert!(reason.contains("autonomy A3") && reason.contains("class close"), "{reason}");
}

#[test]
fn at_a4_the_decider_is_asked_for_the_close_rejects_and_the_run_stops_unclosed() {
    let w = World::new("close-a4");
    install_decider(&w, &reject_reply("fixture: not yet"));
    one_close_forever(&w);
    at_level(&w, "A4");
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 9, "{}", stderr(&o));
    assert_eq!(adapter_starts(&w), 1, "the decider was asked once for the close");
    assert_eq!(w.outcome("r1")["outcome"], "denied");
    assert!(w.fake.calls_of(&["tree", "close"]).is_empty(), "the case was closed");
    assert!(!w.run_dir("r1/status.json").exists());
}

/// A tree that prints the close of case `root` through `operating` every time it is run.
fn one_close_forever(w: &World) {
    w.fake.answer("tree_run", "default", &tree_run_json("AwaitingPerson", 0, None, &format!("[{}]", close_decision("operating"))), 3);
}
