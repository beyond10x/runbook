//! What the runner does for a decision whose action the answers file of the bindings declares
//! (`tools/answers.yaml`, `src/loom_run/answers.rs`): ask the decider once per request, write each
//! answer into the workspace through the file's programs, take the step, then run the command Loom
//! printed. The programs here are POSIX sh the test writes; the runner knows the file's format and
//! nothing about the work files. A fake `b10x-loom` and a fake decider adapter: no model call.

use super::core::{code, spent, stderr, World};
use super::decider::{fixture_settings, install_decider, lines, set_reply, write_settings};
use super::fake::{decision_json, tree_run_json};
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

const BINDINGS: &str = "format: loom.bindings/1
protocol: sdc@8
actions:
  - action: scope.answer
    executor:
      kind: person
      principal: person:owner
    budget: {max_runs: 8}
  - action: amend.upstream
    executor:
      kind: program
      argv: [tools/amend.sh]
    handoff: decide the findings
";

const ANSWERS: &str = "format: runbook.loom-run-answers/1
answers:
- action: scope.answer
  on: ApproveAction
  requests: [tools/q-requests.sh]
  apply: [tools/q-apply.sh, '{id}', '{answer}', '{principal}']
  then: [tools/then.sh, '{principal}', '{case}']
- action: amend.upstream
  on: Handoff
  requests: [tools/f-requests.sh]
  apply: [tools/f-apply.sh, '{id}', '{answer}', '{principal}']
";

const ENV_NOTE: &str = "env >> \"$PWD/program-env.txt\"\n";

const Q_REQUESTS: &str = "awk '
function flush() { if (id != \"\" && sc == \"true\" && st == \"open\") printf \"{\\\"type\\\":\\\"question\\\",\\\"id\\\":\\\"%s\\\",\\\"question\\\":{\\\"id\\\":\\\"%s\\\",\\\"default\\\":\\\"%s\\\"}}\\n\", id, id, df }
/^- id: / { flush(); id = $3; sc = \"\"; st = \"\"; df = \"\"; next }
/^  scope_changing: / { sc = $2 }
/^  status: / { st = $2 }
/^  default: / { df = $2 }
END { flush() }' questions.yaml
";

const Q_APPLY: &str = "awk -v id=\"$1\" -v ans=\"$2\" -v who=\"$3\" '
/^- id: / { cur = ($3 == id) }
cur && /^  status: / { print \"  status: changed\"; print \"  answer: \" ans; print \"  answered_by: \" who; next }
{ print }' questions.yaml > questions.yaml.new && mv questions.yaml.new questions.yaml
";

const F_REQUESTS: &str = "awk '
function flush() { if (id != \"\" && st == \"needs_person\" && dec == \"\") printf \"{\\\"type\\\":\\\"finding\\\",\\\"id\\\":\\\"%s\\\",\\\"finding\\\":{\\\"id\\\":\\\"%s\\\",\\\"default\\\":\\\"%s\\\"}}\\n\", id, id, df }
/^- id: / { flush(); id = $3; st = \"\"; df = \"\"; dec = \"\"; next }
/^  status: / { st = $2 }
/^  default: / { df = $2 }
/^  decision: / { dec = $2 }
END { flush() }' findings.yaml
";

const F_APPLY: &str = "awk -v id=\"$1\" -v ans=\"$2\" -v who=\"$3\" '
/^- id: / { cur = ($3 == id) }
{ print }
cur && /^  status: / { print \"  decision: \" ans; print \"  decided_by: \" who }' findings.yaml > findings.yaml.new && mv findings.yaml.new findings.yaml
";

pub(super) const QUESTIONS_TWO: &str = "questions:
- id: q1
  scope_changing: true
  status: open
  default: A
- id: q2
  scope_changing: true
  status: open
  default: C
- id: q3
  scope_changing: false
  status: open
  default: Z
";

const FINDINGS_ONE: &str = "findings:
- id: f1
  status: needs_person
  default: keep-both
- id: f2
  status: dismissed
  default: x
";

fn write_exe(path: &PathBuf, body: &str) {
    std::fs::write(path, body).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn reply(answer: &str) -> String {
    format!("{{\"valid\":true,\"decision\":\"approve\",\"answer\":\"{answer}\",\"reasons\":[\"it follows from the idea\"],\"cites\":[],\"cost_usd\":0.02,\"adapter\":\"fake\",\"model\":\"claude-opus-5-5\"}}\n")
}

const ESCALATE: &str = "{\"valid\":true,\"decision\":\"escalate\",\"reasons\":[\"the idea does not say\"],\"cites\":[],\"cost_usd\":0.02,\"adapter\":\"fake\",\"model\":\"claude-opus-5-5\"}\n";

/// The world: bindings with the two actions, the answers file (unless `answers` is false), its
/// programs, and a fake decider that answers `B` to everything.
pub(super) fn world(tag: &str, answers: bool) -> World {
    let w = World::new(tag);
    install_decider(&w, &reply("B"));
    let tools = w.root.join("bindings/tools");
    std::fs::write(w.root.join("bindings/sdc.yaml"), BINDINGS).unwrap();
    for (name, body) in [("q-requests.sh", Q_REQUESTS), ("q-apply.sh", Q_APPLY), ("f-requests.sh", F_REQUESTS), ("f-apply.sh", F_APPLY)] {
        write_exe(&tools.join(name), &format!("#!/bin/sh\n{ENV_NOTE}{body}"));
    }
    write_exe(&tools.join("then.sh"), &format!("#!/bin/sh\n{ENV_NOTE}printf '%s\\n' \"$*\" >> then.log\n"));
    if answers {
        std::fs::write(tools.join("answers.yaml"), ANSWERS).unwrap();
    }
    w
}

fn scope_decision() -> String {
    decision_json("scope.answer", "ApproveAction", "person:owner").replace("\"case\":\"root\"", "\"case\":\"IDEA-1.intake\"")
}

/// A tree that stops once at a scope.answer approval with `questions` in the workspace, then ends.
pub(super) fn question_tree(w: &World, questions: &str) {
    let f = &w.fake;
    f.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 0, None, &format!("[{}]", scope_decision())), 3);
    f.hook("tree_run", "1", &format!("cat > work/questions.yaml <<'EOF'\n{questions}EOF\n"));
    f.answer("tree_approve", "default", "{\"decision\":\"Allow\"}\n", 0);
    f.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
}

fn handoff_tree(w: &World, findings: &str) {
    let f = &w.fake;
    let handoff = "{\"action\":\"amend.upstream\",\"case\":\"IDEA-1.intake\",\"command\":[\"b10x-loom\",\"tree\",\"run\",\"--store\",\"@STORE@\",\"--confinement\",\"local\"],\"decides\":\"decide the findings\",\"kind\":\"Handoff\"}";
    f.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 2, None, &format!("[{handoff}]")), 3);
    f.hook("tree_run", "1", &format!("cat > work/findings.yaml <<'EOF'\n{findings}EOF\n"));
    f.answer("tree_run", "2", &tree_run_json("BudgetSpent", 1, Some("the run took its steps"), "[]"), 5);
    f.answer("tree_run", "3", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
}

fn settings(w: &World, decider: Value) -> Value {
    let mut v = fixture_settings(w, 5_000_000);
    for (k, x) in decider.as_object().unwrap() {
        v["decider"][k] = x.clone();
    }
    v
}

fn go(w: &World, decider: Value) -> std::process::Output {
    write_settings(w, &settings(w, decider));
    w.run(&w.args("r1", 5_000_000))
}

fn read(w: &World, file: &str) -> String {
    std::fs::read_to_string(w.run_dir("r1/work").join(file)).unwrap_or_default()
}

#[test]
fn two_scope_questions_are_answered_one_by_one_and_the_step_is_taken_by_the_decider() {
    let w = world("ans-questions", true);
    set_reply(&w, ".2", &reply("D"));
    question_tree(&w, QUESTIONS_TWO);
    let started = std::time::Instant::now();
    let o = go(&w, json!({}));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(started.elapsed().as_secs() < 5, "slow: {:?}", started.elapsed());
    assert_eq!(w.outcome("r1")["outcome"], "ended");
    let q = read(&w, "questions.yaml");
    assert!(q.contains("  answer: B\n  answered_by: decider:model\n"), "{q}");
    assert!(q.contains("  answer: D\n  answered_by: decider:model\n"), "{q}");
    assert_eq!(q.matches("answered_by: decider:model").count(), 2, "the third question is not scope-changing: {q}");
    assert_eq!(read(&w, "then.log"), "decider:model IDEA-1.intake\n");
    let dir = w.run_dir("r1");
    let decisions = lines(&dir.join("decisions.jsonl"));
    assert_eq!(decisions.len(), 2, "{decisions:?}");
    for (line, (id, answer)) in decisions.iter().zip([("q1", "B"), ("q2", "D")]) {
        assert_eq!((line["type"].as_str(), line["class"].as_str(), line["question"].as_str()), (Some("question"), Some("scope_changing_answer"), Some(id)));
        assert_eq!((line["decision"].as_str(), line["answer"].as_str(), line["principal"].as_str()), (Some("approve"), Some(answer), Some("decider:model")));
        assert_eq!((line["case"].as_str(), line["action"].as_str(), line["model"].as_str()), (Some("IDEA-1.intake"), Some("scope.answer"), Some("claude-opus-5-5")));
    }
    let approve = w.fake.calls_of(&["tree", "approve"]);
    assert_eq!(approve.len(), 1);
    let at = approve[0].argv.iter().position(|a| a == "--principal").unwrap();
    assert_eq!(approve[0].argv[at + 1], "decider:model");
    let outcome = w.outcome("r1");
    assert_eq!(outcome["llm_calls"], 2);
    assert_eq!(outcome["spent_micro_usd"], 40_000);
    assert_eq!(spent(&w), 40_000);
    assert_eq!(std::fs::read_dir(dir.join("decider")).unwrap().count(), 2, "one record directory per request");
    let request: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("bindings/tools/request.1.json")).unwrap()).unwrap();
    assert_eq!((request["type"].as_str(), request["question"]["id"].as_str(), request["action"].as_str()), (Some("question"), Some("q1"), Some("scope.answer")));
    assert!(std::path::Path::new(request["case_dir"].as_str().unwrap()).join("case.json").is_file());
}

#[test]
fn a_finding_that_needs_a_person_is_answered_at_its_handoff_and_the_run_goes_on() {
    let w = world("ans-finding", true);
    set_reply(&w, "", &reply("keep-both"));
    handoff_tree(&w, FINDINGS_ONE);
    let o = go(&w, json!({}));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(w.outcome("r1")["outcome"], "ended");
    let f = read(&w, "findings.yaml");
    assert!(f.contains("  decision: keep-both\n  decided_by: decider:model\n"), "{f}");
    assert_eq!(f.matches("decided_by").count(), 1, "the dismissed finding is not asked: {f}");
    let store = w.run_dir("r1/store").display().to_string();
    let runs = w.fake.calls_of(&["tree", "run"]);
    assert_eq!(runs.len(), 3, "the first stop, the printed tree run after the answer, the end");
    assert_eq!(runs[1].argv, ["tree", "run", "--store", store.as_str(), "--confinement", "local"]);
    let decisions = lines(&w.run_dir("r1/decisions.jsonl"));
    assert_eq!(decisions.len(), 1);
    assert_eq!((decisions[0]["type"].as_str(), decisions[0]["class"].as_str(), decisions[0]["finding"].as_str()), (Some("finding"), Some("finding"), Some("f1")));
    assert_eq!((decisions[0]["answer"].as_str(), decisions[0]["principal"].as_str()), (Some("keep-both"), Some("decider:model")));
    let request: Value = serde_json::from_str(&std::fs::read_to_string(w.run_dir("r1/bindings/tools/request.1.json")).unwrap()).unwrap();
    assert_eq!((request["type"].as_str(), request["finding"]["id"].as_str()), (Some("finding"), Some("f1")));
}

#[test]
fn an_escalation_under_stop_ends_the_run_for_a_person_with_the_deciders_reasons() {
    let w = world("ans-stop", true);
    set_reply(&w, "", ESCALATE);
    question_tree(&w, QUESTIONS_TWO);
    let o = go(&w, json!({"escalate": "stop"}));
    assert_eq!(code(&o), 8, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "stopped: decider");
    let reason = outcome["reason"].as_str().unwrap();
    assert!(reason.contains("reserved for a person: the decider escalated (escalate stop): the idea does not say") && reason.contains("ApproveAction"), "{reason}");
    assert!(w.fake.calls_of(&["tree", "approve"]).is_empty());
    assert!(!read(&w, "questions.yaml").contains("answered_by"));
    assert!(!w.run_dir("r1/work/then.log").exists(), "the step was not taken");
    let decisions = lines(&w.run_dir("r1/decisions.jsonl"));
    assert_eq!(decisions.len(), 1);
    assert_eq!((decisions[0]["decision"].as_str(), decisions[0]["class"].as_str()), (Some("escalate"), Some("scope_changing_answer")));
    assert_eq!(spent(&w), 20_000, "the call that escalated is booked");
}

#[test]
fn an_escalation_under_final_takes_the_default_of_the_question_and_says_so() {
    let w = world("ans-final", true);
    set_reply(&w, "", ESCALATE);
    question_tree(&w, QUESTIONS_TWO);
    let o = go(&w, json!({"escalate": "final"}));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let q = read(&w, "questions.yaml");
    assert!(q.contains("  answer: A\n") && q.contains("  answer: C\n"), "each question takes its default: {q}");
    let decisions = lines(&w.run_dir("r1/decisions.jsonl"));
    let kinds: Vec<_> = decisions.iter().map(|l| (l["decision"].as_str().unwrap().to_owned(), l["suppressed"] == true)).collect();
    assert_eq!(kinds, [("escalation_suppressed".to_owned(), false), ("approve".to_owned(), true), ("escalation_suppressed".to_owned(), false), ("approve".to_owned(), true)]);
}

#[test]
fn the_decider_call_cap_ends_the_run_when_the_answers_would_exceed_it() {
    let w = world("ans-cap", true);
    question_tree(&w, QUESTIONS_TWO);
    let mut v = settings(&w, json!({}));
    v["decider"]["max_decider_calls"] = json!(1);
    write_settings(&w, &v);
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 8, "{}", stderr(&o));
    let reason = w.outcome("r1")["reason"].as_str().unwrap().to_owned();
    assert!(reason.contains("decider budget spent: 1 decider calls"), "{reason}");
    // `cycles/suite.yaml` decider-budget-reason: the stop reason names the step, the case and the cap.
    assert!(reason.contains("scope.answer in IDEA-1.intake: ") && reason.contains("(max_calls)"), "{reason}");
    let stop: Value = serde_json::from_str(&std::fs::read_to_string(w.run_dir("r1/run-stopped.json")).unwrap()).unwrap();
    assert_eq!(stop["cause"], "decider_cannot_decide");
    assert!(stop["reason"].as_str().unwrap().contains("scope.answer in IDEA-1.intake: decider budget spent"), "{stop}");
    let q = read(&w, "questions.yaml");
    assert_eq!(q.matches("answered_by: decider:model").count(), 1, "the first answer stays, the second was never asked: {q}");
    assert!(w.fake.calls_of(&["tree", "approve"]).is_empty());
    assert_eq!(lines(&w.run_dir("r1/decisions.jsonl")).len(), 1);
    assert_eq!(std::fs::read_to_string(w.run_dir("r1/bindings/tools/n")).unwrap().trim(), "1", "the decider ran once");
}

#[test]
fn an_action_the_file_does_not_declare_ends_as_it_did_before() {
    let w = world("ans-undeclared", false);
    question_tree(&w, QUESTIONS_TWO);
    let o = go(&w, json!({}));
    assert_eq!(code(&o), 8, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "stopped: decider");
    let reason = outcome["reason"].as_str().unwrap();
    assert!(reason.contains("scope.answer") && reason.contains("answers file"), "{reason}");
    assert!(w.fake.calls_of(&["tree", "approve"]).is_empty());
    assert!(!w.run_dir("r1/bindings/tools/n").exists(), "the decider was not started");
    assert!(lines(&w.run_dir("r1/decisions.jsonl")).is_empty());
    assert!(!read(&w, "questions.yaml").contains("answered_by"));
    let copy = std::fs::read_to_string(w.run_dir("r1/bindings/sdc.yaml")).unwrap();
    assert!(copy.contains("person:owner") && !copy.contains("decider:model"), "the step keeps its person: {copy}");
}

#[test]
fn a_declared_action_with_nothing_to_ask_ends_as_it_did_before() {
    let w = world("ans-none", true);
    question_tree(&w, "questions:\n- id: q1\n  scope_changing: false\n  status: open\n  default: Z\n");
    let o = go(&w, json!({}));
    assert_eq!(code(&o), 8, "{}", stderr(&o));
    assert_eq!(w.outcome("r1")["outcome"], "stopped: decider");
    assert!(!w.run_dir("r1/bindings/tools/n").exists());
}

#[test]
fn the_programs_of_the_file_get_path_and_the_case_and_nothing_of_the_runners_environment() {
    let w = world("ans-env", true);
    question_tree(&w, QUESTIONS_TWO);
    let o = go(&w, json!({}));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let env = read(&w, "program-env.txt");
    assert!(env.lines().any(|l| l.starts_with("PATH=")) && env.contains("LOOM_CASE=IDEA-1.intake"), "{env}");
    for needle in ["SDC_SECRET_MARKER", "marker-secret-value", "ANTHROPIC_API_KEY", "marker-home-dir", "LOGIN-FILE-MARKER", "HOME=", "XDG_RUNTIME_DIR"] {
        assert!(!env.contains(needle), "{needle} reached a program of the file:\n{env}");
    }
    let names: Vec<&str> = env.lines().filter_map(|l| l.split_once('=').map(|(k, _)| k)).collect();
    for name in names {
        assert!(matches!(name, "PATH" | "LOOM_CASE" | "PWD" | "SHLVL" | "_" | "OLDPWD"), "{name} is not PATH or LOOM_*:\n{env}");
    }
}

#[test]
fn a_request_that_comes_back_a_fourth_time_ends_the_run_naming_it() {
    let w = world("ans-again", true);
    write_exe(&w.root.join("bindings/tools/q-apply.sh"), "#!/bin/sh\ntrue\n");
    let f = &w.fake;
    f.answer("tree_run", "default", &tree_run_json("AwaitingPerson", 0, None, &format!("[{}]", scope_decision())), 3);
    f.hook("tree_run", "1", &format!("cat > work/questions.yaml <<'EOF'\n{}EOF\n", "questions:\n- id: q1\n  scope_changing: true\n  status: open\n  default: A\n"));
    f.answer("tree_approve", "default", "{\"decision\":\"Allow\"}\n", 0);
    let o = go(&w, json!({}));
    assert_eq!(code(&o), 1, "{}", stderr(&o));
    assert_eq!(w.outcome("r1")["outcome"], "error");
    let reason = w.outcome("r1")["reason"].as_str().unwrap().to_owned();
    assert!(reason.contains("q1") && reason.contains("3 times"), "{reason}");
    assert_eq!(w.fake.calls_of(&["tree", "approve"]).len(), 3);
    assert_eq!(lines(&w.run_dir("r1/decisions.jsonl")).len(), 3);
}

#[test]
fn a_program_that_fails_ends_the_run_as_an_error_with_its_last_lines() {
    let w = world("ans-apply-fails", true);
    write_exe(&w.root.join("bindings/tools/q-apply.sh"), "#!/bin/sh\necho 'cannot write the answer' >&2\nexit 3\n");
    question_tree(&w, QUESTIONS_TWO);
    let o = go(&w, json!({}));
    assert_eq!(code(&o), 1, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "error");
    let reason = outcome["reason"].as_str().unwrap();
    assert!(reason.contains("q-apply.sh") && reason.contains("exited 3") && reason.contains("cannot write the answer"), "{reason}");
    assert!(w.fake.calls_of(&["tree", "approve"]).is_empty());
}

#[test]
fn a_file_the_runner_cannot_read_refuses_the_run_before_the_first_step_names_it() {
    let w = world("ans-bad-file", true);
    std::fs::write(w.root.join("bindings/tools/answers.yaml"), "format: runbook.loom-run-answers/1\nanswers:\n- action: scope.answer\n  on: ApproveAction\n  requests: [tools/q-requests.sh]\n  apply: [tools/q-apply.sh, '{nope}']\n").unwrap();
    question_tree(&w, QUESTIONS_TWO);
    let o = go(&w, json!({}));
    assert_eq!(code(&o), 1, "{}", stderr(&o));
    let reason = w.outcome("r1")["reason"].as_str().unwrap().to_owned();
    assert!(reason.contains("answers.yaml") && reason.contains("{nope}"), "{reason}");
    assert!(w.fake.calls_of(&["tree", "run"]).is_empty());
}
