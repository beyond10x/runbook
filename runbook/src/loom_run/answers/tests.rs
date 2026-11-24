//! Tests of the answers file: its loader, the request lines, the answer-step rule, and the file
//! committed in `bindings/`.

use super::*;
use crate::assets::testdir::Dir;
use std::path::PathBuf;

fn file_with(tag: &str, text: &str) -> (Dir, PathBuf) {
    let dir = Dir::new(tag);
    let tools = dir.dir("tools");
    std::fs::write(tools.join("answers.yaml"), text).unwrap();
    let path = dir.path().to_path_buf();
    (dir, path)
}

fn refused(tag: &str, text: &str) -> String {
    let (_dir, path) = file_with(tag, text);
    Answers::load(&path).unwrap_err()
}

const GOOD: &str = "format: runbook.loom-run-answers/1\nanswers:\n- action: a.one\n  on: ApproveAction\n  requests: [prog, ask, '{case}']\n  apply: [prog, put, '{id}', '{answer}', '{principal}', '{case}', '{type}']\n  then: [prog, take, '{principal}', '{case}']\n- action: a.two\n  on: Handoff\n  requests: [prog, ask]\n  apply: [prog, put, '{id}']\n";

#[test]
fn a_file_with_declarations_is_read_in_full() {
    let (_dir, path) = file_with("answers-good", GOOD);
    let answers = Answers::load(&path).unwrap();
    let one = answers.find("ApproveAction", "a.one").unwrap();
    assert_eq!(one.requests, ["prog", "ask", "{case}"]);
    assert_eq!(one.then.as_deref().unwrap(), ["prog", "take", "{principal}", "{case}"]);
    assert!(answers.find("Handoff", "a.one").is_none(), "a declaration is for one kind of decision");
    assert!(answers.find("Handoff", "a.two").unwrap().then.is_none());
    assert!(answers.declares("a.two") && !answers.declares("a.three"));
}

#[test]
fn no_file_is_no_declaration() {
    let dir = Dir::new("answers-none");
    let answers = Answers::load(dir.path()).unwrap();
    assert!(!answers.declares("a.one"));
}

#[test]
fn a_file_that_is_wrong_is_refused_naming_what_is_wrong() {
    for (what, text, want) in [
        ("format", GOOD.replace("loom-run-answers/1", "loom-run-answers/2"), "format is not"),
        ("top-key", format!("{GOOD}extra: 1\n"), "is not a key of"),
        ("kind", GOOD.replace("on: Handoff", "on: Approve"), "on: one of"),
        ("placeholder", GOOD.replace("'{type}'", "'{nope}'"), "{nope} is not a name"),
        ("then-placeholder", GOOD.replace("'{principal}', '{case}']\n- action", "'{id}']\n- action"), "{id} is not a name"),
        ("brace", GOOD.replace("'{case}']\n  apply", "'{case']\n  apply"), "never closes"),
        ("twice", format!("{GOOD}- action: a.one\n  on: ApproveAction\n  requests: [p]\n  apply: [p]\n"), "declared twice"),
        ("empty", GOOD.replace("requests: [prog, ask]", "requests: []"), "names no program"),
        ("member", GOOD.replace("  then: [prog, take, '{principal}', '{case}']\n", "  then: [prog, take, '{principal}', '{case}']\n  more: 1\n"), "is not a key"),
        ("not-yaml", "format: [".to_owned(), "answers.yaml"),
    ] {
        let message = refused(&format!("answers-bad-{what}"), &text);
        assert!(message.contains(want) && message.contains("tools/answers.yaml"), "{what}: {message}");
    }
}

fn entry(text: &str) -> Yaml {
    serde_yaml::from_str(text).unwrap()
}

#[test]
fn only_a_person_step_without_a_recorded_result_is_an_answer_step() {
    let answer = entry("action: s.answer\nexecutor: {kind: person, principal: person:owner}\nbudget: {max_runs: 8}\n");
    let with_effect = entry("action: s.go\nexecutor: {kind: person, principal: person:owner}\nevidence:\n- {from: effect, name: performed}\n");
    let with_file = entry("action: s.go\nexecutor: {kind: person, principal: person:owner}\nevidence:\n- {from: file, name: note}\n");
    let program = entry("action: s.run\nexecutor: {kind: program, argv: [x]}\n");
    assert!(is_answer_step(&answer));
    assert!(is_answer_step(&with_file), "a file is not a result");
    assert!(!is_answer_step(&with_effect));
    assert!(!is_answer_step(&program));

    let (_dir, path) = file_with("answers-takes", GOOD);
    let answers = Answers::load(&path).unwrap();
    assert!(!answers.decider_takes("s.answer", &answer), "undeclared: a person");
    assert!(answers.decider_takes("a.one", &answer), "declared: the decider may take it");
    assert!(answers.decider_takes("s.go", &with_effect));
}

#[test]
fn request_lines_are_checked_one_by_one() {
    let good = "{\"type\":\"question\",\"id\":\"q1\",\"question\":{\"default\":\"x\"},\"path\":\"p\"}\n\n{\"type\":\"finding\",\"id\":\"f1\",\"finding\":{}}\n";
    let requests = parse_requests(good).unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!((requests[0].kind.as_str(), requests[0].id.as_str()), ("question", "q1"));
    assert_eq!(requests[0].body["path"], "p");
    assert_eq!(requests[1].kind, "finding");
    assert!(parse_requests("").unwrap().is_empty());
    for (text, want) in [
        ("nonsense", "request line 1: not JSON"),
        ("{\"type\":\"other\",\"id\":\"a\"}", "type is question or finding"),
        ("{\"type\":\"question\",\"id\":\"\",\"question\":{}}", "id: a non-empty text"),
        ("{\"type\":\"question\",\"id\":\"a\"}", "member question is not an object"),
        ("{\"type\":\"question\",\"id\":\"a\",\"question\":{}}\n[1]", "request line 2"),
    ] {
        let message = parse_requests(text).unwrap_err();
        assert!(message.contains(want), "{text}: {message}");
    }
}

#[test]
fn the_reasons_text_is_one_line_per_answer() {
    let given = vec![("q1".to_owned(), "yes\nand more".to_owned()), ("q2".to_owned(), "no".to_owned())];
    assert_eq!(reasons_text(&given), "q1: yes and more\nq2: no");
}

#[test]
fn the_tail_keeps_the_last_three_lines() {
    assert_eq!(tail("a\n\nb\nc\nd\n"), "b | c | d");
    assert_eq!(tail(""), "");
}

#[test]
fn a_decision_line_has_the_members_the_metrics_count_by() {
    let decision = json!({"case": "c1", "action": "scope.answer", "capability": "cap"});
    let requests = parse_requests("{\"type\":\"question\",\"id\":\"q1\",\"question\":{}}").unwrap();
    let reply = json!({"answer": "yes", "reasons": "because"});
    let line = decision_line(&decision, &requests[0], "r1", "approve", &reply, "decider:model", json!({"suppressed": true}));
    assert_eq!(line["class"], "scope_changing_answer");
    assert_eq!((line["decision"].as_str(), line["answer"].as_str(), line["principal"].as_str()), (Some("approve"), Some("yes"), Some("decider:model")));
    assert_eq!(line["question"], "q1");
    assert!(line["finding"].is_null());
    assert_eq!(line["suppressed"], true);
    assert_eq!(line["capability"], "cap");
}

#[test]
fn the_file_committed_in_the_bindings_is_valid_and_declares_both_steps() {
    let bindings = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("bindings");
    let answers = Answers::load(&bindings).unwrap();
    let question = answers.find("ApproveAction", "scope.answer").unwrap();
    assert!(question.then.is_none(), "the program of scope.answer is the binding's own: Loom runs it at the approval");
    assert!(answers.find("Handoff", "amend.upstream").unwrap().then.is_none());
    assert!(is_answer_step_in(&bindings, "scope.answer"));
    assert!(!is_answer_step_in(&bindings, "amend.upstream"));
}

#[test]
fn a_link_a_program_put_at_a_bindings_file_is_not_read_for_the_answer_steps() {
    let dir = Dir::new("answer-step-link");
    let bindings = dir.dir("bindings");
    let outside = dir.dir("outside");
    let step = "actions:\n- action: scope.answer\n  executor: {kind: person}\n";
    std::fs::write(outside.join("steps.yaml"), step).unwrap();
    std::os::unix::fs::symlink(outside.join("steps.yaml"), bindings.join("b.yaml")).unwrap();
    assert!(!is_answer_step_in(&bindings, "scope.answer"), "a file outside the run was read through a link at a bindings file");
    std::fs::write(bindings.join("c.yaml"), step).unwrap();
    assert!(is_answer_step_in(&bindings, "scope.answer"));
}
