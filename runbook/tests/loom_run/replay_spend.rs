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

/// A re-drive that cannot go live holds a call whose cost line says `live`: the adapter says it started a live call
/// that the policy forbids. That is an error of the run, named; the call is never booked as recorded and nothing is
/// booked as spent.
#[test]
fn a_call_the_adapter_says_was_live_in_a_redrive_that_cannot_go_live_is_an_error_of_the_run() {
    let w = world("fix1-live-line");
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "1", &replayed_call("live", "0.25"));
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    let o = redrive(&w, "r1", 5_000_000, &["--on-divergence", "nearest", "--live-adapter", "dry", "--live-cap", "0"]);
    assert_eq!(code(&o), 1, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "error", "{outcome}");
    let reason = outcome["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("node.write") && reason.contains("live"), "the reason names the call and the adapter's word: {outcome}");
    assert_eq!(numbers(&outcome), [Some(0), Some(0), Some(0), Some(0)], "spent, calls, recorded cost, recorded calls: {outcome}");
    assert!(!w.ledger().exists(), "a re-drive that cannot go live made a ledger file");
}

/// A re-drive that cannot go live opens no ledger, so an unreadable ledger does not end it: it ends as it would with
/// no ledger, the file stays as it was, and "Fail closed" in `docs/loom-run.md` names that exception.
#[test]
fn a_redrive_that_cannot_go_live_opens_no_ledger_so_an_unreadable_one_does_not_end_it() {
    let w = world("fix1-bad-ledger");
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "1", &replayed_call("reused", "0.5"));
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    std::fs::write(w.ledger(), "not json\n").unwrap();
    let o = redrive(&w, "r1", 700_000, &["--on-divergence", "nearest", "--live-adapter", "dry", "--live-cap", "0"]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(ledger_text(&w), "not json\n", "a re-drive that cannot go live touched the ledger");
    let page = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/loom-run.md")).unwrap();
    let from = page.find("### Fail closed").expect("the section 'Fail closed'");
    let section = &page[from..from + page[from..].find("### What the spend cap does not promise").expect("the next section")];
    assert!(section.contains("a re-drive that cannot go live opens no ledger, so an unreadable ledger does not end it"), "docs/loom-run.md, 'Fail closed', lacks the exception: {section}");
}

/// A re-drive whose policy cannot go live (`nearest`, or none given: `stop`) spends nothing whatever its live cap
/// says: the live cap is not given, or is 5.
#[test]
fn a_redrive_whose_policy_cannot_go_live_spends_nothing_whatever_its_live_cap() {
    for (tag, options) in [("nearest-nocap", vec!["--on-divergence", "nearest"]), ("nearest-cap5", vec!["--on-divergence", "nearest", "--live-cap", "5"]), ("no-policy", vec![])] {
        let w = world(&format!("fix1-policy-{tag}"));
        w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
        w.fake.hook("tree_run", "1", &replayed_call("reused", "0.5"));
        w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
        let before = write_ledger(&w, 700_000);
        let o = redrive(&w, "r1", 700_000, &options);
        assert_eq!(code(&o), 0, "{tag}: {}", stderr(&o));
        assert_eq!(numbers(&w.outcome("r1")), [Some(0), Some(0), Some(500_000), Some(1)], "{tag}");
        assert_eq!(ledger_text(&w), before, "{tag}");
    }
}

/// A call of the recording that failed (exit 1) and has no cost: the adapter answers it from the recording and writes
/// the line with no `cost_usd`.
fn replayed_failed_call(mode: &str) -> String {
    format!(
        "w=work/.sdc/llm; mkdir -p \"$w\"\n\
i=$(( $(find \"$w\" -mindepth 1 -maxdepth 1 -type d | wc -l) + 1 ))\n\
d=$(printf '%03d-node.write' $i); mkdir -p \"$w/$d\"\n\
printf '{{\"action\":\"node.write\",\"adapter\":\"{mode}\",\"dry\":false,\"exit\":1,\"record\":\"%s/%s\"}}\\n' \"$PWD/$w\" \"$d\" >> \"$w/calls.jsonl\"\n"
    )
}

/// A re-drive that may go live and reuses a call that failed in the source (no cost line): the call is a recorded
/// one, so it is not booked at the reservation as spent and not counted as a call this run started.
#[test]
fn a_reused_call_that_failed_with_no_cost_is_not_booked_as_spent_in_a_live_capable_redrive() {
    let w = world("fix1-reused-failed");
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "1", &replayed_failed_call("reused"));
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    let o = redrive(&w, "r1", 5_000_000, &["--on-divergence", "live", "--live-adapter", "dry", "--live-cap", "2"]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(numbers(&outcome), [Some(0), Some(0), Some(0), Some(1)], "spent, calls, recorded cost, recorded calls: {outcome}");
}

/// The same call in a re-drive that cannot go live: nothing booked, one recorded call, the event costs 0.
#[test]
fn a_failed_call_with_no_cost_in_a_redrive_that_cannot_go_live_books_nothing_and_has_no_ledger() {
    let w = world("fix1-failed-nolive");
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "1", &replayed_failed_call("reused"));
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    let o = redrive(&w, "r1", 5_000_000, &["--on-divergence", "nearest", "--live-adapter", "dry", "--live-cap", "0"]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(numbers(&w.outcome("r1")), [Some(0), Some(0), Some(0), Some(1)]);
    assert!(!w.ledger().exists(), "a re-drive that cannot go live made a ledger file");
    let costed: Vec<Value> = std::fs::read_to_string(w.run_dir("r1").join("events.jsonl")).unwrap().lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter(|e| e["event"] == "costed_call").collect();
    assert_eq!(costed.len(), 1, "{costed:?}");
    assert_eq!(costed[0]["cost_micro_usd"], 0, "{costed:?}");
}

/// `--live-cap 1` is a cap that lets one live call start: the re-drive spends, so its live call is booked in the
/// ledger and a ledger at the cap stops it.
#[test]
fn a_redrive_with_a_live_cap_of_one_can_go_live_and_books_the_live_call() {
    let w = world("fix1-cap-one");
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "1", &replayed_call("live", "0.25"));
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    let o = redrive(&w, "r1", 5_000_000, &["--on-divergence", "live", "--live-adapter", "dry", "--live-cap", "1"]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let ledger: Value = serde_json::from_str(&ledger_text(&w)).unwrap();
    assert_eq!(ledger["spent_micro_usd"], 250_000, "the live call is booked: {ledger}");
}

/// A re-drive that may go live: a recorded call counts against neither cap. The cap is 400000 micro USD and the
/// call limit 2; the step makes three reused calls (1.5 USD) and one live call (0.25 USD).
#[test]
fn recorded_calls_count_against_neither_the_spend_cap_nor_the_call_cap() {
    let w = world("fix1-caps");
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    let calls = format!("{}{}{}{}", replayed_call("reused", "0.5"), replayed_call("reused", "0.5"), replayed_call("reused", "0.5"), replayed_call("live", "0.25"));
    w.fake.hook("tree_run", "1", &calls);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    let mut a = w.args("r1", 400_000);
    let i = a.iter().position(|x| x == "--max-llm-calls").unwrap();
    a[i + 1] = "2".to_owned();
    a.extend(["--recorded".to_owned(), w.root.join("recording").display().to_string()]);
    a.extend(["--on-divergence", "live", "--live-adapter", "dry", "--live-cap", "2"].map(str::to_owned));
    let o = w.command(&a).env("RUNBOOK_HOME", w.root.join("assets")).output().unwrap();
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(numbers(&outcome), [Some(250_000), Some(1), Some(1_500_000), Some(3)], "{outcome}");
}
