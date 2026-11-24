//! What the spend ledger does with a call that crosses its budget and with a reservation that a
//! killed run left behind (`docs/loom-run.md`, "What the spend cap does not promise").

use super::core::{code, spent, stderr, World, RECORD_DIR};
use super::fake::tree_run_json;
use serde_json::{json, Value};
use std::process::{Command, Stdio};

/// A model call that costs `cost` USD whatever budget the copy gave it, as a call does when the
/// model CLI ends a turn above its `--max-budget-usd`.
fn call_costing(cost: &str) -> String {
    format!(
        "w=work/.sdc/llm; [ -d \"$w\" ] || mkdir -p \"$w\"\n\
{RECORD_DIR}\
printf '{{\"action\":\"node.write\",\"record\":\"%s/%s\",\"cost_usd\":{cost}}}\\n' \"$PWD/$w\" \"$d\" >> \"$w/calls.jsonl\"\n"
    )
}

fn ledger_json(w: &World) -> Value {
    serde_json::from_str(&std::fs::read_to_string(w.ledger()).unwrap()).unwrap()
}

fn write_ledger(w: &World, spent: u64, reserved: Value) {
    let v = json!({"format": "runbook.spend-ledger/1", "spent_micro_usd": spent, "reserved": reserved, "entries": []});
    std::fs::write(w.ledger(), serde_json::to_string_pretty(&v).unwrap()).unwrap();
}

fn budgets_of_calls(w: &World) -> Vec<String> {
    w.fake
        .calls()
        .iter()
        .enumerate()
        .filter(|(_, c)| c.argv.starts_with(&["tree".into(), "run".into()]))
        .filter_map(|(i, _)| std::fs::read_to_string(w.fake.dir.join(format!("bind.{}", i + 1))).ok())
        .collect()
}

#[test]
fn a_call_that_crosses_its_budget_is_booked_in_full_and_the_ledger_ends_above_the_cap() {
    // The budget of a call is what the cap has left; what the model CLI then spends is what the
    // call record says, and that is booked whole: one call can end above the cap by its overshoot.
    let w = World::new("overshoot");
    w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "1", &format!("{}cp bindings/sdc.yaml \"$D/bind.$n\"\n", call_costing("0.6")));
    w.fake.hook("tree_run", "2", &format!("{}cp bindings/sdc.yaml \"$D/bind.$n\"\n", call_costing("0.9")));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 4, "{}", stderr(&o));
    let budgets = budgets_of_calls(&w);
    assert_eq!(budgets.len(), 2, "no third call after the cap");
    assert!(budgets[1].contains("'0.4'"), "the second call was given what remained: {}", budgets[1]);
    assert_eq!(spent(&w), 1_500_000, "0.6 and the whole 0.9 of the call that crossed its 0.4");
    assert_eq!(w.outcome("r1")["spent_micro_usd"], 1_500_000);
    assert!(ledger_json(&w)["reserved"].as_object().unwrap().is_empty());
}

#[test]
fn a_reservation_left_by_a_killed_run_reduces_what_is_left_and_never_raises_it() {
    let w = World::new("stale-written");
    write_ledger(&w, 0, json!({"killed-run-1": 700_000}));
    w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "default", &format!("{}cp bindings/sdc.yaml \"$D/bind.$n\"\n", call_costing("1")));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 4, "{}", stderr(&o));
    let budgets = budgets_of_calls(&w);
    assert_eq!(budgets.len(), 1, "the 0.3 that was left bought one call");
    assert!(budgets[0].contains("'0.3'"), "the first call was given the cap less the stale reservation: {}", budgets[0]);
    assert_eq!(spent(&w), 1_000_000, "the call overshot its 0.3 and was booked whole");
    let reserved = ledger_json(&w)["reserved"].clone();
    assert_eq!(reserved, json!({"killed-run-1": 700_000}), "the stale reservation is kept, as it was");
    // A reservation as large as the cap leaves nothing: the run stops before its first call.
    let w = World::new("stale-all");
    write_ledger(&w, 0, json!({"killed-run-1": 1_000_000}));
    w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 4, "{}", stderr(&o));
    assert!(w.fake.calls_of(&["tree", "run"]).is_empty());
    assert_eq!(w.outcome("r1")["outcome"], "stopped: spend cap");
}

#[test]
fn after_a_sigkill_the_next_run_on_the_same_ledger_finds_less_left_not_more() {
    // The real thing: the runner is killed with SIGKILL while a call is on; it never releases the
    // reservation it made for it.
    let w = World::new("stale-sigkill");
    w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook(
        "tree_run",
        "default",
        &format!("if [ ! -e \"$D/first-done\" ]; then touch \"$D/first-done\"; echo $$ > \"$D/pid\"; exec sleep 30; fi\n{}cp bindings/sdc.yaml \"$D/bind.$n\"\n", call_costing("0.6")),
    );
    let mut first = w.command(&w.args("r1", 2_000_000)).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let started = std::time::Instant::now();
    while !w.fake.dir.join("pid").exists() {
        assert!(started.elapsed().as_secs() < 20, "the fake tree run did not start");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let pid = std::fs::read_to_string(w.fake.dir.join("pid")).unwrap().trim().to_owned();
    first.kill().unwrap();
    first.wait().unwrap();
    assert!(Command::new("kill").args(["-TERM", &pid]).status().unwrap().success());
    let left = ledger_json(&w);
    assert_eq!(left["spent_micro_usd"], 0);
    let held: Vec<u64> = left["reserved"].as_object().unwrap().values().map(|v| v.as_u64().unwrap()).collect();
    assert_eq!(held, [1_000_000], "SIGKILL leaves the reservation of the call that was on: {left}");
    // The next run, same ledger, same cap of 2.00: 1.00 is held, so its calls are given 1.00, then 0.40, then none.
    let o = w.run(&w.args("r2", 2_000_000));
    assert_eq!(code(&o), 4, "{}", stderr(&o));
    let budgets = budgets_of_calls(&w);
    assert!(budgets.last().unwrap().contains("'0.4'"), "{budgets:?}");
    assert_eq!(w.outcome("r2")["outcome"], "stopped: spend cap");
    assert_eq!(spent(&w), 1_200_000, "two calls of 0.6: the second was given the 0.4 that the stale 1.00 left of the cap of 2.00 and overshot it; the stale 1.00 was never spent and never given back");
    let after = ledger_json(&w);
    assert_eq!(after["reserved"].as_object().unwrap().values().map(|v| v.as_u64().unwrap()).collect::<Vec<_>>(), [1_000_000], "{after}");
}
