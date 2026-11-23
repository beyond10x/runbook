//! The decision "has this run a model node" is a record and a gate for the login link, and no more
//! (`docs/loom-run.md`, "A run without a model node"): it refuses nothing and limits nothing, so a run
//! with or without a model node takes the money options as every run does. A fake `b10x-loom`, sh
//! programs and stand-ins for the model tool: no model call, no network request.

use super::core::{code, model_call_hook, spent, stderr, World};
use super::fake::tree_run_json;
use serde_json::{json, Value};

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

/// The options of run `r1` with none of the three money options, under `substrate` (the fake
/// `b10x-loom` takes the level as an argument and does not enforce it).
fn bare(w: &World) -> Vec<String> {
    let mut a = w.args("r1", 0);
    for flag in ["--spend-ledger", "--cap-micro-usd", "--max-llm-calls"] {
        without(&mut a, flag);
    }
    set(&mut a, "--confinement", "substrate");
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

#[test]
fn a_run_without_a_model_node_and_without_the_money_options_records_the_decision_once() {
    let w = World::new("bare-no-model");
    bindings(&w, NO_MODEL);
    tree_of_two_steps(&w);
    let o = w.run(&bare(&w));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), 2);
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "ended", "{outcome}");
    assert_eq!((outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some(0), Some(0)), "{outcome}");
    assert_eq!(outcome["has_model_node"], false, "{outcome}");
    assert!(!w.ledger().exists(), "no ledger is opened when none is named");
    let opening = decided(&w);
    assert_eq!(opening.len(), 1, "the decision is written once");
    assert_eq!(opening[0]["has_model_node"], false);
    assert_eq!(events(&w)[1]["event"], "model_node_decided", "it is the event after run_started: {:?}", events(&w).iter().take(3).collect::<Vec<_>>());
}

#[test]
fn a_run_without_a_model_node_given_a_ledger_opens_it_and_books_nothing() {
    let w = World::new("no-model-ledger-given");
    bindings(&w, NO_MODEL);
    tree_of_two_steps(&w);
    let mut a = bare(&w);
    a.extend(["--spend-ledger".to_owned(), w.ledger().display().to_string()]);
    let o = w.run(&a);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(w.ledger().exists(), "a ledger that is named is the run's ledger, whatever the run is");
    assert_eq!(spent(&w), 0);
    assert_eq!(w.outcome("r1")["spent_micro_usd"], 0);
}

#[test]
fn a_tree_run_is_not_a_call_and_a_call_cap_counts_the_recorded_calls_only() {
    let w = World::new("not-a-call");
    bindings(&w, NO_MODEL);
    w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.answer("tree_run", "4", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    let mut a = w.args("r1", 1_000_000);
    set(&mut a, "--max-llm-calls", "1");
    set(&mut a, "--confinement", "substrate");
    let o = w.run(&a);
    assert_eq!(code(&o), 0, "four tree runs against a cap of 1 call: {}", stderr(&o));
    assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), 4);
    assert_ne!(w.outcome("r1")["outcome"], "stopped: llm calls");
    assert_eq!(spent(&w), 0, "nothing was recorded, so nothing was spent");
}

#[test]
fn the_wall_time_bound_stays_for_a_run_without_a_model_node() {
    let w = World::new("zero-wall");
    bindings(&w, NO_MODEL);
    tree_of_two_steps(&w);
    let mut a = bare(&w);
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
fn the_forms_that_count_as_a_model_node_are_recorded_and_start_without_a_ledger() {
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
        ("actions that are not a list", "format: loom.bindings/1\nactions: {a: b}\n".to_owned()),
    ];
    for (what, text) in &forms {
        let w = World::new("forms");
        bindings(&w, text);
        tree_of_two_steps(&w);
        let o = w.run(&bare(&w));
        assert_eq!(code(&o), 0, "{what}: counts as a model node and starts without a ledger: {}", stderr(&o));
        assert_eq!(w.outcome("r1")["has_model_node"], true, "{what}");
        assert!(!w.ledger().exists(), "{what}: no ledger was named, none was opened");
    }
    // a model node named in another bindings file, and in the answers file
    for (what, file, text) in [
        ("another bindings file", "bindings/sdc-intake.yaml", NAMES_MODEL),
        ("the answers file", "bindings/tools/answers.yaml", "format: runbook.loom-run-answers/1\nanswers:\n- action: x\n  on: Handoff\n  requests: [sdc-node, r]\n  apply: [tools/a.sh]\n"),
    ] {
        let w = World::new("forms-other");
        bindings(&w, NO_MODEL);
        tree_of_two_steps(&w);
        std::fs::create_dir_all(w.root.join("bindings/tools")).unwrap();
        std::fs::write(w.root.join(file), text).unwrap();
        let o = w.run(&bare(&w));
        assert_eq!(code(&o), 0, "{what}: {}", stderr(&o));
        assert_eq!(w.outcome("r1")["has_model_node"], true, "{what}");
    }
}

#[test]
fn the_forms_that_do_not_count_are_recorded_as_no_model_node() {
    let programs = "format: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: a.b\n    executor:\n      kind: program\n      argv: [tools/sdc-tool.sh, check-idea, --note, node]\n  - action: c.d\n    executor: {kind: program, argv: [sh, -c, \"echo hello\"]}\n  - action: e.f\n    executor: {kind: person, principal: person:owner}\n";
    for text in [NO_MODEL, programs, "format: loom.bindings/1\n"] {
        let w = World::new("not-forms");
        bindings(&w, text);
        tree_of_two_steps(&w);
        let o = w.run(&bare(&w));
        assert_eq!(code(&o), 0, "{text}: {}", stderr(&o));
        assert_eq!(w.outcome("r1")["has_model_node"], false, "{text}");
    }
}

#[test]
fn a_decider_that_calls_a_model_counts_as_a_model_node() {
    let w = World::new("model-decider");
    bindings(&w, NO_MODEL);
    w.settings(&json!({"autonomy": "A5", "decider": {"kind": "model", "model": "m", "adapter": "claude-cli", "without_claims": true}}));
    tree_of_two_steps(&w);
    let o = w.run(&bare(&w));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(w.outcome("r1")["has_model_node"], true);
    // a decider of a rule calls none
    let w = World::new("rule-decider");
    bindings(&w, NO_MODEL);
    w.settings(&json!({"autonomy": "A5", "decider": {"kind": "rule", "verdict": "approve"}}));
    tree_of_two_steps(&w);
    let o = w.run(&bare(&w));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(w.outcome("r1")["has_model_node"], false);
    // a scripted decider that reserves an amount does
    let w = World::new("scripted-decider");
    bindings(&w, NO_MODEL);
    w.settings(&json!({"decider": {"kind": "scripted", "max_call_micro_usd": 5, "answers": []}}));
    tree_of_two_steps(&w);
    let o = w.run(&bare(&w));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(w.outcome("r1")["has_model_node"], true);
}

#[test]
fn a_model_call_recorded_in_a_run_without_a_model_node_is_booked_and_counted_like_any() {
    let w = World::new("recorded-call-booked");
    bindings(&w, NO_MODEL);
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "1", &model_call_hook("0.1"));
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    let mut a = w.args("r1", 1_000_000);
    set(&mut a, "--confinement", "substrate");
    let o = w.run(&a);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!((outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some(1), Some(100_000)), "{outcome}");
    assert_eq!(spent(&w), 100_000);
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
    let o = w.run(&bare(&w));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let because = decided(&w)[0]["because"].as_str().unwrap().to_owned();
    assert!(because.contains("no model node"), "{because}");
    let w = World::new("record-why-model");
    let o = w.run(&w.args("r1", 0));
    assert_eq!(code(&o), 4, "{}", stderr(&o));
    let because = decided(&w)[0]["because"].as_str().unwrap().to_owned();
    assert!(because.contains("sdc-node"), "{because}");
}
