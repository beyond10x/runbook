//! The `goals` port of `runbook run`: `--goals <file>` is copied to `work/goals.yaml` of the run, and
//! with `decider.requester` the decider answers the decisions of the requester-side classes from
//! that file, as principal `decider:requester`. The loop, the decider of `decider.rs`, a fake
//! adapter that writes the requester's `out/reply.json`, a stand-in for the grounding check
//! (`tools/decision-grounds.sh`, which the run's copy carries beside the bindings) and the fake
//! `b10x-loom`. Invented text, no model, no network. The unit tests are in
//! `src/loom_run/decider/requester_tests.rs`.

use super::core::{code, spent, stderr, World};
use super::decider::{fixture_settings, install_decider, lines, write_settings};
use super::fake::{decision_json, tree_run_json};
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

const GOALS: &str = "goals:\n  - id: G2\n    text: The viewer works without an account.\n";
const VERIFIED: &str = r#"{"recommend":{"check_verdict":"verified","check_report":"every ground holds","grounds":[]}}"#;
const APPROVE_REPLY: &str = r#"{"option":"A","answer":null,"reasons":["the goal G2 supports it"],"grounds":[{"source":"goals","citation":{"form":"artifact_revision","value":{"artifact":"goals","revision":"g1","entry":"G2"}},"supports":"A","how":"it says so"}],"strength":"strong","no_ground_in_goals":false}"#;
const NO_GROUND_REPLY: &str = r#"{"option":null,"answer":null,"reasons":["no entry covers it"],"grounds":[],"strength":"weak","no_ground_in_goals":true}"#;

/// The fake adapter of the requester: counts its starts in `n`, writes `out/reply.json` from
/// `reply.json` (and the model decider's `out/decision.json` from `decision.json`) and prints `response.json`.
const ADAPTER: &str = "#!/bin/sh\nd=$(dirname \"$0\")\nrec=$(dirname \"$1\")\nn=$(cat \"$d/n\" 2>/dev/null || echo 0)\necho $((n + 1)) > \"$d/n\"\ncp \"$d/reply.json\" \"$rec/cwd/out/reply.json\"\ncp \"$d/decision.json\" \"$rec/cwd/out/decision.json\"\ncat \"$d/response.json\"\n";
const GROUNDS: &str = "#!/bin/sh\ncat \"$(dirname \"$0\")/verdict.json\"\n";

fn write_exe(path: &PathBuf, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// The world with the requester's pieces: the class file, the adapter, the grounding check, a goals file.
fn world(tag: &str, reply: &str) -> World {
    let w = World::new(tag);
    install_decider(&w, "{\"valid\":true,\"decision\":\"approve\",\"cost_usd\":0.02,\"adapter\":\"fake\"}\n");
    pieces(&w, reply);
    w
}

/// Puts the requester's pieces into a world that has the decider installed.
fn pieces(w: &World, reply: &str) {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    std::fs::copy(repo.join("driver/examples/sdc/decision-classes.json"), w.root.join("driver/examples/sdc/decision-classes.json")).unwrap();
    std::fs::copy(repo.join("bindings/tools/node-classes.json"), w.root.join("bindings/tools/node-classes.json")).unwrap();
    let tools = w.root.join("bindings/tools");
    write_exe(&tools.join("llm-adapter-claude-cli.sh"), ADAPTER);
    std::fs::write(tools.join("reply.json"), reply).unwrap();
    std::fs::write(tools.join("decision.json"), r#"{"decision":"approve","reasons":["fine"],"cites":[]}"#).unwrap();
    std::fs::write(tools.join("response.json"), r#"{"adapter":"fake","model":"claude-opus-5-5","cost_usd":0.02}"#).unwrap();
    write_exe(&w.root.join("tools/decision-grounds.sh"), GROUNDS);
    std::fs::write(w.root.join("tools/verdict.json"), VERIFIED).unwrap();
    std::fs::write(w.root.join("goals.yaml"), GOALS).unwrap();
}

/// A tree that stops once at the approval of `action` and ends.
fn tree(w: &World, action: &str) {
    let f = &w.fake;
    let d = decision_json(action, "ApproveAction", "person:owner").replace("\"case\":\"root\"", "\"case\":\"IDEA-1.direction\"").replace(",\"path\":\"/work/x.yaml\"", "");
    f.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 0, None, &format!("[{d}]")), 3);
    f.answer("tree_approve", "default", "{\"decision\":\"Allow\"}\n", 0);
    f.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
}

fn go(w: &World, decider: Value, goals: bool) -> std::process::Output {
    let mut v = fixture_settings(w, 5_000_000);
    for (k, x) in decider.as_object().unwrap() {
        v["decider"][k] = x.clone();
    }
    write_settings(w, &v);
    let mut args = w.args("r1", 5_000_000);
    if goals {
        args.extend(["--goals".to_owned(), w.root.join("goals.yaml").display().to_string()]);
    }
    w.run(&args)
}

fn principal_of(argv: &[String]) -> String {
    let at = argv.iter().position(|a| a == "--principal").unwrap();
    argv[at + 1].clone()
}

#[test]
fn the_goals_file_is_copied_unchanged_to_the_root_workspace_before_the_tree_opens() {
    let w = World::new("goals-copy");
    w.one_decision_tree();
    w.scripted_approve();
    std::fs::write(w.root.join("goals.yaml"), GOALS).unwrap();
    let mut args = w.args("r1", 5_000_000);
    args.extend(["--goals".to_owned(), w.root.join("goals.yaml").display().to_string()]);
    let o = w.run(&args);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(std::fs::read_to_string(w.run_dir("r1/work/goals.yaml")).unwrap(), GOALS);
    let opened = w.fake.calls_of(&["tree", "open"]);
    assert_eq!(opened.len(), 1, "the tree is opened once");
}

#[test]
fn a_run_without_goals_has_no_goals_file() {
    let w = World::new("goals-none");
    w.one_decision_tree();
    w.scripted_approve();
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(!w.run_dir("r1/work/goals.yaml").exists());
}

#[test]
fn a_goals_path_that_is_no_file_is_refused_with_exit_64_and_nothing_is_written() {
    let w = World::new("goals-missing");
    w.scripted_approve();
    let mut args = w.args("r1", 5_000_000);
    args.extend(["--goals".to_owned(), w.root.join("nowhere.yaml").display().to_string()]);
    let o = w.run(&args);
    assert_eq!(code(&o), 64, "{}", stderr(&o));
    assert!(stderr(&o).contains("--goals needs a goals file"), "{}", stderr(&o));
    assert!(!w.run_dir("r1").exists() && !w.ledger().exists());
    assert_eq!(w.fake.calls_so_far(), 0);
}

#[test]
fn the_requester_decider_without_goals_is_refused_before_anything_is_written() {
    let w = world("goals-needed", APPROVE_REPLY);
    let o = go(&w, json!({"requester": true}), false);
    assert_ne!(code(&o), 0);
    assert!(stderr(&o).contains("decider.requester: the requester answers from the goals document; give it with --goals"), "{}", stderr(&o));
    assert!(!w.run_dir("r1").exists());
    assert_eq!(w.fake.calls_so_far(), 0);
}

// cycle entry: requester-decider
#[test]
fn requester_decider_a_requester_side_approval_is_answered_from_the_goals_as_decider_requester() {
    let w = world("requester-decider", APPROVE_REPLY);
    tree(&w, "vision.approve");
    let started = std::time::Instant::now();
    let o = go(&w, json!({"requester": true}), true);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(started.elapsed().as_secs() < 5, "slow: {:?}", started.elapsed());
    assert_eq!(w.outcome("r1")["outcome"], "ended");
    let approve = w.fake.calls_of(&["tree", "approve"]);
    assert_eq!(approve.len(), 1);
    assert_eq!(principal_of(&approve[0].argv), "decider:requester");
    let dir = w.run_dir("r1");
    let decisions = lines(&dir.join("decisions.jsonl"));
    assert_eq!(decisions.len(), 1, "{decisions:?}");
    let line = &decisions[0];
    assert_eq!((line["decision"].as_str(), line["principal"].as_str(), line["action"].as_str()), (Some("approve"), Some("decider:requester"), Some("vision.approve")));
    let record = std::fs::read_dir(dir.join("decider")).unwrap().next().unwrap().unwrap().path();
    let body: Value = serde_json::from_str(&std::fs::read_to_string(record.join("decide-request.json")).unwrap()).unwrap();
    assert_eq!((body["class"].as_str(), body["side"].as_str(), body["autonomy"].as_str()), (Some("vision"), Some("requester"), Some("A5")));
    let prompt = std::fs::read_to_string(record.join("prompt.md")).unwrap();
    assert!(prompt.contains("goals"), "the requester reads the goals: {prompt}");
    assert_eq!(std::fs::read_to_string(record.join("cwd/in/goals.yaml")).unwrap(), GOALS);
    assert_eq!(spent(&w), 20_000);
}

#[test]
fn a_delivering_side_approval_is_not_taken_by_the_requester() {
    let w = world("requester-delivering", APPROVE_REPLY);
    tree(&w, "merge.approve");
    let o = go(&w, json!({"requester": true}), true);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let approve = w.fake.calls_of(&["tree", "approve"]);
    assert_eq!(principal_of(&approve[0].argv), "decider:model");
    let record = std::fs::read_dir(w.run_dir("r1/decider")).unwrap().next().unwrap().unwrap().path();
    let body: Value = serde_json::from_str(&std::fs::read_to_string(record.join("decide-request.json")).unwrap()).unwrap();
    assert_ne!(body["side"].as_str(), Some("requester"), "{body}");
}

const QUESTION_REQUEST: &str = r#"{"request": {
  "request_id": "DR-IDEA-1.intake-scope.answer-none", "state": "Open", "question_key": "IDEA-1.intake|scope.answer",
  "subject_path": "work/questions.yaml", "opened_at": "2026-01-02T03:04:05Z", "decision_class": "scope_changing_answer",
  "options": [{"letter": "A", "does": "a one-time code", "consequence": {"answer": "Sign in with a one-time code"}}, {"letter": "B", "does": "a password"}],
  "grounds": []}}"#;

/// The scope questions of `answers.rs` with the requester: the stand-in `sdc-decision-request` beside
/// `sdc-reply`, the requester's pieces, and a goals file.
fn question_world(tag: &str, reply: &str) -> World {
    let w = super::answers::world(tag, true);
    pieces(&w, reply);
    write_exe(&w.root.join("sdc-decision-request"), "#!/bin/sh\ncat \"$(dirname \"$0\")/question-request.json\"\n");
    std::fs::write(w.root.join("question-request.json"), QUESTION_REQUEST).unwrap();
    super::answers::question_tree(&w, super::answers::QUESTIONS_TWO);
    w
}

// cycle entry: requester-decider (the questions of the scope.answer step)
#[test]
fn requester_decider_the_scope_questions_are_answered_by_decider_requester_on_verified_grounds() {
    let w = question_world("requester-questions", &reply_for("A"));
    let o = go(&w, json!({"requester": true}), true);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let q = std::fs::read_to_string(w.run_dir("r1/work/questions.yaml")).unwrap();
    assert_eq!(q.matches("  answer: Sign in with a one-time code\n  answered_by: decider:requester\n").count(), 2, "{q}");
    let decisions = lines(&w.run_dir("r1/decisions.jsonl"));
    assert_eq!(decisions.len(), 2, "{decisions:?}");
    for line in &decisions {
        assert_eq!((line["type"].as_str(), line["class"].as_str(), line["side"].as_str()), (Some("question"), Some("scope_changing_answer"), Some("requester")));
        assert_eq!((line["decision"].as_str(), line["principal"].as_str()), (Some("approve"), Some("decider:requester")));
    }
    let record = w.run_dir("r1/decider");
    let first = std::fs::read_dir(&record).unwrap().map(|e| e.unwrap().path()).min().unwrap();
    assert_eq!(std::fs::read_to_string(first.join("cwd/in/goals.yaml")).unwrap(), GOALS);
    assert!(std::fs::read_to_string(first.join("grounds-check.json")).unwrap().contains("verified"));
}

// cycle entry: requester-no-ground (the question)
#[test]
fn requester_no_ground_a_question_takes_its_default_under_final_and_stops_for_a_person_under_stop() {
    let w = question_world("requester-q-no-ground", NO_GROUND_REPLY);
    let o = go(&w, json!({"requester": true, "escalate": "final"}), true);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let q = std::fs::read_to_string(w.run_dir("r1/work/questions.yaml")).unwrap();
    assert!(q.contains("  answer: A\n") && q.contains("  answer: C\n"), "each question takes its default: {q}");
    let decisions = lines(&w.run_dir("r1/decisions.jsonl"));
    assert_eq!(decisions.len(), 4, "{decisions:?}");
    assert_eq!(decisions[0]["decision"], "escalation_suppressed");
    assert_eq!(decisions[0]["side"], "requester");
    let w = question_world("requester-q-no-ground-stop", NO_GROUND_REPLY);
    let o = go(&w, json!({"requester": true, "escalate": "stop"}), true);
    assert_eq!(code(&o), 8, "{}", stderr(&o));
    assert!(w.fake.calls_of(&["tree", "approve"]).is_empty());
    assert!(!std::fs::read_to_string(w.run_dir("r1/work/questions.yaml")).unwrap().contains("answered_by"));
}

fn reply_for(option: &str) -> String {
    APPROVE_REPLY.replace("\"A\"", &format!("\"{option}\""))
}

// cycle entry: requester-no-ground
#[test]
fn requester_no_ground_stops_the_run_for_a_person_under_stop() {
    let w = world("requester-no-ground-stop", NO_GROUND_REPLY);
    tree(&w, "vision.approve");
    let o = go(&w, json!({"requester": true, "escalate": "stop"}), true);
    assert_eq!(code(&o), 8, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "stopped: decider");
    let reason = outcome["reason"].as_str().unwrap();
    assert!(reason.contains("reserved for a person: the decider escalated (escalate stop)") && reason.contains("no ground in the goals"), "{reason}");
    assert!(w.fake.calls_of(&["tree", "approve"]).is_empty(), "nothing was approved");
    assert!(lines(&w.run_dir("r1/decisions.jsonl")).is_empty());
    let stop: Value = serde_json::from_str(&std::fs::read_to_string(w.run_dir("r1/run-stopped.json")).unwrap()).unwrap();
    assert_eq!(stop["cause"], "decider_cannot_decide");
    let record = std::fs::read_dir(w.run_dir("r1/decider")).unwrap().next().unwrap().unwrap().path();
    let body: Value = serde_json::from_str(&std::fs::read_to_string(record.join("decide-request.json")).unwrap()).unwrap();
    assert_eq!((body["class"].as_str(), body["side"].as_str()), (Some("vision"), Some("requester")));
}

#[test]
fn requester_no_ground_under_final_takes_the_best_option_and_says_so() {
    let w = world("requester-no-ground-final", NO_GROUND_REPLY);
    tree(&w, "vision.approve");
    let o = go(&w, json!({"requester": true, "escalate": "final"}), true);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let decisions = lines(&w.run_dir("r1/decisions.jsonl"));
    assert_eq!(decisions.len(), 1, "{decisions:?}");
    assert_eq!((decisions[0]["decision"].as_str(), decisions[0]["principal"].as_str()), (Some("approve"), Some("decider:requester")));
    assert_eq!(principal_of(&w.fake.calls_of(&["tree", "approve"])[0].argv), "decider:requester");
}
