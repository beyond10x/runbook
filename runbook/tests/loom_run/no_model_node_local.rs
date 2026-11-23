//! A run without a model node under both confinements: none of the three money options is needed
//! (`docs/loom-run.md`, "A run without a model node"). A fake `b10x-loom` and sh hooks: no model
//! call, no network request.

use super::core::{code, model_call_hook, spent, stderr, World};
use super::fake::tree_run_json;
use serde_json::Value;

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

fn set(a: &mut [String], flag: &str, value: &str) {
    let i = a.iter().position(|x| x == flag).unwrap();
    a[i + 1] = value.to_owned();
}

fn without(a: &mut Vec<String>, flag: &str) {
    let i = a.iter().position(|x| x == flag).unwrap();
    a.drain(i..=i + 1);
}

fn world(tag: &str) -> World {
    let w = World::new(tag);
    std::fs::write(w.root.join("bindings/sdc.yaml"), NO_MODEL).unwrap();
    w
}

fn two_steps(w: &World) {
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
}

fn outcome_of(w: &World) -> Value {
    w.outcome("r1")
}

/// The options of run `r1` with none of the three money options, under `level`.
fn bare_under(w: &World, level: &str) -> Vec<String> {
    let mut a = w.args("r1", 0);
    for flag in ["--spend-ledger", "--cap-micro-usd", "--max-llm-calls"] {
        without(&mut a, flag);
    }
    set(&mut a, "--confinement", level);
    a
}

/// A run with caps under `local` starts and counts nothing when it makes no model call.
#[test]
fn under_local_the_same_run_with_caps_starts_and_counts_nothing() {
    let w = world("local-capped");
    two_steps(&w);
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let outcome = outcome_of(&w);
    assert_eq!((outcome["has_model_node"].as_bool(), outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some(false), Some(0), Some(0)), "{outcome}");
    assert_eq!(spent(&w), 0, "a run that makes no model call booked something");
}

#[test]
fn under_local_with_caps_a_recorded_call_is_booked_and_counted_and_does_not_end_the_run() {
    let w = world("local-capped-recorded");
    two_steps(&w);
    w.fake.hook("tree_run", "1", &model_call_hook("0.4"));
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(spent(&w), 400_000, "the recorded call was not booked in the ledger");
    let outcome = outcome_of(&w);
    assert_eq!((outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some(1), Some(400_000)), "{outcome}");
    assert_ne!(outcome["cause"].as_str(), Some("model_call_without_model_node"), "{outcome}");
}

#[test]
fn under_local_with_caps_a_cap_that_a_recorded_call_reaches_stops_the_run() {
    let w = world("local-capped-cap");
    two_steps(&w);
    w.fake.hook("tree_run", "1", &model_call_hook("0.4"));
    let mut a = w.args("r1", 5_000_000);
    set(&mut a, "--max-llm-calls", "1");
    let o = w.run(&a);
    assert_eq!(code(&o), 6, "{}", stderr(&o));
    assert_eq!(outcome_of(&w)["outcome"], "stopped: llm calls");
}

#[test]
fn under_substrate_the_same_run_without_the_money_options_starts_and_counts_nothing() {
    let w = world("substrate-zero");
    two_steps(&w);
    let o = w.run(&bare_under(&w, "substrate"));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let outcome = outcome_of(&w);
    assert_eq!((outcome["has_model_node"].as_bool(), outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some(false), Some(0), Some(0)), "{outcome}");
    assert!(!w.ledger().exists(), "a ledger was made");
    let runs = w.fake.calls_of(&["tree", "run"]);
    assert!(!runs.is_empty() && runs.iter().all(|c| c.argv.windows(2).any(|p| p == ["--confinement", "substrate"])), "{:?}", runs.iter().map(|c| c.argv.clone()).collect::<Vec<_>>());
}

