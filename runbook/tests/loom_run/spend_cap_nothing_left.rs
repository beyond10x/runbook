//! A spend cap that has nothing left starts nothing (`docs/loom-run.md`, "What the spend cap
//! promises"): no `tree run`, no handoff and no `tree approve`, whatever the bindings name, and the run ends
//! `stopped: spend cap` (exit 4) with the decision it holds still open. Only a named cap does this: a run
//! that names none is never stopped by it. A fake `b10x-loom` and stand-ins for the model: no model call,
//! no network request.

use super::core::{code, model_call_hook, stderr, World};
use super::fake::{decision_json, tree_run_json};
use serde_json::{json, Value};

/// A model node whose argv names no per-call budget.
const MODEL_NO_BUDGET: &str = "format: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: node.write\n    executor:\n      kind: program\n      argv:\n        - sdc-node\n        - node.write\n";

/// A model node with a per-call budget of 1 USD.
const MODEL_BUDGET: &str = "format: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: node.write\n    executor:\n      kind: program\n      argv:\n        - sdc-node\n        - node.write\n        - --max-budget-usd\n        - \"1\"\n";

/// A tree with no model tool in it.
const NO_MODEL: &str = "format: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: check.run\n    executor:\n      kind: program\n      argv: [tools/sdc-tool.sh, check-idea]\n";

const HANDOFF: &str = "{\"action\":\"amend.upstream\",\"case\":\"root\",\"command\":[\"b10x-loom\",\"tree\",\"run\",\"--store\",\"@STORE@\",\"--confinement\",\"local\"],\"decides\":\"Add a second stop, then run again.\",\"kind\":\"Handoff\"}";

fn bindings(w: &World, text: &str) {
    std::fs::write(w.root.join("bindings/sdc.yaml"), text).unwrap();
}

fn without(a: &mut Vec<String>, flag: &str) {
    let i = a.iter().position(|x| x == flag).unwrap();
    a.drain(i..=i + 1);
}

fn tree_runs(w: &World) -> usize {
    w.fake.calls_of(&["tree", "run"]).len()
}

fn approves(w: &World) -> usize {
    w.fake.calls_of(&["tree", "approve"]).len()
}

fn events(w: &World) -> Vec<Value> {
    let text = std::fs::read_to_string(w.run_dir("r1").join("events.jsonl")).unwrap();
    text.lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

fn has_event(w: &World, name: &str) -> bool {
    events(w).iter().any(|e| e["event"] == name)
}

fn spend_cap_reached(w: &World) -> bool {
    events(w).iter().any(|e| e["event"] == "cap_reached" && e["cap"] == "spend")
}

/// The scripted decider that costs nothing and approves.
fn free_approver(w: &World) {
    w.settings(&json!({"decider": {"kind": "scripted", "max_call_micro_usd": 0, "answers": [{"approve": true, "principal": "person:owner", "cost_micro_usd": 0, "model_calls": 0}]}}));
}

/// Step 1 books one call of 0.2 USD and prints `decisions`; every later step ends the tree.
fn first_step_books_a_call_and_prints(w: &World, decisions: &str) {
    w.fake.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 1, None, &format!("[{decisions}]")), 3);
    w.fake.hook("tree_run", "1", &model_call_hook("0.2"));
    w.fake.answer("tree_approve", "default", "{\"decision\":\"Allow\"}\n", 0);
    w.fake.answer("tree_run", "default", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
}

/// A cap above 0 that the earlier steps booked whole has nothing left: for a model tool that names its
/// budget option and for one that does not, no second `tree run` starts.
#[test]
fn a_cap_above_0_that_earlier_steps_booked_whole_starts_no_further_tree_run() {
    for (n, text) in [MODEL_BUDGET, MODEL_NO_BUDGET].iter().enumerate() {
        let w = World::new(&format!("scnl-booked-{n}"));
        bindings(&w, text);
        for k in 1..=3u32 {
            w.fake.answer("tree_run", &k.to_string(), &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
            w.fake.hook("tree_run", &k.to_string(), &model_call_hook("0.15"));
        }
        let o = w.run(&w.args("r1", 150_000));
        assert_eq!(code(&o), 4, "variant {n}: {}", stderr(&o));
        assert_eq!(tree_runs(&w), 1, "variant {n}: a tree run started with nothing left");
        let outcome = w.outcome("r1");
        assert_eq!((outcome["outcome"].as_str(), outcome["spent_micro_usd"].as_u64(), outcome["llm_calls"].as_u64()), (Some("stopped: spend cap"), Some(150_000), Some(1)), "variant {n}: {outcome}");
        assert!(spend_cap_reached(&w), "variant {n}");
    }
}

/// The same with no ledger: the cap is counted in the run itself.
#[test]
fn a_cap_counted_in_the_run_itself_that_is_booked_whole_starts_no_further_tree_run() {
    let w = World::new("scnl-booked-noledger");
    bindings(&w, MODEL_NO_BUDGET);
    for k in 1..=3u32 {
        w.fake.answer("tree_run", &k.to_string(), &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
        w.fake.hook("tree_run", &k.to_string(), &model_call_hook("0.2"));
    }
    let mut a = w.args("r1", 150_000);
    without(&mut a, "--spend-ledger");
    let o = w.run(&a);
    assert_eq!((code(&o), tree_runs(&w)), (4, 1), "{}", stderr(&o));
}

/// A run that has a model node by another route than the bindings (here a decider that may call a model;
/// a re-drive of a recording and `--conformance observe` take the same path in `model_node::decide`) and a
/// copy that names no model tool: a cap of 0 still starts no `tree run`.
#[test]
fn a_cap_of_0_starts_no_tree_run_of_a_run_that_has_a_model_node_by_its_decider() {
    let w = World::new("scnl-zero-decider");
    bindings(&w, NO_MODEL);
    w.fake.answer("tree_run", "default", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.settings(&json!({"decider": {"kind": "scripted", "max_call_micro_usd": 100_000, "answers": [{"approve": true, "principal": "person:owner", "cost_micro_usd": 0, "model_calls": 0}]}}));
    let o = w.run(&w.args("r1", 0));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["has_model_node"], true, "{outcome}");
    assert_eq!((code(&o), tree_runs(&w)), (4, 0), "{}", stderr(&o));
    assert_eq!(outcome["outcome"], "stopped: spend cap", "{outcome}");
}

/// A cap of 0 and a run in which nothing can spend (no model tool in the copy, no decider that may call a
/// model, nothing booked): the run starts and ends as it would with no cap. "Spend nothing" is kept by it
/// spending nothing; the guard stops only where something could be spent.
#[test]
fn a_cap_of_0_does_not_stop_a_run_in_which_nothing_can_spend() {
    let w = World::new("scnl-zero-nothing");
    bindings(&w, NO_MODEL);
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    let mut a = w.args("r1", 0);
    without(&mut a, "--spend-ledger");
    let o = w.run(&a);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!((outcome["outcome"].as_str(), outcome["has_model_node"].as_bool(), outcome["spent_micro_usd"].as_u64()), (Some("ended"), Some(false), Some(0)), "{outcome}");
    assert_eq!(tree_runs(&w), 2);
}

/// The open decision stays open: it is asked, never answered, `tree approve` is not called, and the
/// outcome and the events say the cap ended the run.
#[test]
fn after_the_cap_is_passed_the_decision_stays_open_and_the_records_say_the_cap_ended_the_run() {
    for with_node in [true, false] {
        let w = World::new(&format!("scnl-open-{with_node}"));
        bindings(&w, if with_node { MODEL_NO_BUDGET } else { NO_MODEL });
        first_step_books_a_call_and_prints(&w, &decision_json("owner.approve", "ApproveAction", "person:owner"));
        free_approver(&w);
        let o = w.run(&w.args("r1", 150_000));
        assert_eq!((code(&o), approves(&w), tree_runs(&w)), (4, 0, 1), "model node {with_node}: {}", stderr(&o));
        let outcome = w.outcome("r1");
        assert_eq!((outcome["outcome"].as_str(), outcome["exit_code"].as_i64()), (Some("stopped: spend cap"), Some(4)), "{outcome}");
        assert!(has_event(&w, "decision_asked") && !has_event(&w, "decision_answered"), "model node {with_node}: the decision must be asked and open");
        assert!(spend_cap_reached(&w), "model node {with_node}");
    }
}

/// A close that the decider approves is not answered either: `tree approve` runs a program.
#[test]
fn after_the_cap_is_passed_a_close_is_not_approved() {
    let w = World::new("scnl-close");
    bindings(&w, MODEL_BUDGET);
    first_step_books_a_call_and_prints(&w, &decision_json("case.close", "Close", "person:owner"));
    free_approver(&w);
    let o = w.run(&w.args("r1", 150_000));
    assert_eq!((code(&o), approves(&w)), (4, 0), "{}", stderr(&o));
}

/// A handoff that the step printed is not run once the cap is passed: its printed `tree run` is a call
/// that starts programs.
#[test]
fn after_the_cap_is_passed_the_printed_command_of_a_handoff_is_not_run() {
    for with_node in [true, false] {
        let w = World::new(&format!("scnl-handoff-{with_node}"));
        bindings(&w, if with_node { MODEL_NO_BUDGET } else { NO_MODEL });
        first_step_books_a_call_and_prints(&w, HANDOFF);
        free_approver(&w);
        let o = w.run(&w.args("r1", 150_000));
        assert_eq!((code(&o), tree_runs(&w)), (4, 1), "model node {with_node}: {}", stderr(&o));
        assert!(!has_event(&w, "decision_answered"), "model node {with_node}");
    }
}

/// A decider that costs something after the cap is passed is stopped before it is asked (as before).
#[test]
fn a_decider_that_costs_something_is_not_asked_after_the_cap_is_passed() {
    let w = World::new("scnl-decider-costs");
    bindings(&w, MODEL_BUDGET);
    first_step_books_a_call_and_prints(&w, &decision_json("owner.approve", "ApproveAction", "person:owner"));
    w.settings(&json!({"decider": {"kind": "scripted", "max_call_micro_usd": 100_000, "answers": [{"approve": true, "principal": "person:owner", "cost_micro_usd": 100_000, "model_calls": 1}]}}));
    let o = w.run(&w.args("r1", 150_000));
    assert_eq!((code(&o), approves(&w)), (4, 0), "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!((outcome["outcome"].as_str(), outcome["spent_micro_usd"].as_u64(), outcome["llm_calls"].as_u64()), (Some("stopped: spend cap"), Some(150_000), Some(1)), "{outcome}");
}

/// No cap named, no limit: the same step books past what would have been a cap, and the decision it printed
/// is answered. A guard that treated "no cap" as "nothing left" would fail this case, and
/// `a_ledger_alone_records_the_spend_and_stops_nothing` of `caps_optional.rs`.
#[test]
fn with_no_cap_named_a_decision_after_a_costly_step_is_answered() {
    let w = World::new("scnl-nocap");
    bindings(&w, MODEL_NO_BUDGET);
    first_step_books_a_call_and_prints(&w, &decision_json("owner.approve", "ApproveAction", "person:owner"));
    free_approver(&w);
    let mut a = w.args("r1", 0);
    for flag in ["--spend-ledger", "--cap-micro-usd", "--max-llm-calls"] {
        without(&mut a, flag);
    }
    let o = w.run(&a);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(approves(&w), 1, "the decision was not answered");
    assert!(!spend_cap_reached(&w));
}
