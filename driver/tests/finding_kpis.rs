//! `sdc-finding-kpis` on the two kinds of run directory (docs/kpis.md § 4, docs/loom-run-directory.md,
//! "The two evaluation tools").
//!
//! The Loom fixture `tests/fixtures/loom-run/` is written by hand in the shape `runbook loom-run`
//! writes (the member table of docs/loom-run-directory.md), with invented text only: no stub run
//! made it and no real run directory was copied. The driver fixture is `tests/fixtures/autopilot-stopped/`
//! plus a small run that this file writes, with a person action, a forced continuation, a reopened
//! stop and a revision.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("finding-kpis").join(name);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap().flatten() {
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if src.is_dir() {
            copy_tree(&src, &dst);
        } else {
            fs::copy(&src, &dst).unwrap();
        }
    }
}

fn kpis(run: &Path, extra: &[&str]) -> (i32, Value, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_sdc-finding-kpis")).arg(run).args(extra).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let doc = serde_json::from_str(&text).unwrap_or(Value::Null);
    (out.status.code().unwrap_or(-1), doc, text, String::from_utf8_lossy(&out.stderr).into_owned())
}

const NO_SOURCE: &str = "no source in a runbook loom-run directory";

#[test]
fn a_loom_run_directory_gives_the_kpis_its_files_hold_and_null_for_the_rest() {
    let (code, k, _, err) = kpis(&fixture("loom-run"), &["--json"]);
    assert_eq!(code, 0, "{err}");
    // L1: one person action (spec.approve), questions 1, finding decisions 1, context fields 2.
    assert_eq!(k["L1_human_interventions"]["value"], 5);
    assert_eq!(k["L1_human_interventions"]["person_actions"], 1);
    assert_eq!(k["L1_human_interventions"]["person_answers"], json!({"context_fields": 2, "finding_decisions": 1, "findings_raised": 0, "questions": 1}));
    // A Loom run's settings have no `stand_in` member: null, not false.
    assert_eq!(k["L1_human_interventions"]["stand_in"], Value::Null);
    assert_eq!(k["L1_human_interventions"]["stand_in_by"], Value::Null);
    // L2 from rounds.jsonl (20 s, 32 s, 1 s over a 55 s span).
    assert_eq!(k["L2_autonomous_seconds"]["rounds"], 3);
    assert_eq!(k["L2_autonomous_seconds"]["value"], 20);
    assert_eq!(k["L2_autonomous_seconds"]["max"], 32);
    assert_eq!(k["L2_autonomous_seconds"]["sum"], 53);
    // L7: closed by status.json, the decider is a model, nothing waited on a person.
    assert_eq!(k["L7_waiting_on_person_seconds"]["value"], 0);
    assert_eq!(k["L7_waiting_on_person_seconds"]["failure"], false);
    assert_eq!(k["L7_waiting_on_person_seconds"]["decider"], "model");
    assert_eq!(k["L7_waiting_on_person_seconds"]["ongoing"], false);
    // L3 to L5 from the product tree's work/ files.
    assert_eq!(k["L3_defaults_accepted"], json!({"findings": {"value": 1, "of": 2}, "questions": {"value": 1, "of": 2}, "context": {"value": 2, "of": 3}}));
    assert_eq!(k["L4_late_findings"], json!({"value": 2, "by_stage": {"plan": 1, "spec": 1}}));
    assert_eq!(k["L5_caught_by"]["program"], 1);
    assert_eq!(k["L5_caught_by"]["llm"], 2);
    assert_eq!(k["L5_caught_by"]["resolved_by_person"], 1);
    // No source under Loom: null, with the reason.
    assert_eq!(k["L6_rework"]["value"], Value::Null);
    assert!(k["L6_rework"]["why"].as_str().unwrap().contains("revised"));
    for (key, members) in [
        ("forced", vec!["value"]),
        ("unstuck", vec!["value"]),
        ("stopped_for_fix", vec!["reopens", "seconds"]),
        ("clean_run", vec!["value", "forced", "unstuck", "reopens"]),
    ] {
        for m in members {
            assert_eq!(k[key][m], Value::Null, "{key}.{m}");
        }
        assert!(k[key]["why"].as_str().unwrap().starts_with(NO_SOURCE), "{key}");
    }
}

#[test]
fn the_text_form_prints_null_and_the_reason_for_a_figure_a_loom_run_does_not_hold() {
    let (code, _, text, err) = kpis(&fixture("loom-run"), &[]);
    assert_eq!(code, 0, "{err}");
    let line = text.lines().find(|l| l.starts_with("clean_run: ")).expect("a clean_run line");
    assert!(line.contains("\"value\":null") && line.contains(NO_SOURCE), "{line}");
}

#[test]
fn the_case_store_is_found_from_the_run_directory_not_from_a_constant() {
    // The same run with its root case stored under another name.
    let d = scratch("renamed");
    copy_tree(&fixture("loom-run"), &d);
    fs::rename(d.join("store/IDEA-1"), d.join("store/CASE-7")).unwrap();
    let (code, k, _, err) = kpis(&d, &["--json"]);
    assert_eq!(code, 0, "{err}");
    let (_, want, _, _) = kpis(&fixture("loom-run"), &["--json"]);
    assert_eq!(k, want);
}

#[test]
fn a_driver_run_directory_keeps_its_figures() {
    let (code, k, _, err) = kpis(&fixture("autopilot-stopped"), &["--json"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(k["L1_human_interventions"]["person_answers"], json!({"context_fields": 0, "finding_decisions": 1, "findings_raised": 0, "questions": 1}));
    assert_eq!(k["L1_human_interventions"]["stand_in"], true);
    assert_eq!(k["L2_autonomous_seconds"]["value"], 600);
    assert_eq!(k["forced"]["value"], 0);
    assert_eq!(k["clean_run"]["value"], true);
    assert_eq!(k["L6_rework"]["value"], 0);
    for key in ["forced", "unstuck", "stopped_for_fix", "clean_run", "L6_rework"] {
        assert!(k[key].get("why").is_none(), "{key} has no `why` in a driver run");
    }
}

fn jsonl(path: &Path, lines: &[Value]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
}

#[test]
fn a_driver_run_counts_a_person_action_a_forced_continuation_a_reopen_and_a_revision() {
    let d = scratch("driver");
    let root = d.join("store/IDEA-1");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("case.json"), "{}").unwrap();
    jsonl(&root.join("events.jsonl"), &[
        json!({"event": "opened", "at": "2026-11-22T09:00:00Z", "protocol": {"id": "sdc", "revision": 1}}),
        json!({"event": "revised", "artifacts": {"idea": "t1"}}),
        json!({"event": "revised", "artifacts": {"idea": "t2"}}),
        json!({"event": "action_finished", "action": "idea.accept", "at": "2026-11-22T09:00:05Z", "exit": "0", "principal": "person:reviewer-1"}),
    ]);
    jsonl(&d.join("defects.jsonl"), &[json!({"cause": "invented", "case": "IDEA-1", "action": "idea.accept", "forced": true})]);
    jsonl(&d.join("run-settings.jsonl"), &[json!({"at": "2026-11-22T09:00:00Z", "decider": {"kind": "person"}})]);
    let (code, k, _, err) = kpis(&d, &["--json"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(k["L1_human_interventions"]["person_actions"], 1);
    assert_eq!(k["L1_human_interventions"]["stand_in"], false);
    assert_eq!(k["forced"]["value"], 1);
    assert_eq!(k["clean_run"]["value"], false);
    assert_eq!(k["L6_rework"]["value"], 1);
    assert_eq!(k["L6_rework"]["by_artifact"], json!({"idea": 1}));
}

#[test]
fn a_directory_of_neither_kind_is_refused_with_exit_2_naming_both_files() {
    let d = scratch("empty");
    let (code, _, text, err) = kpis(&d, &["--json"]);
    assert_eq!(code, 2, "{err}");
    assert!(text.is_empty(), "{text}");
    assert!(err.contains("settings.json") && err.contains("events.jsonl"), "{err}");
    let (code, _, _, err) = kpis(&d.join("missing"), &[]);
    assert_eq!(code, 2, "{err}");
}
