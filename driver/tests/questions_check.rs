//! `questions.check` (`check-questions` in `driver/examples/sdc/tools/sdc-tool.sh`) on a scope-changing
//! question's context keys (`story:interview-question-context`): `excerpt`, `default_reason` and a
//! `consequences` entry `{does, cost}` for every option. The shape is the one
//! `sdc-decision-request` parses.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

const DISSECTION: &str = "items:
  - {id: d1, type: ambiguity, span: for me and my partner, note: whose list}
  - {id: d2, type: wish, span: to-do list, note: a list}
";

const OPTIONS: &str = "  options:
  - One shared list
  - Separate lists
  default: One shared list
";
const EXCERPT: &str = "  excerpt: for me and my partner\n";
const REASON: &str = "  default_reason: The idea says one partner, so one list.\n";
const CONSEQUENCES: &str = "  consequences:
  - does: Builds one list both read and edit.
    cost: The smallest build.
  - does: Builds a list per user with sharing.
    cost: Roughly twice the build.
";
const ONE_CONSEQUENCE: &str = "  consequences:
  - does: Builds one list both read and edit.
    cost: The smallest build.
";
const EMPTY_COST: &str = "  consequences:
  - does: Builds one list both read and edit.
    cost: The smallest build.
  - does: Builds a list per user with sharing.
    cost: ''
";

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn tool() -> PathBuf {
    manifest().join("examples/sdc/tools/sdc-tool.sh")
}

/// A case's work/ with one dissection and the given questions.yaml; runs `check-questions` in it.
fn check_in(name: &str, questions: &str) -> Output {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("questions-check")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("dissection.yaml"), DISSECTION).unwrap();
    fs::write(dir.join("questions.yaml"), questions).unwrap();
    Command::new("bash")
        .arg(tool())
        .arg("check-questions")
        .current_dir(&dir)
        .output()
        .unwrap()
}

/// q1 about d1, scope-changing and open, with the given context blocks between its keys.
fn scope_question(blocks: &[&str], tail: &str) -> String {
    format!(
        "questions:\n- id: q1\n  item: d1\n  question: One list or two?\n{}{}  scope_changing: true\n{tail}",
        OPTIONS,
        blocks.concat()
    )
}

const OPEN: &str = "  status: open\n";
const SETTLED: &str = "  status: accepted\n  answer: One shared list\n  answered_by: operator\n";

fn code(o: &Output) -> i32 {
    o.status.code().unwrap()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn refused_naming(case: &str, blocks: &[&str], key: &str) {
    let o = check_in(case, &scope_question(blocks, OPEN));
    assert_eq!(code(&o), 2, "{case}: stderr: {}", stderr(&o));
    let e = stderr(&o);
    assert!(
        e.contains("q1") && e.contains(key),
        "{case}: stderr names q1 and {key}: {e}"
    );
}

#[test]
fn scope_changing_without_excerpt_is_refused() {
    refused_naming("no-excerpt", &[REASON, CONSEQUENCES], "excerpt");
}

#[test]
fn scope_changing_without_default_reason_is_refused() {
    refused_naming("no-reason", &[EXCERPT, CONSEQUENCES], "default_reason");
}

#[test]
fn scope_changing_without_consequences_is_refused() {
    refused_naming("no-consequences", &[EXCERPT, REASON], "consequences");
}

#[test]
fn scope_changing_with_a_consequence_missing_for_an_option_is_refused() {
    refused_naming(
        "short-consequences",
        &[EXCERPT, REASON, ONE_CONSEQUENCE],
        "consequences",
    );
}

#[test]
fn scope_changing_with_an_empty_cost_is_refused() {
    refused_naming("empty-cost", &[EXCERPT, REASON, EMPTY_COST], "consequences");
}

#[test]
fn scope_changing_with_all_three_exits_as_before() {
    let blocks = [EXCERPT, REASON, CONSEQUENCES];
    let open = check_in("all-open", &scope_question(&blocks, OPEN));
    assert_eq!(code(&open), 1, "open: stderr: {}", stderr(&open));
    let settled = check_in("all-settled", &scope_question(&blocks, SETTLED));
    assert_eq!(code(&settled), 0, "settled: stderr: {}", stderr(&settled));
}

#[test]
fn a_question_that_is_not_scope_changing_needs_none_of_them() {
    let q = "questions:\n- id: q1\n  item: d1\n  question: One list or two?\n  options:\n  - a\n  - b\n  default: a\n  scope_changing: false\n";
    let open = check_in("plain-open", &format!("{q}  status: open\n"));
    assert_eq!(code(&open), 1, "stderr: {}", stderr(&open));
    let settled = check_in(
        "plain-settled",
        &format!("{q}  status: accepted\n  answer: a\n"),
    );
    assert_eq!(code(&settled), 0, "stderr: {}", stderr(&settled));
}

#[test]
fn every_missing_key_of_a_question_is_named() {
    let o = check_in("all-missing", &scope_question(&[], OPEN));
    assert_eq!(code(&o), 2);
    let e = stderr(&o);
    for key in ["excerpt", "default_reason", "consequences"] {
        assert!(e.contains(key), "{key} named: {e}");
    }
}

/// The questions fixtures keep the exit they had before the keys (never 2: their shape is valid).
#[test]
fn the_questions_fixtures_pass_the_check() {
    let fx = manifest().join("examples/sdc/fixtures");
    let dissection = fs::read_to_string(fx.join("dissection.yaml")).unwrap();
    for (name, want) in [
        ("questions-open.yaml", 1),
        ("questions-answered.yaml", 0),
        ("scope-question/questions-open.yaml", 1),
        ("scope-question/questions-answered.yaml", 1),
    ] {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("questions-check")
            .join(name.replace('/', "-"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("dissection.yaml"), &dissection).unwrap();
        fs::copy(fx.join(name), dir.join("questions.yaml")).unwrap();
        let o = Command::new("bash")
            .arg(tool())
            .arg("check-questions")
            .current_dir(&dir)
            .output()
            .unwrap();
        assert_eq!(code(&o), want, "{name}: stderr: {}", stderr(&o));
    }
}

/// Every scope-changing question in a fixture carries the three keys, one consequence per option.
#[test]
fn scope_changing_questions_in_fixtures_carry_the_keys() {
    let fx = manifest().join("examples/sdc/fixtures");
    for name in [
        "questions-open.yaml",
        "questions-answered.yaml",
        "scope-question/questions-open.yaml",
        "scope-question/questions-answered.yaml",
        "offline-decision/questions-answered.yaml",
        "questions-trial-2.yaml",
    ] {
        let doc: serde_yaml::Value =
            serde_yaml::from_str(&fs::read_to_string(fx.join(name)).unwrap()).unwrap();
        for q in doc["questions"].as_sequence().unwrap() {
            if q["scope_changing"].as_bool() != Some(true) {
                continue;
            }
            let id = q["id"].as_str().unwrap();
            let options = q["options"].as_sequence().map_or(0, Vec::len);
            let consequences = q["consequences"].as_sequence().map_or(0, Vec::len);
            assert!(options >= 2, "{name} {id}: options");
            assert_eq!(consequences, options, "{name} {id}: consequences");
            assert!(q["excerpt"].as_str().is_some_and(|s| !s.is_empty()));
            assert!(q["default_reason"].as_str().is_some_and(|s| !s.is_empty()));
        }
    }
}
