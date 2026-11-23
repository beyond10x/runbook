//! Where a run without a model node needs no caps: only under a confinement that takes the network
//! from bound programs (`substrate`). Under `--confinement local` the same run is refused by name,
//! before the opening, unless it has caps (`docs/loom-run.md`, "A run without a model node"). A
//! fake `b10x-loom` and sh hooks: no model call, no network request.

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

/// The options of run `r1` with every cap at 0 and no ledger, under `level`.
fn zero_under(w: &World, level: &str) -> Vec<String> {
    let mut a = w.args("r1", 0);
    set(&mut a, "--max-llm-calls", "0");
    without(&mut a, "--spend-ledger");
    set(&mut a, "--confinement", level);
    a
}

/// What a refusal before the opening leaves behind: nothing.
fn assert_nothing_written(w: &World, what: &str) {
    assert!(!w.run_dir("r1").exists(), "{what}: a run directory was made");
    assert!(!w.ledger().exists(), "{what}: a ledger file was made");
    assert!(!w.root.join("xdg/runbook").exists(), "{what}: a private directory for the login was made");
    assert_eq!(std::fs::read_dir(w.root.join("runs")).unwrap().count(), 0, "{what}: the runs directory holds something");
    assert_eq!(w.fake.calls_so_far(), 0, "{what}: Loom was called");
}

#[test]
fn under_local_a_run_without_a_model_node_and_without_caps_is_refused_by_name_before_anything_is_written() {
    type Change = fn(&mut Vec<String>);
    let forms: [(&str, Change); 4] = [
        ("every cap at 0 and no ledger", |_| {}),
        ("no ledger only", |a| {
            set(a, "--max-llm-calls", "100");
            set(a, "--cap-micro-usd", "1000000");
        }),
        ("--max-llm-calls 0 only", |a| {
            set(a, "--max-llm-calls", "0");
            set(a, "--cap-micro-usd", "1000000");
            a.extend(["--spend-ledger".into(), "ledger.json".into()]);
        }),
        ("--cap-micro-usd 0 only", |a| {
            set(a, "--max-llm-calls", "100");
            set(a, "--cap-micro-usd", "0");
            a.extend(["--spend-ledger".into(), "ledger.json".into()]);
        }),
    ];
    for (n, (what, change)) in forms.iter().enumerate() {
        let w = world(&format!("local-refused-{n}"));
        two_steps(&w);
        let mut a = zero_under(&w, "local");
        change(&mut a);
        if let Some(i) = a.iter().position(|x| x == "ledger.json") {
            a[i] = w.ledger().display().to_string();
        }
        let o = w.run(&a);
        let said = stderr(&o);
        assert_eq!(code(&o), 2, "{what}: {said}");
        for needle in ["local", "--cap-micro-usd", "--max-llm-calls", "--spend-ledger", "without a model node"] {
            assert!(said.contains(needle), "{what}: the refusal does not name {needle}: {said}");
        }
        assert_nothing_written(&w, what);
    }
}

#[test]
fn under_local_the_refusal_comes_before_a_bound_program_starts() {
    let w = world("local-refused-first");
    two_steps(&w);
    let marker = w.root.join("marker-first-step");
    let touch = format!("touch '{}'\n", marker.display());
    w.fake.hook("tree_run", "1", &touch);
    let o = w.run(&zero_under(&w, "local"));
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(!marker.exists(), "a first step started before the refusal; Loom was not to be called at all");
    assert_nothing_written(&w, "the refusal");
}

#[test]
fn under_local_the_same_run_with_caps_starts_and_counts_nothing() {
    let w = world("local-capped");
    two_steps(&w);
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let outcome = outcome_of(&w);
    assert_eq!((outcome["has_model_node"].as_bool(), outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some(false), Some(0), Some(0)), "{outcome}");
    assert_eq!(spent(&w), 0, "a run that makes no model call booked something");
    let events = std::fs::read_to_string(w.run_dir("r1").join("events.jsonl")).unwrap();
    assert!(events.contains("local") && events.contains("model_node_decided"), "the decision does not name the level: {events}");
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
fn under_substrate_the_same_run_without_caps_starts_and_counts_nothing() {
    let w = world("substrate-zero");
    two_steps(&w);
    let o = w.run(&zero_under(&w, "substrate"));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let outcome = outcome_of(&w);
    assert_eq!((outcome["has_model_node"].as_bool(), outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some(false), Some(0), Some(0)), "{outcome}");
    assert!(!w.ledger().exists(), "a ledger was made");
    let runs = w.fake.calls_of(&["tree", "run"]);
    assert!(!runs.is_empty() && runs.iter().all(|c| c.argv.windows(2).any(|p| p == ["--confinement", "substrate"])), "{:?}", runs.iter().map(|c| c.argv.clone()).collect::<Vec<_>>());
}

#[test]
fn under_substrate_a_recorded_call_in_a_run_without_caps_ends_the_run() {
    let w = world("substrate-recorded");
    two_steps(&w);
    w.fake.hook("tree_run", "1", &model_call_hook("0.4"));
    let o = w.run(&zero_under(&w, "substrate"));
    assert_eq!(code(&o), 13, "{}", stderr(&o));
    assert_eq!(outcome_of(&w)["cause"], "model_call_without_model_node");
}

#[test]
fn a_run_with_a_model_node_is_refused_without_a_ledger_under_both_levels_as_before() {
    for level in ["local", "substrate"] {
        let w = World::new(&format!("with-{level}"));
        two_steps(&w);
        let mut a = w.args("r1", 1_000_000);
        without(&mut a, "--spend-ledger");
        set(&mut a, "--confinement", level);
        let o = w.run(&a);
        assert_eq!(code(&o), 64, "{level}: {}", stderr(&o));
        assert!(stderr(&o).contains("--spend-ledger is required: the run has a model node"), "{level}: {}", stderr(&o));
        assert_nothing_written(&w, level);
    }
}
