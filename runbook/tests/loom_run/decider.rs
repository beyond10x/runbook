//! The run end to end with the model decider: the loop, the real decider of `decider.rs` starting a
//! fake `bindings/tools/decider.sh` of the run's bindings copy, the run directory writer, and a fake
//! `b10x-loom`. The unit tests of the decider and of the bindings copy are in
//! `src/loom_run/decider/tests.rs` and `src/loom_run/bindings_copy/tests.rs`.
//!
//! The settings file has the shape `sdc-experiment run` writes (`runbook.loom-run-settings/1`, see
//! `settings.rs` for what that launcher leaves out) plus the members a model decider needs.

use super::core::{code, model_call_hook, spent, stderr, World};
use super::fake::{decision_json, tree_run_json};
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

const DECIDER_SH: &str = "#!/bin/sh\nd=$(dirname \"$0\")\nprintf '%s\\n' \"$@\" > \"$d/args.txt\"\nenv > \"$d/env.txt\"\ncat \"$d/reply.json\"\n";

/// Puts a fake `decider.sh` that prints `reply` into the bindings of the world, the instruction
/// directory the model decider needs above them, and a file for `sdc-reply`.
pub fn install_decider(w: &World, reply: &str) {
    let tools = w.root.join("bindings/tools");
    std::fs::create_dir_all(&tools).unwrap();
    let script = tools.join("decider.sh");
    std::fs::write(&script, DECIDER_SH).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(tools.join("reply.json"), reply).unwrap();
    std::fs::create_dir_all(w.root.join("instructions/decider")).unwrap();
    std::fs::write(w.root.join("instructions/decider/approval.md"), "decide\n").unwrap();
    std::fs::write(w.root.join("sdc-reply"), "#!/bin/sh\n").unwrap();
    std::fs::create_dir_all(w.root.join("driver/examples/sdc")).unwrap();
    std::fs::write(w.root.join("driver/examples/sdc/rule-ends.json"), "{}\n").unwrap();
}

/// The file `sdc-experiment run` writes for a loom run, as committed in
/// `tests/fixtures/loom-run-settings.json` (`driver/tests/experiment_loom.rs` keeps the writer equal
/// to it), with the fake's sha256 and the world's login file put where the writer prints them as names.
pub fn fixture(w: &World) -> Value {
    let mut v: Value = serde_json::from_str(include_str!("../fixtures/loom-run-settings.json")).unwrap();
    v["loom"]["sha256"] = json!(w.fake.sha256());
    v["loom"]["login_file"] = json!(w.login_path().display().to_string());
    v
}

/// The fixture for run `r1` of `World::args`: the cap given and a wall time of 600 s (the options
/// repeated in the file must agree with the options, `settings.rs`).
pub fn fixture_settings(w: &World, cap: u64) -> Value {
    let mut v = fixture(w);
    v["run"] = json!("r1");
    v["loom"]["cap_micro_usd"] = json!(cap);
    v["max_wall_seconds"] = json!(600);
    v
}

pub fn write_settings(w: &World, v: &Value) {
    std::fs::write(w.root.join("settings.json"), serde_json::to_string_pretty(v).unwrap()).unwrap();
}

/// The file as it is: `sdc-reply` is found on the runner's PATH (the world puts its root there).
fn model_settings(w: &World, cap: u64) -> Value {
    fixture_settings(w, cap)
}

/// An approval as the model decider reads it: the fake's decision without its `path` member.
pub fn approval() -> String {
    decision_json("scope.approve", "ApproveAction", "person:owner").replace(",\"path\":\"/work/x.yaml\"", "")
}

/// A tree of two model steps of `step_cost` USD and an approval between them, then terminal.
pub fn tree_with_approval(w: &World, step_cost: &str) {
    let f = &w.fake;
    f.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    f.hook("tree_run", "1", &model_call_hook(step_cost));
    f.answer("tree_run", "2", &tree_run_json("AwaitingPerson", 0, None, &format!("[{}]", approval())), 3);
    f.answer("tree_approve", "default", "{\"decision\":\"Allow\"}\n", 0);
    f.answer("tree_run", "3", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    f.hook("tree_run", "3", &model_call_hook(step_cost));
    f.answer("tree_run", "4", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
}

pub fn lines(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path).unwrap_or_default().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

pub const APPROVE: &str = "{\"valid\":true,\"decision\":\"approve\",\"cost_usd\":0.02,\"adapter\":\"fake\"}\n";

#[test]
fn two_model_steps_and_an_approval_by_the_model_decider_end_delivered_with_the_run_directory() {
    let w = World::new("e2e-delivered");
    install_decider(&w, APPROVE);
    tree_with_approval(&w, "0.30");
    write_settings(&w, &model_settings(&w, 5_000_000));
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "ended");
    assert_eq!(outcome["llm_calls"], 3, "two steps and one decider call");
    assert_eq!(outcome["spent_micro_usd"], 620_000);
    assert_eq!(spent(&w), 620_000);
    // The approval was run with the decider's principal; the decider ran from the run's copy.
    let approve = w.fake.calls_of(&["tree", "approve"]);
    assert_eq!(approve.len(), 1);
    let at = approve[0].argv.iter().position(|a| a == "--principal").unwrap();
    assert_eq!(approve[0].argv[at + 1], "decider:model");
    assert!(!approve[0].argv.contains(&"--deny".to_owned()));
    let copy = w.run_dir("r1/bindings/tools");
    assert!(w.run_dir("r1/instructions/decider/approval.md").is_file() && w.run_dir("r1/driver/examples/sdc/rule-ends.json").is_file(), "the tools read their root: instructions and driver/examples beside the copy");
    let args = std::fs::read_to_string(copy.join("args.txt")).unwrap();
    assert!(args.starts_with("model\n"), "{args}");
    // decider.sh got the built environment and nothing of the runner's.
    let env = std::fs::read_to_string(copy.join("env.txt")).unwrap();
    for needle in ["SDC_SECRET_MARKER", "ANTHROPIC_API_KEY", "marker-home-dir", "LOGIN-FILE-MARKER"] {
        assert!(!env.contains(needle), "{needle} reached decider.sh: {env}");
    }
    assert!(env.contains(&format!("HOME={}", w.run_dir("r1/home").display())), "{env}");
    assert!(env.contains("SDC_LLM_MODEL_DECIDER=claude-opus-5-5") && env.contains("SDC_DECIDER_ESCALATE=final"), "{env}");
    // The run directory of the writer: what it can say from the events of this run.
    let dir = w.run_dir("r1");
    assert_eq!(lines(&dir.join("run-settings.jsonl")).len(), 1);
    let calls = lines(&dir.join("llm-calls.jsonl"));
    assert_eq!(calls.len(), 2, "one line per costed model step: {calls:?}");
    let decisions = lines(&dir.join("decisions.jsonl"));
    assert_eq!(decisions.len(), 1);
    assert_eq!(decisions[0]["principal"], "decider:model");
    assert_eq!(decisions[0]["decision"], "approve");
    assert_eq!(decisions[0]["model_calls"], 1);
    assert_eq!(lines(&dir.join("rounds.jsonl")).len(), 4);
    let status: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("status.json")).unwrap()).unwrap();
    assert_eq!((status["state"].as_str(), status["outcome"].as_str(), status["delivered"].as_bool()), (Some("closed"), Some("operating"), Some(true)));
    assert!(!dir.join("run-stopped.json").exists());
    // The settings of the run hold no login path and name what the command was given.
    let recorded: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("settings.json")).unwrap()).unwrap();
    assert_eq!(recorded["login_file"], "named");
    assert_eq!(recorded["loom"]["cap_micro_usd"], 5_000_000);
    assert!(recorded["decider"].get("program").is_none(), "the path of the copy's decider is not recorded: {recorded}");
}

#[test]
fn a_run_with_the_model_decider_is_stopped_by_the_spend_cap_and_the_directory_says_so() {
    let w = World::new("e2e-cap");
    install_decider(&w, APPROVE);
    tree_with_approval(&w, "0.30");
    write_settings(&w, &model_settings(&w, 520_000));
    let o = w.run(&w.args("r1", 520_000));
    assert_eq!(code(&o), 4, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "stopped: spend cap");
    assert_eq!(outcome["spent_micro_usd"], 520_000, "0.30 for the first step, 0.02 for the decider, what the cap had left for the second");
    assert_eq!(spent(&w), 520_000);
    assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), 3, "no fourth step after the cap");
    let dir = w.run_dir("r1");
    let stop: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("run-stopped.json")).unwrap()).unwrap();
    assert_eq!(stop["cause"], "spend_cap");
    assert!(!dir.join("status.json").exists(), "a stopped run is not closed");
    assert_eq!(lines(&dir.join("llm-calls.jsonl")).len(), 2);
    assert_eq!(lines(&dir.join("decisions.jsonl")).len(), 1);
}

#[test]
fn a_decider_that_escalates_stops_the_run_for_a_person_and_books_nothing_for_it() {
    let w = World::new("e2e-cannot");
    install_decider(&w, "{\"valid\":true,\"decision\":\"escalate\",\"reasons\":[\"the position is not clear\"],\"cost_usd\":0.02,\"adapter\":\"fake\"}\n");
    tree_with_approval(&w, "0.30");
    let mut v = model_settings(&w, 5_000_000);
    v["decider"]["escalate"] = json!("stop");
    write_settings(&w, &v);
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 8, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "stopped: decider");
    let reason = outcome["reason"].as_str().unwrap();
    assert!(reason.contains("reserved for a person") && reason.contains("the position is not clear"), "{reason}");
    assert_eq!(spent(&w), 300_000, "the step only: the reservation of the decider was released");
    assert!(w.fake.calls_of(&["tree", "approve"]).is_empty(), "nothing was approved");
    let dir = w.run_dir("r1");
    let stop: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("run-stopped.json")).unwrap()).unwrap();
    assert_eq!(stop["cause"], "decider_cannot_decide");
    assert!(lines(&dir.join("decisions.jsonl")).is_empty());
}

#[test]
fn a_rejection_by_the_model_decider_runs_the_printed_command_with_deny() {
    let w = World::new("e2e-reject");
    install_decider(&w, "{\"valid\":true,\"decision\":\"reject\",\"cost_usd\":0.02,\"adapter\":\"fake\"}\n");
    tree_with_approval(&w, "0.30");
    write_settings(&w, &model_settings(&w, 5_000_000));
    let o = w.run(&w.args("r1", 5_000_000));
    let approve = w.fake.calls_of(&["tree", "approve"]);
    assert_eq!(approve.len(), 1, "{}", stderr(&o));
    assert_eq!(approve[0].argv.last().map(String::as_str), Some("--deny"));
    let decisions = lines(&w.run_dir("r1/decisions.jsonl"));
    assert_eq!(decisions[0]["decision"], "reject");
}
