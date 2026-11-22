//! Does the committed authored suite of `ess/` go red when a refusal of `sdc.decision` is weakened?
//!
//! `ess/scenarios/decision-*.yaml` are run by nothing in this repository against a program
//! (`runbook/tests/daemon/conformance.rs` counts them out of its domain), so the only thing that
//! can hold the refusals of `ess/domains/decision.yaml` is ESS's own interpreter over the
//! specification. Each test copies the specification to a private directory, weakens one guard
//! with a text replacement, authors the suite from the copy's scenarios and runs it on the
//! interpreter: the mutant is killed when any of those steps fails. `ess` is the pinned one
//! (`sdc-conform ess-path`, `driver/ess-pin.json`: `sdc-conform ess-install` once, or `ESS_BIN`);
//! a test without it fails with the reason, it does not skip.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const CONFORM: &str = env!("CARGO_BIN_EXE_sdc-conform");

fn ess() -> PathBuf {
    let o = Command::new(CONFORM).arg("ess-path").output().unwrap();
    assert!(o.status.success(), "the pinned ess is not installed (run `sdc-conform ess-install` or set ESS_BIN): {}", String::from_utf8_lossy(&o.stderr));
    PathBuf::from(String::from_utf8_lossy(&o.stdout).trim())
}

fn spec() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ess")
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let target = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            fs::copy(e.path(), &target).unwrap();
        }
    }
}

/// `Ok` when the interpreter passes every scenario of the specification with `from` replaced by
/// `to` in `domains/decision.yaml` (the mutant survived); `Err` with the step's output otherwise.
fn survives(name: &str, mutation: Option<(&str, &str)>) -> Result<(), String> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("decision-contract-mutants").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    for f in ["system.yaml", "components.yaml", "ess-inputs.yaml"] {
        fs::copy(spec().join(f), dir.join(f)).unwrap();
    }
    copy_dir(&spec().join("domains"), &dir.join("domains"));
    copy_dir(&spec().join("scenarios"), &dir.join("scenarios"));
    if let Some((from, to)) = mutation {
        let file = dir.join("domains/decision.yaml");
        let text = fs::read_to_string(&file).unwrap();
        assert_eq!(text.matches(from).count(), 1, "the text to weaken is not there exactly once: {from}");
        fs::write(&file, text.replacen(from, to, 1)).unwrap();
    }
    let ess = ess();
    let step = |args: &[&str]| -> Result<(), String> {
        let out = Command::new(&ess).args(args).current_dir(&dir).output().expect("the pinned ess starts");
        if out.status.success() {
            Ok(())
        } else {
            Err(format!("ess {}: {}{}", args.join(" "), String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
        }
    };
    step(&["specify", "validate", "--path", "."])?;
    step(&["verify", "conform", "author", "--path", ".", "--scenarios", ".", "--out", "suite.json"])?;
    step(&["verify", "conform", "run", "--target", "interpreted", "--path", ".", "--suite", "suite.json", "--report-format", "2", "--report-out", "report.json"])
}

const DECIDE_GUARD: &str = "        when: defined(option)\n        when_subject:\n          predicate: {not: {exists: {in: options, as: o, that: o.letter == input.option}}}";
const RECOMMEND_GUARD: &str = "        when: {all: [check_verdict == verified, defined(recommended_option)]}";

#[test]
fn the_unmutated_specification_passes_its_scenarios_on_the_interpreter() {
    survives("control", None).unwrap();
}

#[test]
fn an_inverted_decide_guard_is_killed() {
    let renamed = "        when: defined(option)\n        when_subject:\n          predicate: {not: {exists: {in: options, as: o, that: o.letter != input.option}}}";
    assert!(survives("inverted", Some((DECIDE_GUARD, renamed))).is_err(), "an inverted Decide guard survived");
}

#[test]
fn a_decide_guard_that_holds_for_a_person_only_is_killed() {
    let weak = DECIDE_GUARD.replacen("when: defined(option)", "when: {all: [defined(option), principal_kind == person]}", 1);
    if let Ok(()) = survives("decide-person-only", Some((DECIDE_GUARD, &weak))) {
        panic!("OptionNotOffered held for a person only and all 19 scenarios still passed: a decider or a rule that decides an unoffered letter is not tested");
    }
}

#[test]
fn a_recommend_guard_that_holds_for_a_strong_recommendation_only_is_killed() {
    let weak = RECOMMEND_GUARD.replacen("defined(recommended_option)]", "defined(recommended_option), strength == strong]", 1);
    if let Ok(()) = survives("recommend-strong-only", Some((RECOMMEND_GUARD, &weak))) {
        panic!("OptionNotOffered held for a strong recommendation only and all 19 scenarios still passed: a weak or moderate recommendation of an unoffered letter is not tested");
    }
}
