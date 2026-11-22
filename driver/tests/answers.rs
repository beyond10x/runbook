//! `sdc-answers`, the programs of `bindings/tools/answers.yaml`: the requests the decider is asked
//! and the writing of its answers into `questions.yaml` and `findings.yaml` of a workspace, as
//! `scripts/run-trial.sh` writes them.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const QUESTIONS: &str = "questions:
- id: q1
  scope_changing: true
  status: open
  default: One shared list
  options: [One shared list, Separate lists]
- id: q2
  scope_changing: false
  status: open
  default: x
- id: q3
  scope_changing: true
  status: accepted
  answer: Separate lists
  answered_by: person:owner
  default: Separate lists
- id: q4
  scope_changing: true
  status: changed
  answer: Both
  answered_by: non-interactive
  default: One
";

const FINDINGS: &str = "findings:
- id: f1
  status: needs_person
  default: keep-both
  options: [keep-both, drop-one]
- id: f2
  status: dismissed
- id: f3
  status: needs_person
  decision: drop-one
  decided_by: decider:model
- id: f4
  status: needs_person
  default: a
  refused_decision: {decided_by: 'decider:model', decision: b, why: the check says no}
- id: f5
  status: needs_person
  refused_decision: {decided_by: 'person:owner', decision: b, why: no}
";

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_sdc-answers"))
}

fn workspace(name: &str, questions: &str, findings: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("answers").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("questions.yaml"), questions).unwrap();
    fs::write(dir.join("findings.yaml"), findings).unwrap();
    dir
}

fn run(dir: &PathBuf, args: &[&str]) -> Output {
    Command::new(bin()).args(args).current_dir(dir).output().unwrap()
}

fn lines(o: &Output) -> Vec<serde_json::Value> {
    String::from_utf8_lossy(&o.stdout).lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

fn read(dir: &Path, file: &str) -> serde_yaml::Value {
    serde_yaml::from_str(&fs::read_to_string(dir.join(file)).unwrap()).unwrap()
}

#[test]
fn only_the_scope_changing_questions_nobody_with_the_right_answered_are_requested() {
    let dir = workspace("q-requests", QUESTIONS, FINDINGS);
    let o = run(&dir, &["requests", "questions"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let got = lines(&o);
    let ids: Vec<&str> = got.iter().map(|l| l["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["q1", "q4"], "q2 is not scope-changing, q3 was answered by a person");
    assert_eq!(got[0]["type"], "question");
    assert_eq!(got[0]["question"]["default"], "One shared list");
    assert_eq!((got[0]["subject"].as_str(), got[0]["path"].as_str(), got[0]["capability"].as_str()), (Some("questions"), Some("questions.yaml"), Some("scope.answer")));
}

#[test]
fn an_answer_is_accepted_when_it_is_the_default_and_changed_when_it_is_not() {
    let dir = workspace("q-apply", QUESTIONS, FINDINGS);
    assert!(run(&dir, &["apply", "question", "q1", "One shared list", "decider:model"]).status.success());
    assert!(run(&dir, &["apply", "question", "q4", "Neither", "decider:model"]).status.success());
    let doc = read(&dir, "questions.yaml");
    let q = &doc["questions"];
    assert_eq!((q[0]["answer"].as_str(), q[0]["status"].as_str(), q[0]["answered_by"].as_str()), (Some("One shared list"), Some("accepted"), Some("decider:model")));
    assert_eq!((q[3]["answer"].as_str(), q[3]["status"].as_str(), q[3]["answered_by"].as_str()), (Some("Neither"), Some("changed"), Some("decider:model")));
    assert_eq!(q[2]["answered_by"].as_str(), Some("person:owner"), "an answer of a person is left alone");
    let again = lines(&run(&dir, &["requests", "questions"]));
    assert!(again.is_empty(), "nothing waits any more: {again:?}");
}

#[test]
fn findings_that_wait_are_requested_and_a_refused_decision_of_the_decider_comes_with_its_refusal() {
    let dir = workspace("f-requests", QUESTIONS, FINDINGS);
    let o = run(&dir, &["requests", "findings"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let got = lines(&o);
    let ids: Vec<&str> = got.iter().map(|l| l["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["f1", "f4"], "f2 is dismissed, f3 decided, f5 was refused after a person's decision");
    assert_eq!(got[0]["type"], "finding");
    assert_eq!(got[0]["finding"]["options"], serde_json::json!(["keep-both", "drop-one"]));
    assert_eq!(got[0]["capability"], serde_json::Value::Null);
    assert_eq!(got[1]["refusal"]["why"], "the check says no");
    assert!(got[1]["finding"].get("refused_decision").is_none(), "the request holds the finding without it");
}

#[test]
fn a_decision_is_written_with_its_principal_and_clears_the_refusal() {
    let dir = workspace("f-apply", QUESTIONS, FINDINGS);
    assert!(run(&dir, &["apply", "finding", "f4", "a", "decider:model"]).status.success());
    let doc = read(&dir, "findings.yaml");
    let f = &doc["findings"][3];
    assert_eq!((f["decision"].as_str(), f["decided_by"].as_str()), (Some("a"), Some("decider:model")));
    assert!(f.get("refused_decision").is_none());
    let o = run(&dir, &["apply", "finding", "nope", "a", "decider:model"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("no finding nope"));
}

#[test]
fn a_finding_raised_again_after_its_decision_was_applied_is_a_defect_of_that_apply() {
    let findings = "findings:
- id: f1
  status: needs_person
  reraised: {decided_by: 'decider:model', decision: keep-both, says: the same contradiction}
";
    let dir = workspace("f-reraised", QUESTIONS, findings);
    let o = run(&dir, &["requests", "findings"]);
    assert_eq!(o.status.code(), Some(1));
    let why = String::from_utf8_lossy(&o.stderr);
    assert!(why.contains("f1 raised again after decider:model's decision \"keep-both\" was applied") && why.contains("did not take effect"), "{why}");
}

#[test]
fn unreadable_input_and_bad_usage_exit_1_with_the_reason() {
    let dir = workspace("usage", QUESTIONS, FINDINGS);
    fs::remove_file(dir.join("questions.yaml")).unwrap();
    let o = run(&dir, &["requests", "questions"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("questions.yaml"));
    let o = run(&dir, &["requests", "nothing"]);
    assert_eq!(o.status.code(), Some(1));
    let o = run(&dir, &[]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("usage"));
}
