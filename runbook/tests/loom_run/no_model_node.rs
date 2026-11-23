//! A run whose bindings hold no model node runs with caps of 0 and no spend ledger; a run with one
//! keeps every cap as it was; a model program started in a run that was opened without one is
//! refused (`docs/loom-run.md`, "A run without a model node"). A fake `b10x-loom`, sh programs and a
//! stand-in for the model tool that writes a marker file: no model call, no network request.

use super::core::{code, model_call_hook, spent, stderr, World};
use super::fake::tree_run_json;
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

const NO_MODEL: &str = "format: loom.bindings/1
protocol: sdc@8
actions:
  - action: check.run
    executor:
      kind: program
      argv: [tools/sdc-tool.sh, check-idea]
  - action: scope.approve
    executor: {kind: person, principal: person:owner}
";

const NAMES_MODEL: &str = "format: loom.bindings/1
protocol: sdc@8
actions:
  - action: node.write
    executor:
      kind: program
      argv: [sdc-node, node.write]
";

fn set(a: &mut [String], flag: &str, value: &str) {
    let i = a.iter().position(|x| x == flag).unwrap();
    a[i + 1] = value.to_owned();
}

fn without(a: &mut Vec<String>, flag: &str) {
    let i = a.iter().position(|x| x == flag).unwrap();
    a.drain(i..=i + 1);
}

/// The options of run `r1` with every cap at 0 and no ledger.
fn zero(w: &World) -> Vec<String> {
    let mut a = w.args("r1", 0);
    set(&mut a, "--max-llm-calls", "0");
    without(&mut a, "--spend-ledger");
    a
}

fn bindings(w: &World, text: &str) {
    std::fs::write(w.root.join("bindings/sdc.yaml"), text).unwrap();
}

fn tree_of_two_steps(w: &World) {
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
}

fn events(w: &World) -> Vec<Value> {
    let text = std::fs::read_to_string(w.run_dir("r1").join("events.jsonl")).unwrap();
    text.lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

fn decided(w: &World) -> Vec<Value> {
    events(w).into_iter().filter(|e| e["event"] == "model_node_decided").collect()
}

/// A stand-in for the model tool that the runner finds on its PATH (the world's root is first);
/// it writes the marker file `marker`, so a marker after a run says a model program started.
fn stand_in_model_tool(w: &World) -> PathBuf {
    let marker = w.root.join("marker-model-tool");
    let script = w.root.join("sdc-node");
    std::fs::write(&script, format!("#!/bin/sh\necho started >> '{}'\n", marker.display())).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    marker
}

const REWRITE_TO_MODEL: &str = "cat > bindings/sdc.yaml <<'EOF'
format: loom.bindings/1
protocol: sdc@8
actions:
  - action: node.write
    executor:
      kind: program
      argv: [sdc-node, node.write]
EOF
";

#[test]
fn a_run_without_a_model_node_runs_with_caps_of_0_and_no_ledger() {
    let w = World::new("zero-caps");
    bindings(&w, NO_MODEL);
    tree_of_two_steps(&w);
    let o = w.run(&zero(&w));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), 2);
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "ended", "{outcome}");
    assert_eq!((outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some(0), Some(0)), "{outcome}");
    assert_eq!(outcome["has_model_node"], false, "{outcome}");
    assert!(!w.ledger().exists(), "no ledger is opened");
    let opening = decided(&w);
    assert_eq!(opening.len(), 1, "the decision is written once");
    assert_eq!(opening[0]["has_model_node"], false);
    assert_eq!(events(&w)[1]["event"], "model_node_decided", "it is the event after run_started: {:?}", events(&w).iter().take(3).collect::<Vec<_>>());
}

#[test]
fn a_run_without_a_model_node_given_a_ledger_does_not_open_it() {
    let w = World::new("zero-caps-ledger-given");
    bindings(&w, NO_MODEL);
    tree_of_two_steps(&w);
    let mut a = w.args("r1", 0);
    set(&mut a, "--max-llm-calls", "0");
    let o = w.run(&a);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(!w.ledger().exists(), "a ledger that is named and not needed is not created");
    assert_eq!(w.outcome("r1")["spent_micro_usd"], 0);
}

#[test]
fn without_a_model_node_a_tree_run_is_not_a_call_and_the_run_does_not_end_on_the_call_cap() {
    let w = World::new("not-a-call");
    bindings(&w, NO_MODEL);
    w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.answer("tree_run", "4", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    let mut a = w.args("r1", 1_000_000);
    set(&mut a, "--max-llm-calls", "1");
    let o = w.run(&a);
    assert_eq!(code(&o), 0, "four tree runs against a cap of 1 call: {}", stderr(&o));
    assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), 4);
    assert_ne!(w.outcome("r1")["outcome"], "stopped: llm calls");
    assert!(!w.ledger().exists(), "no ledger, so nothing was spent");
}

#[test]
fn the_wall_time_bound_stays_for_a_run_without_a_model_node() {
    let w = World::new("zero-wall");
    bindings(&w, NO_MODEL);
    tree_of_two_steps(&w);
    let mut a = zero(&w);
    set(&mut a, "--max-wall-seconds", "0");
    let o = w.run(&a);
    assert_eq!(code(&o), 5, "{}", stderr(&o));
    assert_eq!(w.outcome("r1")["outcome"], "stopped: wall time");
    assert!(w.fake.calls_of(&["tree", "run"]).is_empty());
}

#[test]
fn a_run_with_a_model_node_stops_at_a_call_cap_of_0_before_the_first_tree_run() {
    let w = World::new("model-cap-0-calls");
    let mut a = w.args("r1", 1_000_000);
    set(&mut a, "--max-llm-calls", "0");
    let o = w.run(&a);
    assert_eq!(code(&o), 6, "{}", stderr(&o));
    assert!(w.fake.calls_of(&["tree", "run"]).is_empty(), "no step started");
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "stopped: llm calls");
    assert_eq!(outcome["has_model_node"], true, "{outcome}");
    assert_eq!(decided(&w)[0]["has_model_node"], true);
}

#[test]
fn a_run_with_a_model_node_stops_at_a_spend_cap_of_0_before_the_first_tree_run() {
    let w = World::new("model-cap-0-spend");
    let o = w.run(&w.args("r1", 0));
    assert_eq!(code(&o), 4, "{}", stderr(&o));
    assert!(w.fake.calls_of(&["tree", "run"]).is_empty(), "no step started");
    assert_eq!(w.outcome("r1")["outcome"], "stopped: spend cap");
    assert_eq!(w.outcome("r1")["has_model_node"], true);
}

#[test]
fn a_run_with_a_model_node_refuses_a_missing_ledger_before_anything_starts() {
    let w = World::new("model-no-ledger");
    let mut a = w.args("r1", 1_000_000);
    without(&mut a, "--spend-ledger");
    let o = w.run(&a);
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    let text = stderr(&o);
    assert!(text.contains("refused") && text.contains("--spend-ledger is required") && text.contains("model node"), "{text}");
    assert!(!w.run_dir("r1").exists(), "nothing was written");
    assert!(w.fake.calls().is_empty(), "nothing was started");
}

#[test]
fn the_forms_that_count_as_a_model_node_refuse_a_missing_ledger() {
    let wrap = |argv: &str| format!("format: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: a.b\n    executor:\n      kind: program\n      argv: {argv}\n");
    let forms: Vec<(&str, String)> = vec![
        ("the first word names the model tool", wrap("[sdc-node, a.b]")),
        ("a path whose last part is the model tool", wrap("[tools/sdc-node, a.b]")),
        ("a shell wrapper whose words name it", wrap("[sh, -c, \"sdc-node a.b\"]")),
        ("an env wrapper", wrap("[env, X=1, sdc-node, a.b]")),
        ("a timeout wrapper", wrap("[timeout, \"60\", sdc-node, a.b]")),
        ("an argv that is a text", wrap("\"sdc-node a.b\"")),
        ("an argv that is a mapping", wrap("{run: sdc-node}")),
        ("an argv with a word that is not a text", wrap("[tools/sdc-tool.sh, 7]")),
        ("an empty argv", wrap("[]")),
        ("an executor kind the rule does not know", "format: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: a.b\n    executor: {kind: agent}\n".to_owned()),
        ("an action with no executor", "format: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: a.b\n".to_owned()),
        ("a file that is not YAML", "format: [unclosed\n".to_owned()),
        ("actions that are not a list", "format: loom.bindings/1\nactions: {a: b}\n".to_owned()),
    ];
    for (what, text) in &forms {
        let w = World::new("forms");
        bindings(&w, text);
        let mut a = w.args("r1", 0);
        without(&mut a, "--spend-ledger");
        let o = w.run(&a);
        assert_eq!(code(&o), 2, "{what}: counts as a model node, so the missing ledger is refused: {}", stderr(&o));
        assert!(stderr(&o).contains("--spend-ledger is required"), "{what}: {}", stderr(&o));
    }
    // a model node named in another bindings file, and in the answers file
    for (what, file, text) in [
        ("another bindings file", "bindings/sdc-intake.yaml", NAMES_MODEL),
        ("the answers file", "bindings/tools/answers.yaml", "format: runbook.loom-run-answers/1\nanswers:\n- action: x\n  on: Handoff\n  requests: [sdc-node, r]\n  apply: [tools/a.sh]\n"),
    ] {
        let w = World::new("forms-other");
        bindings(&w, NO_MODEL);
        std::fs::create_dir_all(w.root.join("bindings/tools")).unwrap();
        std::fs::write(w.root.join(file), text).unwrap();
        let mut a = w.args("r1", 0);
        without(&mut a, "--spend-ledger");
        let o = w.run(&a);
        assert_eq!(code(&o), 2, "{what}: {}", stderr(&o));
    }
}

#[test]
fn the_forms_that_do_not_count_run_without_a_ledger() {
    let programs = "format: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: a.b\n    executor:\n      kind: program\n      argv: [tools/sdc-tool.sh, check-idea, --note, node]\n  - action: c.d\n    executor: {kind: program, argv: [sh, -c, \"echo hello\"]}\n  - action: e.f\n    executor: {kind: person, principal: person:owner}\n";
    for text in [NO_MODEL, programs, "format: loom.bindings/1\n"] {
        let w = World::new("not-forms");
        bindings(&w, text);
        tree_of_two_steps(&w);
        let o = w.run(&zero(&w));
        assert_eq!(code(&o), 0, "{text}: {}", stderr(&o));
        assert_eq!(w.outcome("r1")["has_model_node"], false, "{text}");
    }
}

#[test]
fn a_decider_that_calls_a_model_counts_as_a_model_node() {
    let w = World::new("model-decider");
    bindings(&w, NO_MODEL);
    w.settings(&json!({"autonomy": "A5", "decider": {"kind": "model", "model": "m", "adapter": "claude-cli", "without_claims": true}}));
    let mut a = w.args("r1", 0);
    without(&mut a, "--spend-ledger");
    let o = w.run(&a);
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(stderr(&o).contains("--spend-ledger is required") && stderr(&o).contains("decider"), "{}", stderr(&o));
    // a decider of a rule calls none
    w.settings(&json!({"autonomy": "A5", "decider": {"kind": "rule", "verdict": "approve"}}));
    tree_of_two_steps(&w);
    let o = w.run(&zero(&w));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(w.outcome("r1")["has_model_node"], false);
    // a scripted decider that reserves an amount does
    let w = World::new("scripted-decider");
    bindings(&w, NO_MODEL);
    w.settings(&json!({"decider": {"kind": "scripted", "max_call_micro_usd": 5, "answers": []}}));
    let mut a = w.args("r1", 0);
    without(&mut a, "--spend-ledger");
    assert_eq!(code(&w.run(&a)), 2);
}

#[test]
fn a_run_that_re_drives_a_recording_or_observes_conformance_counts_as_having_a_model_node() {
    let w = World::new("conformance-flag");
    bindings(&w, NO_MODEL);
    let mut a = w.args("r1", 0);
    without(&mut a, "--spend-ledger");
    a.extend(["--conformance".to_owned(), "observe".to_owned()]);
    let o = w.run(&a);
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(stderr(&o).contains("--spend-ledger is required") && stderr(&o).contains("conformance"), "{}", stderr(&o));
}

#[test]
fn a_run_with_a_model_node_and_an_unreadable_ledger_fails_closed_before_the_first_call() {
    let w = World::new("model-ledger-bad");
    std::fs::write(w.ledger(), "this is not a ledger").unwrap();
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 1, "{}", stderr(&o));
    assert!(w.outcome("r1")["reason"].as_str().unwrap().contains("spend ledger"));
    assert!(w.fake.calls_of(&["tree", "run"]).is_empty());
}

// --- a model program in a run opened without a model node

#[test]
fn a_binding_rewritten_after_the_opening_to_name_the_model_tool_is_refused_before_it_starts() {
    let w = World::new("rewrite-tree-run");
    let marker = stand_in_model_tool(&w);
    bindings(&w, NO_MODEL);
    w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "1", REWRITE_TO_MODEL);
    w.fake.hook("tree_run", "2", "sdc-node node.write\n");
    let o = w.run(&zero(&w));
    assert_eq!(code(&o), 13, "{}", stderr(&o));
    assert!(!marker.exists(), "the model program started");
    assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), 1, "the second tree run was not started");
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "stopped: model call", "{outcome}");
    assert_eq!(outcome["cause"], "model_call_without_model_node", "{outcome}");
    assert_eq!((outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some(0), Some(0)));
    assert_eq!(outcome["has_model_node"], false, "the decision of the opening is not changed");
    assert!(!w.ledger().exists());
    let stop: Value = serde_json::from_str(&std::fs::read_to_string(w.run_dir("r1").join("run-stopped.json")).unwrap()).unwrap();
    assert_eq!(stop["cause"], "model_call_without_model_node", "{stop}");
    assert!(std::fs::read_to_string(w.run_dir("r1").join("events.jsonl")).unwrap().contains("\"cap\":\"model_node\""));
}

#[test]
fn a_binding_rewritten_to_something_unreadable_is_refused_too() {
    let w = World::new("rewrite-unreadable");
    bindings(&w, NO_MODEL);
    w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "1", "echo 'format: [unclosed' > bindings/sdc.yaml\n");
    let o = w.run(&zero(&w));
    assert_eq!(code(&o), 13, "{}", stderr(&o));
    assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), 1);
}

#[test]
fn a_handoff_command_is_refused_when_the_copy_was_rewritten_to_name_the_model_tool() {
    let w = World::new("rewrite-handoff");
    let marker = stand_in_model_tool(&w);
    bindings(&w, NO_MODEL);
    let handoff = "{\"action\":\"amend.upstream\",\"case\":\"root\",\"command\":[\"b10x-loom\",\"tree\",\"run\",\"--store\",\"@STORE@\",\"--confinement\",\"local\"],\"decides\":\"Add a second stop, then run again.\",\"kind\":\"Handoff\"}";
    w.fake.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 2, None, &format!("[{handoff}]")), 3);
    w.fake.hook("tree_run", "1", REWRITE_TO_MODEL);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.fake.hook("tree_run", "2", "sdc-node node.write\n");
    w.settings(&json!({"decider": {"kind": "scripted", "answers": [{"approve": true, "cost_micro_usd": 0}]}}));
    let o = w.run(&zero(&w));
    assert_eq!(code(&o), 13, "{}", stderr(&o));
    assert!(!marker.exists(), "the model program started");
    assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), 1, "the printed command was not run");
    assert_eq!(w.outcome("r1")["cause"], "model_call_without_model_node");
}

#[test]
fn an_approval_that_would_run_a_bound_program_is_refused_when_the_copy_was_rewritten_to_name_the_model_tool() {
    // `tree approve` runs the program a person action binds: it is a path that can start a model program
    let w = World::new("rewrite-approve");
    let marker = stand_in_model_tool(&w);
    bindings(&w, NO_MODEL);
    let approval = super::fake::decision_json("scope.approve", "ApproveAction", "person:owner");
    w.fake.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 2, None, &format!("[{approval}]")), 3);
    w.fake.hook("tree_run", "1", REWRITE_TO_MODEL);
    w.fake.hook("tree_approve", "default", "sdc-node node.write\n");
    w.settings(&json!({"decider": {"kind": "scripted", "answers": [{"approve": true, "cost_micro_usd": 0}]}}));
    let o = w.run(&zero(&w));
    assert_eq!(code(&o), 13, "{}", stderr(&o));
    assert!(!marker.exists(), "the model program started");
    assert!(w.fake.calls_of(&["tree", "approve"]).is_empty(), "the approval was not sent to Loom");
    assert_eq!(w.outcome("r1")["cause"], "model_call_without_model_node");
}

#[test]
fn a_model_call_recorded_in_a_run_without_a_model_node_ends_it_naming_the_cause() {
    // The adapter of a model call writes its record into the workspace; the runner reads it after
    // every tree run, so a call that no check could stop is still named and ends the run.
    let w = World::new("recorded-call");
    bindings(&w, NO_MODEL);
    w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "default", &model_call_hook("0.1"));
    let o = w.run(&zero(&w));
    assert_eq!(code(&o), 13, "{}", stderr(&o));
    assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), 1);
    let outcome = w.outcome("r1");
    assert_eq!(outcome["cause"], "model_call_without_model_node", "{outcome}");
    assert!(outcome["reason"].as_str().unwrap().contains("recorded"), "{outcome}");
    assert!(!w.ledger().exists());
}

// --- the run copy

#[test]
fn a_bindings_file_edited_in_the_run_copy_never_lowers_the_caps_of_a_run_opened_with_a_model_node() {
    let w = World::new("copy-edit-keeps-caps");
    w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    // the first program rewrites the copy so that no model node is left in it, and makes a model call
    w.fake.hook("tree_run", "1", &format!("{}cp bindings/sdc.yaml \"$D/before\"\ncat > bindings/sdc.yaml <<'EOF'\nformat: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: check.run\n    executor:\n      kind: program\n      argv: [tools/sdc-tool.sh, check-idea]\nEOF\n", model_call_hook("0.1")));
    w.fake.hook("tree_run", "default", &model_call_hook("0.1"));
    let mut a = w.args("r1", 10_000_000);
    set(&mut a, "--max-llm-calls", "3");
    let o = w.run(&a);
    assert_eq!(code(&o), 6, "the call cap of the opening still stops the run: {}", stderr(&o));
    assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), 3);
    let outcome = w.outcome("r1");
    assert_eq!((outcome["outcome"].as_str(), outcome["llm_calls"].as_u64(), outcome["has_model_node"].as_bool()), (Some("stopped: llm calls"), Some(3), Some(true)), "{outcome}");
    assert_eq!(spent(&w), 300_000, "the calls were reserved and booked through the ledger");
}

#[test]
fn the_decision_is_in_the_records_once_and_the_opening_event_says_why() {
    let w = World::new("record-why");
    bindings(&w, NO_MODEL);
    tree_of_two_steps(&w);
    let o = w.run(&zero(&w));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let because = decided(&w)[0]["because"].as_str().unwrap().to_owned();
    assert!(because.contains("no model node"), "{because}");
    let w = World::new("record-why-model");
    let o = w.run(&w.args("r1", 0));
    assert_eq!(code(&o), 4, "{}", stderr(&o));
    let because = decided(&w)[0]["because"].as_str().unwrap().to_owned();
    assert!(because.contains("sdc-node"), "{because}");
}
