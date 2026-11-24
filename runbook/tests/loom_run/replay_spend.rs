//! What a re-drive books as spent (`docs/loom-run.md`, "A re-drive spends nothing it did not
//! start"): a reply taken from the recording costs what the recording says, and that is not money
//! this run spent. The fake `b10x-loom` writes the cost lines a replayed model node writes, and a
//! stand-in `sdc-replay` makes the plan; no model call, no network, no recording.

use super::core::{code, stderr, World};
use super::fake::{decision_json, tree_run_json};
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::process::Output;

const STAND_IN: &str = r#"#!/bin/sh
case "$1" in
  prepare)
    out=""; prev=""; plan_only=no
    for a in "$@"; do
      [ "$prev" = --out ] && out=$a
      [ "$a" = --plan-only ] && plan_only=yes
      prev=$a
    done
    if [ "$plan_only" = yes ]; then echo '{"settings":{}}'; exit 0; fi
    mkdir -p "$out/replay"
    echo '{"first_line":"re-drive with a stand-in plan"}' > "$out/replay/plan.json"
    echo '{}'
    ;;
  *) echo '{}' ;;
esac
"#;

/// A `tree_run` hook: one model call of `node.write` that the replay adapter answered in `mode`
/// (`reused`, `recorded` or `live`) and the record says cost `cost` USD.
fn replayed_call(mode: &str, cost: &str) -> String {
    format!(
        "w=work/.sdc/llm; mkdir -p \"$w\"\n\
i=$(( $(find \"$w\" -mindepth 1 -maxdepth 1 -type d | wc -l) + 1 ))\n\
d=$(printf '%03d-node.write' $i); mkdir -p \"$w/$d\"\n\
printf '{{\"action\":\"node.write\",\"adapter\":\"{mode}\",\"dry\":false,\"exit\":0,\"record\":\"%s/%s\",\"cost_usd\":{cost}}}\\n' \"$PWD/$w\" \"$d\" >> \"$w/calls.jsonl\"\n"
    )
}

/// The world with the stand-in helper first on the PATH and the recording named by `--recorded`.
fn world(tag: &str) -> World {
    let w = World::new(tag);
    let helper = w.root.join("sdc-replay");
    std::fs::write(&helper, STAND_IN).unwrap();
    std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::create_dir_all(w.root.join("recording")).unwrap();
    std::fs::create_dir_all(w.root.join("assets/protocol")).unwrap();
    w
}

fn redrive(w: &World, name: &str, cap: u64, replay_options: &[&str]) -> Output {
    let mut args = w.args(name, cap);
    args.extend(["--recorded".to_owned(), w.root.join("recording").display().to_string()]);
    args.extend(replay_options.iter().map(|o| (*o).to_owned()));
    // RUNBOOK_HOME keeps a built `driver/target` of the checkout from answering for the stand-in.
    w.command(&args).env("RUNBOOK_HOME", w.root.join("assets")).output().unwrap()
}

fn ledger_text(w: &World) -> String {
    std::fs::read_to_string(w.ledger()).unwrap()
}

fn write_ledger(w: &World, spent: u64) -> String {
    let v = json!({"format": "runbook.spend-ledger/1", "spent_micro_usd": spent, "reserved": {}, "entries": [{"run": "earlier", "what": "step", "micro_usd": spent}]});
    let text = serde_json::to_string_pretty(&v).unwrap() + "\n";
    std::fs::write(w.ledger(), &text).unwrap();
    text
}

fn numbers(outcome: &Value) -> [Option<u64>; 4] {
    ["spent_micro_usd", "llm_calls", "recorded_micro_usd", "recorded_llm_calls"].map(|k| outcome[k].as_u64())
}

#[test]
fn a_redrive_that_cannot_go_live_books_nothing_names_the_recorded_cost_and_leaves_the_ledger_alone() {
    let w = world("redrive-no-live");
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "1", &replayed_call("reused", "0.5"));
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    // The earlier runs spent the whole cap: a run that spent money would be stopped by it.
    let before = write_ledger(&w, 700_000);
    let o = redrive(&w, "r1", 700_000, &["--on-divergence", "nearest", "--live-adapter", "dry", "--live-cap", "0"]);
    assert_eq!(code(&o), 0, "a re-drive is not stopped by a spent cap: {}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "ended", "{outcome}");
    assert_eq!(numbers(&outcome), [Some(0), Some(0), Some(500_000), Some(1)], "spent, calls, recorded cost, recorded calls: {outcome}");
    assert_eq!(ledger_text(&w), before, "a re-drive reserves and books nothing");
    let ended = std::fs::read_to_string(w.run_dir("r1").join("events.jsonl")).unwrap().lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).find(|e| e["event"] == "ended").unwrap();
    assert_eq!(numbers(&ended), [Some(0), Some(0), Some(500_000), Some(1)], "the ended event says the same: {ended}");
    let costed: Vec<Value> = std::fs::read_to_string(w.run_dir("r1").join("events.jsonl")).unwrap().lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter(|e| e["event"] == "costed_call").collect();
    assert_eq!(costed.len(), 1, "{costed:?}");
    assert_eq!(costed[0]["cost_micro_usd"], 500_000, "the call keeps the cost the recording gives it");
}

#[test]
fn a_redrive_that_may_go_live_books_the_live_calls_and_names_the_recorded_ones() {
    let w = world("redrive-live");
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "1", &format!("{}{}", replayed_call("reused", "0.5"), replayed_call("live", "0.25")));
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    let o = redrive(&w, "r1", 5_000_000, &["--on-divergence", "live", "--live-adapter", "dry", "--live-cap", "2"]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(numbers(&outcome), [Some(250_000), Some(1), Some(500_000), Some(1)], "{outcome}");
    let ledger: Value = serde_json::from_str(&ledger_text(&w)).unwrap();
    assert_eq!(ledger["spent_micro_usd"], 250_000, "only the live call is in the ledger: {ledger}");
}

#[test]
fn a_run_that_is_no_redrive_books_every_call_as_spent_and_names_no_recorded_cost() {
    let w = world("not-a-redrive");
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "1", &replayed_call("reused", "0.5"));
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(numbers(&outcome), [Some(500_000), Some(1), Some(0), Some(0)], "{outcome}");
}

#[test]
fn the_decider_of_a_redrive_that_cannot_go_live_is_booked_as_recorded_too() {
    let w = world("redrive-decider");
    w.settings(&json!({"decider": {"kind": "scripted", "max_call_micro_usd": 100_000, "answers": [
        {"approve": true, "principal": "person:owner", "cost_micro_usd": 20_000, "model_calls": 1}]}}));
    let f = &w.fake;
    f.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    let d = decision_json("owner.approve", "ApproveAction", "person:owner");
    f.answer("tree_run", "2", &tree_run_json("AwaitingPerson", 0, None, &format!("[{d}]")), 3);
    f.answer("tree_approve", "default", "{\"decision\":\"Allow\"}\n", 0);
    f.answer("tree_run", "3", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    let before = write_ledger(&w, 700_000);
    let o = redrive(&w, "r1", 700_000, &["--on-divergence", "nearest", "--live-adapter", "dry", "--live-cap", "0"]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(numbers(&outcome), [Some(0), Some(0), Some(20_000), Some(1)], "{outcome}");
    assert_eq!(ledger_text(&w), before);
}
