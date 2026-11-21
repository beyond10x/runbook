//! `sdc-run-compare` against the fixtures in `tests/fixtures/` (docs/run-compare.md).
//!
//! `trial-2-train` and `trial-2b-fresh` are the recorded runs of the same names with every file
//! the tool does not read removed, every absolute path and transcript stripped, and the principal
//! of the operator renamed; the tool's figures for them are the figures of `docs/trial-2.md`.
//! `autopilot-closed` and `autopilot-stopped` are written by hand for the records of the
//! unattended run (`run-settings.jsonl`, `run-stopped.json`, `person:` principals, `replay.json`,
//! `oracle.json`) that no recorded run has yet.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("run-compare").join(name);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn tool(args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sdc-run-compare")).args(args).output().unwrap()
}

fn run_arg(label: &str, dir: &Path) -> Vec<String> {
    vec!["--run".into(), format!("{label}={}", dir.display())]
}

/// Compare two runs; returns (exit code, stderr, markdown, json).
fn compare(a: &Path, b: &Path, tag: &str) -> (i32, String, String, Value) {
    let out = scratch(tag);
    let mut args = run_arg("a", a);
    args.extend(run_arg("b", b));
    args.extend(["--out".into(), out.join("report.md").display().to_string()]);
    args.extend(["--json".into(), out.join("report.json").display().to_string()]);
    let o = tool(&args);
    let md = fs::read_to_string(out.join("report.md")).unwrap_or_default();
    let js = fs::read_to_string(out.join("report.json")).map(|t| serde_json::from_str(&t).unwrap()).unwrap_or(Value::Null);
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stderr).into_owned(), md, js)
}

/// The value of one metric of the run at `idx` (0 = a, 1 = b).
fn metric<'a>(js: &'a Value, idx: usize, name: &str) -> &'a Value {
    js["runs"][idx]["metrics"].as_array().unwrap().iter().find(|m| m["metric"] == name)
        .unwrap_or_else(|| panic!("no metric {name:?} in run {idx}"))
        .get("value").unwrap()
}

fn num(js: &Value, idx: usize, name: &str) -> f64 {
    metric(js, idx, name).as_f64().unwrap_or_else(|| panic!("metric {name:?} of run {idx} is {:?}", metric(js, idx, name)))
}

fn text<'a>(js: &'a Value, idx: usize, name: &str) -> &'a str {
    metric(js, idx, name).as_str().unwrap_or_else(|| panic!("metric {name:?} of run {idx} is {:?}", metric(js, idx, name)))
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn recorded_runs_agree_with_the_trial_2_figures() {
    let (code, err, _, js) = compare(&fixture("trial-2-train"), &fixture("trial-2b-fresh"), "recorded");
    assert_eq!(code, 0, "stderr: {err}");
    // docs/trial-2.md: 27 work calls $0.503, 5 decider calls $0.024, 17 findings, 14 late,
    // 14 resolved by agent default and 2 by a person, rework 58, 18 rounds and 1009 s in the driver
    assert_eq!(num(&js, 0, "LLM calls"), 27.0);
    assert!((num(&js, 0, "LLM cost (USD)") - 0.503).abs() < 0.0005);
    assert_eq!(num(&js, 0, "decider calls"), 5.0);
    assert!((num(&js, 0, "decider cost (USD)") - 0.024).abs() < 0.0005);
    assert_eq!(num(&js, 0, "findings raised"), 17.0);
    assert_eq!(num(&js, 0, "findings late"), 14.0);
    assert_eq!(num(&js, 0, "findings early"), 3.0);
    assert_eq!(num(&js, 0, "findings resolved by agent default"), 14.0);
    assert_eq!(num(&js, 0, "findings resolved by decider"), 0.0);
    assert_eq!(num(&js, 0, "findings resolved by person"), 2.0);
    assert_eq!(num(&js, 0, "findings unresolved"), 1.0);
    assert_eq!(num(&js, 0, "rework revisions"), 58.0);
    assert_eq!(num(&js, 0, "time inside the driver (s)"), 1009.0);
    assert_eq!(num(&js, 0, "LLM calls [claude-haiku-5-5]"), 27.0);
    assert_eq!(num(&js, 0, "LLM calls [change.implement]"), 8.0);
    assert_eq!(text(&js, 0, "end state").split(", by ").next().unwrap(), "stopped: awaiting_person in IDEA-1.todo.access (last round ended in IDEA-1.todo.lists)");
    assert_eq!(text(&js, 0, "oracle check"), "none");
    assert_eq!(text(&js, 0, "replay"), "not recorded");
    // the operator's two approvals and answers, in the older bare-id form of the principal
    assert_eq!(num(&js, 0, "decisions by a person"), 9.0);
    // first event 00:53:37, last event 02:21:31 (docs/trial-2.md: 5274 s)
    assert_eq!(num(&js, 0, "elapsed wall time (s)"), 5274.0);
    assert_eq!(num(&js, 0, "time outside the driver (s)"), 4265.0);

    assert_eq!(num(&js, 1, "LLM calls"), 3.0);
    assert_eq!(num(&js, 1, "findings raised"), 0.0);
    assert!(text(&js, 1, "end state").starts_with("stopped: awaiting_person in IDEA-1.intake, by agent:sdc-orchestrator: not continued"));
}

#[test]
fn same_inputs_give_the_same_bytes_and_no_path_or_clock() {
    let a = fixture("trial-2-train");
    let b = fixture("trial-2b-fresh");
    let (_, _, md1, js1) = compare(&a, &b, "det-1");
    let (_, _, md2, js2) = compare(&a, &b, "det-2");
    assert_eq!(md1, md2);
    assert_eq!(js1, js2);
    for s in [md1, js1.to_string()] {
        assert!(!s.contains(concat!("/ho", "me/")), "absolute path in the output");
        assert!(!s.contains("det-1") && !s.contains("target/tmp"), "scratch path in the output");
        assert!(!s.to_lowercase().contains("generated"), "wall-clock line in the output");
        assert!(s.contains("trial-2-train") && s.contains("trial-2b-fresh"), "run directory names are printed");
    }
}

#[test]
fn comparison_has_one_line_per_metric_with_both_values_and_the_difference() {
    let (_, _, md, _) = compare(&fixture("trial-2-train"), &fixture("trial-2b-fresh"), "table");
    let (head, table) = md.split_once("## Comparison").expect("a comparison section");
    assert!(head.contains("## a: trial-2-train") && head.contains("## b: trial-2b-fresh"));
    assert!(table.contains("| metric | a | b | b - a |"));
    assert!(table.contains("| LLM calls | 27 | 3 | -24 |"), "{table}");
    assert!(table.contains("| findings raised | 17 | 0 | -17 |"), "{table}");
    assert!(table.contains("| replay | not recorded | not recorded | same |"), "{table}");
    for key in ["end state", "elapsed wall time (s)", "time inside the driver (s)", "time waiting on a person (s)",
        "decisions by a person", "LLM cost (USD)", "decider calls", "findings late", "findings early",
        "findings resolved by agent default", "findings resolved by decider", "findings resolved by person",
        "findings dismissed", "rework revisions", "oracle check", "replay"] {
        assert_eq!(table.lines().filter(|l| l.starts_with(&format!("| {key} |"))).count(), 1, "{key}");
    }
}

#[test]
fn closed_unattended_run_reports_every_field() {
    let (code, err, md, js) = compare(&fixture("autopilot-closed"), &fixture("autopilot-stopped"), "closed");
    assert_eq!(code, 0, "stderr: {err}");
    assert_eq!(text(&js, 0, "end state"), "closed: operating");
    assert_eq!(num(&js, 0, "elapsed wall time (s)"), 2460.0);
    assert_eq!(num(&js, 0, "time inside the driver (s)"), 2455.0);
    assert_eq!(num(&js, 0, "time waiting on a person (s)"), 0.0);
    assert_eq!(num(&js, 0, "decisions by a person"), 0.0);
    assert_eq!(num(&js, 0, "LLM calls"), 4.0);
    assert!(close(num(&js, 0, "LLM cost (USD)"), 0.652));
    assert_eq!(num(&js, 0, "LLM calls [claude-sonnet-5-5]"), 3.0);
    assert!(close(num(&js, 0, "LLM cost (USD) [claude-sonnet-5-5]"), 0.65));
    assert_eq!(num(&js, 0, "LLM calls [claude-haiku-5-5]"), 1.0);
    assert_eq!(num(&js, 0, "LLM calls [spec.author]"), 2.0);
    assert!(close(num(&js, 0, "LLM cost (USD) [spec.author]"), 0.35));
    // Every decider call that was made, whatever became of its reply, once: the escalation
    // suppressed afterwards (its cost is on the `escalation_suppressed` line, none on the line
    // after it), and three invalid replies of which the last is repeated by the fallback line.
    assert_eq!(num(&js, 0, "decider calls"), 6.0);
    assert!(close(num(&js, 0, "decider cost (USD)"), 0.016));
    assert!(close(num(&js, 0, "total cost (USD)"), 0.668));
    assert_eq!(num(&js, 0, "findings raised"), 3.0);
    assert_eq!(num(&js, 0, "findings late"), 2.0);
    assert_eq!(num(&js, 0, "findings early"), 1.0);
    assert_eq!(num(&js, 0, "findings resolved by agent default"), 1.0);
    assert_eq!(num(&js, 0, "findings resolved by decider"), 1.0);
    assert_eq!(num(&js, 0, "findings resolved by person"), 0.0);
    assert_eq!(num(&js, 0, "findings dismissed"), 1.0);
    assert_eq!(num(&js, 0, "findings rejected by the program check"), 1.0);
    assert_eq!(num(&js, 0, "rework revisions"), 1.0);
    assert_eq!(text(&js, 0, "oracle check"), "held (2 of 2 checks)");
    assert_eq!(text(&js, 0, "replay"), "2 of 3 cases match");
    assert!(text(&js, 0, "setting").contains("autonomy A5") && text(&js, 0, "setting").contains("decider haiku (claude-haiku-5-5)"));
    assert!(md.contains("spec.author") && md.contains("claude-sonnet-5-5"));
}

#[test]
fn stopped_run_names_its_reason_and_counts_a_person() {
    let (code, err, _, js) = compare(&fixture("autopilot-closed"), &fixture("autopilot-stopped"), "stopped");
    assert_eq!(code, 0, "stderr: {err}");
    assert_eq!(text(&js, 1, "end state"),
        "stopped: awaiting_person in IDEA-1.intake, by operator: stopped to look at the questions before the decider takes them");
    // first event to last event, from the events' `at` (this run has no observed.jsonl)
    assert_eq!(num(&js, 1, "elapsed wall time (s)"), 600.0);
    // the last round ended awaiting_person at 10:10:00; the stop record is 10:12:00
    assert_eq!(num(&js, 1, "time waiting on a person (s)"), 120.0);
    // one action (its authority record at the same instant counts once), one answer, one finding decision
    assert_eq!(num(&js, 1, "decisions by a person"), 3.0);
    assert!(metric(&js, 1, "LLM calls").is_null(), "an absent llm-calls.jsonl is not recorded, not 0");
    assert!(metric(&js, 1, "decider calls").is_null());
    assert_eq!(text(&js, 1, "replay"), "not recorded");
    assert_eq!(text(&js, 1, "oracle check"), "none");
    assert_eq!(num(&js, 1, "findings unresolved"), 1.0);
}

#[test]
fn trial_2_wait_is_measured_per_stop_not_per_leg() {
    // The recorded run has run-settings.jsonl lines only for its last three legs: the earlier
    // stops are read from the verdict of the round, the later ones from the leg boundary.
    let (_, _, _, js) = compare(&fixture("trial-2-train"), &fixture("trial-2b-fresh"), "wait");
    let wait = num(&js, 0, "time waiting on a person (s)");
    let outside = num(&js, 0, "time outside the driver (s)");
    assert!(wait > 0.0 && wait <= outside, "wait {wait}, outside {outside}");
}

#[test]
fn unreadable_run_directory_exits_2_naming_the_missing_file() {
    let out = scratch("missing");
    let mut args = run_arg("a", &fixture("trial-2-train"));
    args.extend(run_arg("b", &out.join("no-such-run")));
    args.extend(["--out".into(), out.join("report.md").display().to_string()]);
    let o = tool(&args);
    assert_eq!(o.status.code(), Some(2));
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("store/IDEA-1/events.jsonl"), "{err}");
    assert!(err.contains("no-such-run"), "{err}");
    assert!(!out.join("report.md").exists(), "no report is written for a run that cannot be read");

    // a run directory with no event log
    let empty = out.join("empty-run");
    fs::create_dir_all(empty.join("store/IDEA-1")).unwrap();
    let mut args = run_arg("a", &fixture("trial-2-train"));
    args.extend(run_arg("b", &empty));
    args.extend(["--out".into(), out.join("report.md").display().to_string()]);
    let o = tool(&args);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("empty-run/store/IDEA-1/events.jsonl"));
}

#[test]
fn a_malformed_record_exits_2_naming_file_and_line() {
    let out = scratch("malformed");
    let run = out.join("run");
    fs::create_dir_all(run.join("store/IDEA-1")).unwrap();
    fs::write(run.join("store/IDEA-1/events.jsonl"), "{\"seq\":1,\"event\":\"revised\",\"at\":\"2026-11-22T10:00:00Z\"}\n").unwrap();
    fs::write(run.join("llm-calls.jsonl"), "{\"action\":\"a\"}\nnot json\n").unwrap();
    let mut args = run_arg("a", &fixture("trial-2-train"));
    args.extend(run_arg("b", &run));
    args.extend(["--out".into(), out.join("report.md").display().to_string()]);
    let o = tool(&args);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("llm-calls.jsonl:2"));
}

#[test]
fn usage_errors_exit_64() {
    let out = scratch("usage");
    let md = out.join("report.md").display().to_string();
    let one = [run_arg("a", &fixture("trial-2-train")), vec!["--out".into(), md.clone()]].concat();
    assert_eq!(tool(&one).status.code(), Some(64), "one run is no comparison");
    let no_out = [run_arg("a", &fixture("trial-2-train")), run_arg("b", &fixture("trial-2b-fresh"))].concat();
    assert_eq!(tool(&no_out).status.code(), Some(64), "--out is required");
    let dup = [run_arg("a", &fixture("trial-2-train")), run_arg("a", &fixture("trial-2b-fresh")), vec!["--out".into(), md]].concat();
    assert_eq!(tool(&dup).status.code(), Some(64), "labels are distinct");
}
