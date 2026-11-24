//! The three money options are optional (`docs/loom-run.md`, "The money options"): none named is no limit and
//! no refusal; a cap that is named stops the run when it is reached, a cap of 0 spends nothing.
//! A fake `b10x-loom` and stand-ins for the model: no model call, no network request.

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
";

fn without(a: &mut Vec<String>, flag: &str) {
    let i = a.iter().position(|x| x == flag).unwrap();
    a.drain(i..=i + 1);
}

fn set(a: &mut [String], flag: &str, value: &str) {
    let i = a.iter().position(|x| x == flag).unwrap();
    a[i + 1] = value.to_owned();
}

/// The options of run `r1` under `level` with none of the three money options.
fn bare(w: &World, level: &str) -> Vec<String> {
    let mut a = w.args("r1", 0);
    for flag in ["--spend-ledger", "--cap-micro-usd", "--max-llm-calls"] {
        without(&mut a, flag);
    }
    set(&mut a, "--confinement", level);
    a
}

/// `calls` tree runs, each one model call costing `cost` USD, then the last one closes the tree.
fn calls_then_close(w: &World, calls: u32, cost: &str) {
    for n in 1..=calls {
        w.fake.answer("tree_run", &n.to_string(), &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
        w.fake.hook("tree_run", &n.to_string(), &model_call_hook(cost));
    }
    w.fake.answer("tree_run", &(calls + 1).to_string(), &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
}

fn events(w: &World) -> Vec<Value> {
    let text = std::fs::read_to_string(w.run_dir("r1").join("events.jsonl")).unwrap();
    text.lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

#[test]
fn a_run_with_a_model_node_and_none_of_the_three_options_starts_and_ends_closed_under_every_level() {
    for level in ["local", "substrate"] {
        let w = World::new(&format!("bare-{level}"));
        calls_then_close(&w, 2, "0.6");
        let o = w.run(&bare(&w, level));
        assert_eq!(code(&o), 0, "{level}: {}", stderr(&o));
        let outcome = w.outcome("r1");
        assert_eq!(outcome["outcome"], "ended", "{level}: {outcome}");
        assert_eq!((outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some(2), Some(1_200_000)), "{level}: the calls made are stated: {outcome}");
        assert_eq!(outcome["has_model_node"], true, "{level}: {outcome}");
        assert!(!w.ledger().exists(), "{level}: no ledger is opened");
        assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), 3, "{level}");
        let started = events(&w).into_iter().find(|e| e["event"] == "run_started").unwrap();
        assert!(started["settings"]["loom"]["cap_micro_usd"].is_null() && started["settings"]["max_llm_calls"].is_null(), "{level}: a cap nobody named is recorded as null: {started}");
    }
}

#[test]
fn a_call_is_given_the_budget_the_bindings_name_when_no_cap_is_named() {
    let w = World::new("bare-budget");
    calls_then_close(&w, 1, "0.6");
    let o = w.run(&bare(&w, "local"));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let copy = std::fs::read_to_string(w.run_dir("r1").join("bindings/sdc.yaml")).unwrap();
    assert!(copy.contains("'1'") || copy.contains("\"1\"") || copy.contains("- 1\n"), "the per-call budget of the bindings is not lowered: {copy}");
}

#[test]
fn a_run_without_a_model_node_and_none_of_the_three_options_starts_under_local_and_substrate() {
    for level in ["local", "substrate"] {
        let w = World::new(&format!("bare-nomodel-{level}"));
        std::fs::write(w.root.join("bindings/sdc.yaml"), NO_MODEL).unwrap();
        w.fake.answer("tree_run", "1", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
        let o = w.run(&bare(&w, level));
        assert_eq!(code(&o), 0, "{level}: {}", stderr(&o));
        let outcome = w.outcome("r1");
        assert_eq!((outcome["outcome"].as_str(), outcome["has_model_node"].as_bool()), (Some("ended"), Some(false)), "{level}: {outcome}");
        assert!(!w.ledger().exists(), "{level}");
    }
}

#[test]
fn caps_of_0_under_local_start_and_stop_at_the_first_model_call() {
    // a spend cap of 0: the run starts (run directory, opening events) and the first call stops it
    let w = World::new("zero-spend-local");
    calls_then_close(&w, 1, "0.6");
    let mut a = w.args("r1", 0);
    without(&mut a, "--spend-ledger");
    let o = w.run(&a);
    assert_eq!(code(&o), 4, "{}", stderr(&o));
    assert!(w.fake.calls_of(&["tree", "run"]).is_empty(), "no call was started");
    let outcome = w.outcome("r1");
    assert_eq!((outcome["outcome"].as_str(), outcome["spent_micro_usd"].as_u64()), (Some("stopped: spend cap"), Some(0)), "{outcome}");
    assert!(events(&w).iter().any(|e| e["event"] == "cap_reached" && e["cap"] == "spend"));
    // both caps at 0 under local with a model-free tree: the run starts and the call cap ends it
    let w = World::new("zero-both-local");
    std::fs::write(w.root.join("bindings/sdc.yaml"), NO_MODEL).unwrap();
    w.fake.answer("tree_run", "1", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    let mut a = w.args("r1", 0);
    without(&mut a, "--spend-ledger");
    set(&mut a, "--max-llm-calls", "0");
    let o = w.run(&a);
    assert_eq!(code(&o), 6, "{}", stderr(&o));
    assert!(w.run_dir("r1").join("outcome.json").is_file());
    assert_eq!(w.outcome("r1")["outcome"], "stopped: llm calls");
}

#[test]
fn a_cap_without_a_ledger_is_counted_in_the_run_and_stops_it_at_the_cap() {
    let w = World::new("cap-no-ledger");
    calls_then_close(&w, 3, "0.6");
    let mut a = bare(&w, "local");
    a.extend(["--cap-micro-usd".to_owned(), "1000000".to_owned()]);
    let o = w.run(&a);
    assert_eq!(code(&o), 4, "{}", stderr(&o));
    assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), 2, "no third call after the cap");
    let outcome = w.outcome("r1");
    assert_eq!((outcome["outcome"].as_str(), outcome["spent_micro_usd"].as_u64(), outcome["llm_calls"].as_u64()), (Some("stopped: spend cap"), Some(1_000_000), Some(2)), "0.6, then 0.4 (what the cap left): {outcome}");
    assert!(!w.ledger().exists(), "the cap is counted in the run: no file");
}

#[test]
fn a_cap_named_in_the_settings_file_alone_is_the_cap_of_the_run() {
    let w = World::new("cap-in-settings");
    calls_then_close(&w, 3, "0.6");
    w.settings(&json!({"cap_micro_usd": 1_000_000}));
    let o = w.run(&bare(&w, "local"));
    assert_eq!(code(&o), 4, "{}", stderr(&o));
    assert_eq!(w.outcome("r1")["spent_micro_usd"], 1_000_000);
    let started = events(&w).into_iter().find(|e| e["event"] == "run_started").unwrap();
    assert_eq!(started["settings"]["loom"]["cap_micro_usd"], 1_000_000, "{started}");
    // a member beside the option must still equal it
    w.settings(&json!({"cap_micro_usd": 1_000_000}));
    let mut a = w.args("r2", 2_000_000);
    without(&mut a, "--spend-ledger");
    let o = w.run(&a);
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(stderr(&o).contains("cap_micro_usd") && !w.run_dir("r2").exists(), "{}", stderr(&o));
    // the call cap by its member
    let w = World::new("calls-in-settings");
    calls_then_close(&w, 3, "0.1");
    w.settings(&json!({"max_llm_calls": 2}));
    let o = w.run(&bare(&w, "local"));
    assert_eq!(code(&o), 6, "{}", stderr(&o));
    assert_eq!(w.outcome("r1")["llm_calls"], 2);
}

#[test]
fn a_call_cap_alone_stops_the_run_without_a_ledger_and_without_a_spend_cap() {
    let w = World::new("calls-alone");
    calls_then_close(&w, 3, "0.1");
    let mut a = bare(&w, "local");
    a.extend(["--max-llm-calls".to_owned(), "2".to_owned()]);
    let o = w.run(&a);
    assert_eq!(code(&o), 6, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!((outcome["outcome"].as_str(), outcome["llm_calls"].as_u64()), (Some("stopped: llm calls"), Some(2)), "{outcome}");
    assert!(!w.ledger().exists());
}

#[test]
fn a_ledger_alone_records_the_spend_and_stops_nothing() {
    let w = World::new("ledger-alone");
    calls_then_close(&w, 3, "0.6");
    let mut a = bare(&w, "local");
    a.extend(["--spend-ledger".to_owned(), w.ledger().display().to_string()]);
    let o = w.run(&a);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(spent(&w), 1_800_000, "three calls of 0.6, booked");
    assert_eq!(w.outcome("r1")["spent_micro_usd"], 1_800_000);
    assert!(w.outcome("r1")["outcome"] == "ended");
}

#[test]
fn a_ledger_and_a_cap_still_stop_the_run_as_before() {
    let w = World::new("ledger-and-cap");
    calls_then_close(&w, 3, "0.6");
    let mut a = bare(&w, "local");
    a.extend(["--spend-ledger".to_owned(), w.ledger().display().to_string(), "--cap-micro-usd".to_owned(), "1000000".to_owned()]);
    let o = w.run(&a);
    assert_eq!(code(&o), 4, "{}", stderr(&o));
    assert_eq!(spent(&w), 1_000_000);
}
