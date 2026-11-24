//! The caps of a run without a model node: under `local`, a cap that is named ends a run without a
//! model node as it ends a run with one. A fake `b10x-loom`, no model call, no network request.

use super::core::{code, model_call_hook, stderr, World};
use super::fake::tree_run_json;
use serde_json::Value;
use std::process::Output;

pub(super) const NO_MODEL: &str = "format: loom.bindings/1
protocol: sdc@8
actions:
  - action: check.run
    executor:
      kind: program
      argv: [tools/sdc-tool.sh, check-idea]
  - action: scope.approve
    executor: {kind: person, principal: person:owner}
";

pub(super) fn set(a: &mut [String], flag: &str, value: &str) {
    let i = a.iter().position(|x| x == flag).unwrap();
    a[i + 1] = value.to_owned();
}

pub(super) fn last_line(o: &Output) -> String {
    stderr(o).lines().last().unwrap_or("").to_owned()
}

/// Under `local` with caps, the caps end a run without a model node at the same line as a run with
/// one: the same options and the same recorded calls give the same exit, the same outcome and the
/// same count of calls. The amount booked is the same while the caps are not reached (100000 for one
/// recorded call of 0.1 USD, 200000 for two). At a cap it can differ: the runner lowers the
/// `--max-budget-usd` of a declared model tool to what the cap has left, and an undeclared program has
/// no such option. What one step records is booked whole, and the runner bounds neither the amount nor
/// the count of what one step records. Here each step records one call, so the figures differ by one
/// call (cap 150000: 150000 with a model node, 200000 without; cap 50000: 50000 and 100000). The case
/// `under_local_what_one_step_records_is_booked_whole_and_no_tree_run_follows_the_cap` of
/// `no_model_node_refusals_and_booking.rs` holds the whole booking. `docs/loom-run.md` says this.
#[test]
fn under_local_the_caps_end_a_run_without_a_model_node_as_they_end_a_run_with_one() {
    let mut wrong = Vec::new();
    // (cap, calls, booked by the run with a model node, booked by the run without one)
    for (n, (cap, calls, booked_with, booked_without)) in [(10_000_000u64, 1u64, 100_000u64, 100_000u64), (10_000_000, 2, 200_000, 200_000), (150_000, 100, 150_000, 200_000), (50_000, 100, 50_000, 100_000)].iter().enumerate() {
        let mut seen = Vec::new();
        for with_node in [true, false] {
            let w = World::new(&format!("zc-parity-{n}-{with_node}"));
            if !with_node {
                std::fs::write(w.root.join("bindings/sdc.yaml"), NO_MODEL).unwrap();
            }
            w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
            w.fake.hook("tree_run", "default", &model_call_hook("0.1"));
            let mut a = w.args("r1", *cap);
            set(&mut a, "--max-llm-calls", &calls.to_string());
            let o = w.run(&a);
            let outcome = w.outcome("r1");
            let booked = std::fs::read_to_string(w.ledger()).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v["spent_micro_usd"].as_u64());
            println!(
                "cap {cap}, calls {calls}, model node {with_node}: exit {}, outcome {}, llm_calls {}, spent {}, booked {booked:?}, tree runs {}",
                code(&o),
                outcome["outcome"],
                outcome["llm_calls"],
                outcome["spent_micro_usd"],
                w.fake.calls_of(&["tree", "run"]).len()
            );
            if outcome["has_model_node"] != with_node {
                wrong.push(format!("cap {cap}, calls {calls}: has_model_node is {} in the run built {}", outcome["has_model_node"], if with_node { "with a model node" } else { "without one" }));
            }
            if outcome["spent_micro_usd"].as_u64() != booked {
                wrong.push(format!("cap {cap}, calls {calls}, model node {with_node}: the outcome states {} and the ledger holds {booked:?}", outcome["spent_micro_usd"]));
            }
            seen.push((code(&o), outcome["outcome"].clone(), outcome["llm_calls"].clone(), w.fake.calls_of(&["tree", "run"]).len()));
            let want = if with_node { *booked_with } else { *booked_without };
            if booked != Some(want) {
                wrong.push(format!("cap {cap}, calls {calls}, model node {with_node}: the ledger holds {booked:?}, {want} expected"));
            }
        }
        if seen[0] != seen[1] {
            wrong.push(format!("cap {cap}, calls {calls}: with a model node {:?}, without one {:?}", seen[0], seen[1]));
        }
        if seen[1].0 == 0 {
            wrong.push(format!("cap {cap}, calls {calls}: the run without a model node ended with exit 0 past its caps"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
