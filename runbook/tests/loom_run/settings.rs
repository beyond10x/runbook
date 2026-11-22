//! The one settings format, `runbook.loom-run-settings/1` (`docs/loom-run-settings.md`), and what a
//! run does with the file `sdc-experiment run` writes.
//!
//! `tests/fixtures/loom-run-settings.json` is the one copy of that file kept on this side. The
//! launcher's test (`driver/tests/experiment_loom.rs`, the test that names the fixture) prints the
//! file for a record and compares it with the fixture; the tests here run the fixture. Neither
//! crate's test builds the other.

use super::core::{code, stderr, World};
use super::decider::{fixture, fixture_settings, install_decider, lines, tree_with_approval, write_settings, APPROVE};
use serde_json::{json, Value};

/// Runs with the fixture changed by `mutate` and asserts a refusal (exit 2, nothing written, no
/// call) that names `needle`.
fn refused(tag: &str, mutate: impl Fn(&mut Value), needle: &str) {
    let w = World::new(tag);
    let mut v = fixture_settings(&w, 5_000_000);
    mutate(&mut v);
    write_settings(&w, &v);
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 2, "{needle}: {}", stderr(&o));
    assert!(stderr(&o).contains(needle), "{needle} not named: {}", stderr(&o));
    assert!(!w.run_dir("r1").exists(), "{needle}: a run directory was made");
    assert_eq!(w.fake.calls_so_far(), 0, "{needle}: the binary was called");
    assert!(!w.ledger().exists(), "{needle}: the ledger was made");
}

#[test]
fn the_file_sdc_experiment_writes_runs_whole_with_the_model_decider_it_names() {
    // Every member of the fixture is accepted and none is left to guess: the run takes its name, its
    // caps (from the file, through `args_for`) and its decider from the file, and ends delivered.
    let w = World::new("fixture-runs");
    install_decider(&w, APPROVE);
    tree_with_approval(&w, "0.30");
    let v = fixture(&w);
    write_settings(&w, &v);
    let o = w.run(&w.args_for(&v));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let run = v["run"].as_str().unwrap();
    assert_eq!(w.outcome(run)["outcome"], "ended");
    // decider.kind and autonomy: the model decider answered the approval, as the principal the
    // runner derives from the kind (the file names none).
    let approve = w.fake.calls_of(&["tree", "approve"]);
    assert_eq!(approve.len(), 1);
    let at = approve[0].argv.iter().position(|a| a == "--principal").unwrap();
    assert_eq!(approve[0].argv[at + 1], "decider:model");
    // decider.adapter names the fake adapter of the copy, decider.model reaches its request and
    // sdc-reply found on PATH its environment.
    let env = std::fs::read_to_string(w.run_dir(&format!("{run}/bindings/tools/env.txt"))).unwrap();
    let asked: Value = serde_json::from_str(&std::fs::read_to_string(w.run_dir(&format!("{run}/bindings/tools/adapter-request.json"))).unwrap()).unwrap();
    assert_eq!(asked["model"], "claude-opus-5-5");
    assert!(env.contains(&format!("SDC_REPLY={}", w.root.join("sdc-reply").display())), "{env}");
    // adapter and review.adapter: the model nodes of the copy run on metaharness, the review class on claude-cli.
    let bindings = std::fs::read_to_string(w.run_dir(&format!("{run}/bindings/sdc.yaml"))).unwrap();
    assert!(bindings.contains("--adapter"), "{bindings}");
    assert_eq!(lines(&w.run_dir(&format!("{run}/decisions.jsonl"))).len(), 1);
    let recorded: Value = serde_json::from_str(&std::fs::read_to_string(w.run_dir(&format!("{run}/settings.json"))).unwrap()).unwrap();
    assert_eq!(recorded["login_file"], "named");
    assert_eq!(recorded["autonomy"], "A5");
    assert_eq!(recorded["decider"]["kind"], "model");
}

#[test]
fn the_file_without_the_members_the_launcher_now_writes_is_stopped_where_the_decider_needs_them() {
    // Without `autonomy` the model decider is built and the run stops at the first decision for a
    // person: it answers only at A5. This is what the launcher wrote before it wrote `autonomy`.
    let w = World::new("fixture-no-autonomy");
    install_decider(&w, APPROVE);
    tree_with_approval(&w, "0.30");
    let mut v = fixture_settings(&w, 5_000_000);
    v.as_object_mut().unwrap().remove("autonomy");
    write_settings(&w, &v);
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 8, "{}", stderr(&o));
    let reason = w.outcome("r1")["reason"].as_str().unwrap().to_owned();
    assert!(reason.contains("no autonomy level named"), "{reason}");
    // And without `decider.kind` the decider is refused by name.
    refused("fixture-no-kind", |v| {
        v["decider"].as_object_mut().unwrap().remove("kind");
    }, "decider.kind");
}

#[test]
fn a_member_that_repeats_an_option_must_agree_with_it_and_is_refused_by_name() {
    refused("sha", |v| v["loom"]["sha256"] = json!("00".repeat(32)), "loom.sha256");
    refused("conf", |v| v["loom"]["confinement"] = json!("substrate"), "loom.confinement");
    refused("cap", |v| v["loom"]["cap_micro_usd"] = json!(1), "loom.cap_micro_usd");
    refused("calls", |v| v["max_llm_calls"] = json!(150), "max_llm_calls");
    refused("wall", |v| v["max_wall_seconds"] = json!(3600), "max_wall_seconds");
    refused("calls-null", |v| v["max_llm_calls"] = Value::Null, "max_llm_calls");
    refused("run", |v| v["run"] = json!("another"), "is not the run named by --run");
    refused("format", |v| v["format"] = json!("runbook.loom-run-settings/2"), "is not runbook.loom-run-settings/1");
    refused("bin", |v| v["loom"]["bin"] = json!("/opt/loom/b10x-loom"), "loom.bin");
    refused("loom-key", |v| v["loom"]["extra"] = json!(1), "loom.extra");
}

#[test]
fn the_login_file_is_given_once_and_the_deciders_program_is_the_runners() {
    refused(
        "login-twice",
        |v| v["login_file"] = json!("/somewhere/else"),
        "login_file and loom.login_file",
    );
    refused("program", |v| v["decider"]["program"] = json!("/opt/decider.sh"), "decider.program");
    refused("principal", |v| v["decider"]["principal"] = json!("person:owner"), "decider.principal");
}

#[test]
fn a_scripted_decider_takes_the_principal_of_its_answers_from_the_settings() {
    // Loom prints `operator` for every decision. `decider.principal` is the default of the answers,
    // an answer's own `principal` wins, and an answer with neither keeps what Loom printed.
    use super::fake::{decision_json, tree_run_json};
    let w = World::new("scripted-principal");
    let three = format!("[{},{},{}]", decision_json("a.one", "ApproveAction", "operator"), decision_json("a.two", "Approve", "operator"), decision_json("a.three", "ApproveAction", "operator"));
    w.fake.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 0, None, &three), 3);
    w.fake.answer("tree_approve", "default", "{\"decision\":\"Allow\"}\n", 0);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.settings(&json!({"decider": {"kind": "scripted", "principal": "person:owner", "max_call_micro_usd": 0, "answers": [
        {"approve": true, "cost_micro_usd": 0},
        {"approve": true, "principal": "person:other", "cost_micro_usd": 0},
        {"approve": true, "cost_micro_usd": 0}]}}));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let principal_of = |c: &super::fake::Call| c.argv[c.argv.iter().position(|a| a == "--principal").unwrap() + 1].clone();
    let principals: Vec<String> = w.fake.calls_of(&["tree", "approve"]).iter().map(principal_of).collect();
    assert_eq!(principals, ["person:owner", "person:other", "person:owner"]);
    // Without `decider.principal` an answer without one keeps the principal Loom printed.
    let w = World::new("scripted-no-principal");
    w.fake.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 0, None, &format!("[{}]", decision_json("a.one", "ApproveAction", "operator"))), 3);
    w.fake.answer("tree_approve", "default", "{\"decision\":\"Allow\"}\n", 0);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.settings(&json!({"decider": {"kind": "scripted", "max_call_micro_usd": 0, "answers": [{"approve": true, "cost_micro_usd": 0}]}}));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(principal_of(&w.fake.calls_of(&["tree", "approve"])[0]), "operator");
}

#[test]
fn a_decider_kind_person_is_refused_by_name_before_any_run_directory_or_call() {
    // `cycles/suite.yaml` autopilot-person: a run is not started with a person as its decider.
    refused("kind-person", |v| v["decider"]["kind"] = json!("person"), "person");
    refused("kind-person-no-autonomy", |v| {
        v["decider"]["kind"] = json!("person");
        v.as_object_mut().unwrap().remove("autonomy");
    }, "person");
}

#[test]
fn a_stand_in_names_its_principal_the_run_directory_labels_it_and_no_principal_is_invented() {
    // `cycles/suite.yaml` principals: the scripted stand-in is `person:stand-in` in the approval and in
    // the decision line, its run settings say what kind of decider it is, nothing is recorded as
    // `human`; a decision that prints no principal and an answer that names none runs with none.
    use super::fake::{decision_json, tree_run_json};
    let w = World::new("stand-in-principal");
    let d = decision_json("a.one", "ApproveAction", "operator");
    w.fake.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 0, None, &format!("[{d}]")), 3);
    w.fake.answer("tree_approve", "default", "{\"decision\":\"Allow\"}\n", 0);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.settings(&json!({"decider": {"kind": "scripted", "principal": "person:stand-in", "max_call_micro_usd": 0, "answers": [{"approve": true, "cost_micro_usd": 0}]}}));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let argv = &w.fake.calls_of(&["tree", "approve"])[0].argv;
    assert_eq!(argv[argv.iter().position(|a| a == "--principal").unwrap() + 1], "person:stand-in");
    let dir = w.run_dir("r1");
    let decided = lines(&dir.join("decisions.jsonl"));
    assert_eq!(decided[0]["principal"], "person:stand-in");
    let first = lines(&dir.join("run-settings.jsonl")).remove(0);
    assert_eq!(first["decider"]["kind"], "scripted");
    assert_eq!(first["decider"]["principal"], "person:stand-in");
    let mut hits = Vec::new();
    super::core::grep_tree(&dir, "human", &mut hits);
    assert!(hits.is_empty(), "something records `human`: {hits:?}");
    // No principal in the printed command and none in the answer: the approval goes out without one.
    let w = World::new("no-principal");
    let bare = decision_json("a.one", "ApproveAction", "operator").replace(",\"--principal\",\"operator\"", "");
    w.fake.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 0, None, &format!("[{bare}]")), 3);
    w.fake.answer("tree_approve", "default", "{\"decision\":\"Allow\"}\n", 0);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.settings(&json!({"decider": {"kind": "scripted", "max_call_micro_usd": 0, "answers": [{"approve": true, "cost_micro_usd": 0}]}}));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(!w.fake.calls_of(&["tree", "approve"])[0].argv.iter().any(|a| a == "--principal"));
    assert_eq!(lines(&w.run_dir("r1/decisions.jsonl"))[0]["principal"], "");
}
