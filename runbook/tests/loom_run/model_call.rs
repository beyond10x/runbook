//! A model call that fails. The driver does not end the run for a call with exit other than 0 and
//! no cost: the step's retry rule decides (`driver/src/main.rs`: `program_error`, `max_runs`,
//! `writer_spent`), except for the exits 126, 127 and 75, where the tool could not run. The runner
//! does the same, and books the reservation of a call whose cost it cannot read, as it always did.
//! Where the run does end because of a call, the reason names the action, the adapter, the exit and
//! the first line of the adapter's stderr.

use super::core::{code, spent, stderr, World};
use super::fake::tree_run_json;
use serde_json::Value;

/// A `tree_run` hook: one model call of `idea.dissect` on the adapter `metaharness` that ends with
/// `exit`, a `cost_usd` of `cost` (JSON text) and the given text in its `stderr.txt`.
fn failed_call_hook(exit: i32, cost: &str, stderr_text: &str) -> String {
    format!(
        "w=work/.sdc/llm; mkdir -p \"$w\"\n\
i=$(( $(find \"$w\" -mindepth 1 -maxdepth 1 -type d | wc -l) + 1 ))\n\
d=$(printf '%03d-idea.dissect' $i); mkdir -p \"$w/$d\"\n\
printf '%s' '{stderr_text}' > \"$w/$d/stderr.txt\"\n\
printf '{{\"action\":\"idea.dissect\",\"adapter\":\"metaharness\",\"exit\":{exit},\"cost_usd\":{cost},\"record\":\"%s/%s\"}}\\n' \"$PWD/$w\" \"$d\" >> \"$w/calls.jsonl\"\n"
    )
}

fn reason_of(w: &World) -> String {
    w.outcome("r1")["reason"].as_str().unwrap().to_owned()
}

#[test]
fn a_failed_call_without_a_cost_books_its_reservation_and_the_run_goes_on() {
    let w = World::new("failed-goes-on");
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "1", &failed_call_hook(2, "null", "metaharness: the adapter refused to plan the launch"));
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), 2, "the run stopped after the failed call");
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "ended", "{outcome}");
    assert_eq!(spent(&w), 1_000_000, "a call without a cost books its reservation, the largest per-call budget of the bindings");
    assert_eq!((outcome["spent_micro_usd"].as_u64(), outcome["llm_calls"].as_u64()), (Some(1_000_000), Some(1)), "{outcome}");
    let line = std::fs::read_to_string(w.run_dir("r1").join("llm-calls.jsonl")).unwrap();
    let call: Value = serde_json::from_str(line.lines().next().unwrap()).unwrap();
    assert_eq!((call["exit"].as_i64(), call["cost_usd"].as_f64(), call["cost_unknown"].as_bool()), (Some(2), Some(1.0), Some(true)), "{call}");
}

#[test]
fn failed_calls_without_a_cost_run_into_the_spend_cap_and_stop_there() {
    let w = World::new("failed-cap");
    w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "default", &failed_call_hook(2, "null", "refused"));
    let o = w.run(&w.args("r1", 2_500_000));
    assert_eq!(code(&o), 4, "{}", stderr(&o));
    assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), 3, "1.0, 1.0 and the 0.5 that remained, then none");
    assert_eq!(spent(&w), 2_500_000);
    assert_eq!(w.outcome("r1")["outcome"], "stopped: spend cap");
}

#[test]
fn a_failed_call_with_the_exit_of_a_program_that_could_not_run_ends_the_run_naming_its_cause() {
    for exit in [126, 127, 75] {
        let w = World::new(&format!("failed-exit{exit}"));
        w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
        w.fake.hook("tree_run", "default", &failed_call_hook(exit, "null", "no login at /marker-home-dir/.claude/.credentials.json: not there"));
        let o = w.run(&w.args("r1", 5_000_000));
        assert_eq!(code(&o), 1, "{}", stderr(&o));
        assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), 1);
        let outcome = w.outcome("r1");
        assert_eq!(outcome["outcome"], "error");
        assert_eq!(
            reason_of(&w),
            format!("model step idea.dissect: adapter metaharness exited {exit}: no login at ~/.claude/.credentials.json: not there"),
        );
        assert_eq!(spent(&w), 1_000_000, "the reservation of the call whose cost is unknown is booked");
    }
}

#[test]
fn a_successful_call_without_a_cost_ends_the_run_naming_the_call() {
    let w = World::new("exit0-null");
    w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "default", &failed_call_hook(0, "null", ""));
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 1, "{}", stderr(&o));
    assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), 1);
    assert_eq!(
        reason_of(&w),
        "model step idea.dissect: adapter metaharness exited 0 and recorded no cost (cost_usd is not a number: null)",
    );
    assert_eq!(spent(&w), 1_000_000);
}

#[test]
fn the_reason_holds_the_first_line_of_the_stderr_cut_at_300_characters() {
    let w = World::new("reason-cut");
    w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    let long = "e".repeat(500);
    w.fake.hook("tree_run", "default", &failed_call_hook(127, "null", &format!("{long}\nsecond line")));
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 1, "{}", stderr(&o));
    let reason = reason_of(&w);
    assert!(reason.ends_with(&format!(": {}", "e".repeat(300))), "{reason}");
    assert!(!reason.contains("second line"), "{reason}");
}

#[test]
fn the_reason_says_so_when_the_call_left_no_stderr() {
    let w = World::new("reason-empty");
    w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "default", &failed_call_hook(127, "null", ""));
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 1, "{}", stderr(&o));
    assert_eq!(reason_of(&w), "model step idea.dissect: adapter metaharness exited 127: (no stderr)");
}

#[test]
fn a_failed_call_with_a_cost_is_booked_at_that_cost_and_the_run_goes_on() {
    let w = World::new("failed-with-cost");
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "1", &failed_call_hook(1, "0.25", "the model said no"));
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(spent(&w), 250_000);
}
