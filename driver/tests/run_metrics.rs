//! `sdc-run-metrics` and `sdc-experiment` on small run directories written by the tests
//! (docs/workflow-metrics.md). Each fixture is a case tree with known takes, so every figure the
//! tools print can be counted by hand from the builder below.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{json, Value};

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("run-metrics").join(name);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn jsonl(path: &Path, lines: &[Value]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    fs::write(path, text).unwrap();
}

fn write_json(path: &Path, v: &Value) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, v.to_string()).unwrap();
}

/// `2026-11-21T10:MM:SS.000Z` for second `s` after 10:00:00.
fn at(s: u32) -> String {
    format!("2026-11-21T10:{:02}:{:02}.000Z", s / 60, s % 60)
}

fn at_whole(s: u32) -> String {
    format!("2026-11-21T10:{:02}:{:02}Z", s / 60, s % 60)
}

/// One case being written: events, observed times, evidence.
struct CaseW {
    dir: PathBuf,
    events: Vec<Value>,
    observed: Vec<Value>,
    t: u32,
    ev_n: usize,
}

impl CaseW {
    fn new(dir: PathBuf, protocol: &str, effects: &[(&str, &str)], manual: &[&str]) -> Self {
        let actions: serde_json::Map<String, Value> = effects.iter().map(|(a, e)| (a.to_string(), json!({"effect": e}))).collect();
        write_json(&dir.join("ir.json"), &json!({"actions": actions}));
        let b: serde_json::Map<String, Value> = effects.iter()
            .map(|(a, _)| (a.to_string(), if manual.contains(a) { json!({"manual": true, "argv": ["true"]}) } else { json!({"argv": ["x"]}) }))
            .collect();
        write_json(&dir.join("bindings.json"), &Value::Object(b));
        let mut c = CaseW { dir, events: vec![], observed: vec![], t: 0, ev_n: 0 };
        c.push(json!({"event": "opened", "at": "-", "protocol": {"id": protocol, "revision": 1}}));
        c.push(json!({"event": "revised", "artifacts": {"spec": "t1"}}));
        c
    }

    fn push(&mut self, e: Value) {
        let i = self.events.len();
        self.observed.push(json!({"format": "sdc.observed/1", "index": i, "event": e["event"], "observed_at": at(self.t)}));
        self.events.push(e);
    }

    /// One take: started at the current second, finished `secs` later, with an evidence record of
    /// `kind`/`result` when `kind` is not empty.
    #[allow(clippy::too_many_arguments)]
    fn take(&mut self, action: &str, llm: bool, retry_of: &[&str], secs: u32, exit: &str, kind: &str, result: Option<&str>, principal: Option<&str>, not_called: Option<&str>) -> u32 {
        let start = self.t;
        self.push(json!({"event": "action_started", "action": action, "llm": llm, "retry_of": retry_of}));
        self.t += secs;
        let mut evidence = vec![];
        if !kind.is_empty() {
            self.ev_n += 1;
            let id = format!("e{:04}-{action}", self.ev_n);
            write_json(&self.dir.join("evidence").join(format!("{id}.json")), &json!({"id": id, "kind": kind, "result": result}));
            evidence.push(id);
        }
        let mut fin = json!({"event": "action_finished", "action": action, "exit": exit, "evidence": evidence, "principal": principal});
        if let Some(n) = not_called {
            fin["not_called"] = json!(n);
        }
        self.push(fin);
        self.t += 1;
        start
    }

    fn finish(mut self, outcome: Option<&str>) {
        if let Some(o) = outcome {
            self.push(json!({"event": "terminated", "outcome": o}));
        }
        jsonl(&self.dir.join("events.jsonl"), &self.events);
        jsonl(&self.dir.join("observed.jsonl"), &self.observed);
    }
}

fn root(run: &Path, outcome: Option<&str>) {
    let c = CaseW::new(run.join("store/IDEA-1"), "sdc", &[], &[]);
    c.finish(outcome);
}

fn settings(run: &Path, decider: &str) {
    jsonl(&run.join("run-settings.jsonl"), &[json!({"format": "sdc.run-settings/1", "at": at_whole(0), "autonomy": "A5",
        "decider": {"kind": "haiku", "model": decider}, "models": {"implement": {"model": "claude-sonnet-5-5"}}})]);
}

/// A closed run with one story case: change.implement is rejected `rejections` times by
/// merge.approve (decider), reworked each time, then approved and merged.
fn story_run(run: &Path, rejections: usize, decider: &str) {
    root(run, Some("operating"));
    settings(run, decider);
    let effects = [("change.implement", "write"), ("tests.run", "read"), ("merge.approve", "write"), ("change.merge", "write")];
    let mut c = CaseW::new(run.join("store/IDEA-1/children/IDEA-1.s"), "sdc.story", &effects, &["merge.approve"]);
    let mut calls = vec![];
    let mut decisions = vec![];
    for k in 0..=rejections {
        let retry: &[&str] = if k == 0 { &[] } else { &["merge_approval"] };
        let s = c.take("change.implement", true, retry, 10, "0", "implementation_change", None, None, None);
        calls.push(json!({"action": "change.implement", "case": "IDEA-1.s", "started": at_whole(s + 1), "model": "claude-sonnet-5-5",
            "class": "implement", "duration_ms": 9000, "cost_usd": if k == 0 { 0.10 } else { 0.05 },
            "usage": {"input_tokens": 10, "output_tokens": 100, "cache_read_input_tokens": 1000, "cache_creation_input_tokens": 50, "thinking_tokens": 5}}));
        c.take("tests.run", false, &[], 2, "0", "conformance_run", Some("passed"), None, None);
        let last = k == rejections;
        let s = c.take("merge.approve", false, &[], 1, "0", "merge_approval", Some(if last { "approved" } else { "rejected" }), Some("decider:haiku"), None);
        decisions.push(json!({"time": at_whole(s), "case": "IDEA-1.s", "type": "approval", "class": "approval", "action": "merge.approve",
            "decision": if last { "approve" } else { "reject" }, "principal": "decider:haiku", "model": decider, "cost_usd": 0.01, "valid": true}));
    }
    c.take("change.merge", false, &[], 1, "0", "merge_record", None, None, None);
    c.finish(Some("passed"));
    jsonl(&run.join("llm-calls.jsonl"), &calls);
    jsonl(&run.join("decisions.jsonl"), &decisions);
    fs::create_dir_all(run.join("store/IDEA-1/work")).unwrap();
    fs::write(run.join("store/IDEA-1/work/findings.yaml"),
        "findings:\n- id: f1\n  stage: story\n  status: resolved\n  resolution:\n    amends: [r1, q1]\n- id: f2\n  stage: spec\n  status: rejected\n").unwrap();
}

fn metrics(run: &Path) -> (i32, Value, String) {
    let o: Output = Command::new(env!("CARGO_BIN_EXE_sdc-run-metrics")).arg(run).args(["--json", "-"]).output().unwrap();
    let doc = serde_json::from_slice(&o.stdout).unwrap_or(Value::Null);
    (o.status.code().unwrap_or(-1), doc, String::from_utf8_lossy(&o.stderr).into_owned())
}

fn node<'a>(doc: &'a Value, name: &str) -> &'a Value {
    doc["nodes"].as_array().unwrap().iter().find(|n| n["node"] == name).unwrap_or_else(|| panic!("no node {name}"))
}

fn edge<'a>(doc: &'a Value, name: &str) -> &'a Value {
    doc["edges"].as_array().unwrap().iter().find(|n| n["edge"] == name)
        .unwrap_or_else(|| panic!("no edge {name} in {:?}", doc["edges"].as_array().unwrap().iter().map(|e| e["edge"].clone()).collect::<Vec<_>>()))
}

fn close(a: &Value, b: f64) -> bool {
    a.as_f64().is_some_and(|x| (x - b).abs() < 1e-9)
}

#[test]
fn a_rejected_approval_is_one_approval_edge_and_a_looping_factor_of_two() {
    let run = scratch("story-1").join("run-a");
    story_run(&run, 1, "claude-haiku-5-5");
    let (code, doc, err) = metrics(&run);
    assert_eq!(code, 0, "{err}");
    assert_eq!(doc["format"], "sdc.run-metrics/1");
    let n = node(&doc, "story/change.implement");
    assert_eq!(n["kind"], "llm-writer");
    assert_eq!((n["takings"].as_u64(), n["real_takes"].as_u64(), n["instances"].as_u64()), (Some(2), Some(2), Some(1)));
    assert!(close(&n["looping_factor"], 2.0) && close(&n["first_pass_yield"], 0.0));
    assert_eq!(n["retries_by_cause"], json!({"merge.approve:rejected": 1}));
    assert_eq!(n["llm_calls"], 2);
    assert!(close(&n["cost_usd"], 0.15) && close(&n["rework_cost_usd"], 0.05), "{n}");
    assert_eq!(n["tokens"], json!({"input": 20, "output": 200, "cache_read": 2000, "cache_creation": 100, "thinking": 10}));
    assert!(close(&n["driver_s"], 20.0) && close(&n["model_s"], 18.0));
    assert_eq!(n["models"], json!({"claude-sonnet-5-5": 2}));
    assert_eq!(n["efforts"], json!({"unrecorded": 2}));
    // The approval node: asked twice, its decider cost on it.
    let a = node(&doc, "story/merge.approve");
    assert_eq!(a["kind"], "approval");
    assert!(close(&a["decider_cost_usd"], 0.02));
    // The loop: the rework take (0.05), tests.run, the second ask (decider 0.01).
    let e = edge(&doc, "story: merge.approve:rejected -> change.implement");
    assert_eq!((e["kind"].as_str(), e["count"].as_u64()), (Some("approval"), Some(1)));
    assert!(close(&e["loop_cost_usd"], 0.06), "{e}");
    assert!(close(&e["loop_driver_s"], 13.0), "{e}");
    // A finding raised at story that amends r1 and q1: one edge to intake.
    assert_eq!(edge(&doc, "story: finding@story -> intake")["count"], 1);
    let d = &doc["decisions"]["by_class"]["approval"];
    assert_eq!((d["asked"].as_u64(), d["approved"].as_u64(), d["rejected"].as_u64(), d["calls"].as_u64()), (Some(2), Some(1), Some(1), Some(2)));
    let m = &doc["metrics"];
    assert_eq!(m["end"], "closed: operating");
    assert_eq!((m["clean"].as_bool(), m["delivered"].as_bool(), m["furthest_stage"].as_str()), (Some(true), Some(true), Some("story")));
    assert!(close(&m["cost_usd"], 0.17) && close(&m["looping_factor"], 2.0));
    assert_eq!(m["findings_raised"], 1);
}

#[test]
fn a_check_sends_the_writer_back_and_a_not_called_run_is_no_take() {
    let run = scratch("spec").join("run-s");
    root(&run, None);
    let effects = [("spec.author", "write"), ("spec.validate", "read")];
    let mut c = CaseW::new(run.join("store/IDEA-1/children/IDEA-1.x"), "sdc.spec", &effects, &[]);
    c.take("spec.author", true, &[], 5, "0", "spec_source", Some("authored"), None, None);
    c.take("spec.validate", false, &[], 1, "1", "spec_validation", Some("invalid"), None, None);
    c.take("spec.author", true, &["spec_validation"], 5, "0", "spec_source", Some("authored"), None, None);
    c.take("spec.validate", false, &[], 1, "0", "spec_validation", Some("valid"), None, None);
    c.take("spec.author", true, &[], 0, "0", "", None, None, Some("llm-node: spec.author not called: own default"));
    c.take("spec.author", true, &[], 0, "75", "", None, None, None);
    c.finish(None);
    write_json(&run.join("run-stopped.json"), &json!({"format": "sdc.run-stopped/1", "at": at_whole(100), "by": "rule:autopilot",
        "verdict_at_stop": "writer_spent", "cause": "writer_spent", "case": "IDEA-1.x"}));
    write_json(&run.join("run-stopped.2026-11-21T10:00:50Z.json"), &json!({"format": "sdc.run-stopped/1", "at": at_whole(50)}));
    jsonl(&run.join("defects.jsonl"), &[json!({"format": "sdc.run-defect/1", "forced": true}), json!({"forced": false})]);
    let (code, doc, err) = metrics(&run);
    assert_eq!(code, 0, "{err}");
    let n = node(&doc, "spec/spec.author");
    assert_eq!((n["takings"].as_u64(), n["real_takes"].as_u64(), n["not_called"].as_u64(), n["could_not_run"].as_u64()), (Some(4), Some(2), Some(1), Some(1)));
    assert_eq!(n["retries_by_cause"], json!({"spec.validate:invalid": 1}));
    let e = edge(&doc, "spec: spec.validate:invalid -> spec.author");
    assert_eq!((e["kind"].as_str(), e["count"].as_u64()), (Some("check"), Some(1)));
    assert_eq!(node(&doc, "spec/spec.validate")["sent_back"], 1);
    let m = &doc["metrics"];
    assert_eq!(m["end"], "stopped: writer_spent in IDEA-1.x");
    assert_eq!((m["clean"].as_bool(), m["reopens"].as_u64(), m["forced"].as_u64(), m["finished"].as_bool()), (Some(false), Some(1), Some(1), Some(true)));
    assert_eq!(m["llm_calls"], Value::Null, "no llm-calls.jsonl: not recorded, never 0");
}

#[test]
fn a_run_without_a_case_tree_is_exit_2() {
    let run = scratch("empty").join("nothing");
    fs::create_dir_all(&run).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_sdc-run-metrics")).arg(&run).output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("store/IDEA-1/events.jsonl"));
    let o = Command::new(env!("CARGO_BIN_EXE_sdc-run-metrics")).output().unwrap();
    assert_eq!(o.status.code(), Some(64));
}

#[test]
fn the_same_run_gives_the_same_bytes() {
    let d = scratch("same");
    story_run(&d.join("r"), 2, "m");
    let a = Command::new(env!("CARGO_BIN_EXE_sdc-run-metrics")).arg(d.join("r")).args(["--json", "-"]).output().unwrap().stdout;
    let b = Command::new(env!("CARGO_BIN_EXE_sdc-run-metrics")).arg(d.join("r")).args(["--json", "-"]).output().unwrap().stdout;
    assert_eq!(a, b);
}

fn experiment(args: &[&str], cwd: &Path) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_sdc-experiment")).args(args).current_dir(cwd).output().unwrap();
    (o.status.code().unwrap_or(-1), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
}

#[test]
fn groups_report_median_range_n_and_overlap() {
    let d = scratch("groups");
    for (name, r) in [("a1", 2), ("a2", 1), ("a3", 2), ("b1", 0), ("b2", 0), ("b3", 1)] {
        story_run(&d.join("runs").join(name), r, if name.starts_with('a') { "haiku" } else { "opus" });
    }
    let (code, out) = experiment(&["compare", "--group", "a=runs/a1,runs/a2,runs/a3", "--group", "b=runs/b1,runs/b2,runs/b3",
        "--out", "g.md", "--json", "g.json"], &d);
    assert_eq!(code, 0, "{out}");
    let g: Value = serde_json::from_str(&fs::read_to_string(d.join("g.json")).unwrap()).unwrap();
    let row = g["metrics"].as_array().unwrap().iter().find(|m| m["metric"] == "node.story/change.implement.real_takes").unwrap();
    assert_eq!(row["groups"][0], json!({"group": "a", "n": 3, "median": 3.0, "min": 2.0, "max": 3.0}));
    assert_eq!(row["groups"][1], json!({"group": "b", "n": 3, "median": 1.0, "min": 1.0, "max": 2.0}));
    assert_eq!(row["differences"][0]["separate"], false, "ranges [2,3] and [1,2] overlap");
    // An edge that never occurred in b reads 0 there (each b run took change.implement).
    let e = g["metrics"].as_array().unwrap().iter().find(|m| m["metric"] == "edge.story: merge.approve:rejected -> change.implement.count").unwrap();
    assert_eq!(e["groups"][1]["n"], 3);
    // Grouped by a setting key instead.
    let (code, out) = experiment(&["compare", "--by", "decider.model", "--runs", "runs/a1,runs/b1,runs/b2", "--out", "k.md"], &d);
    assert_eq!(code, 0, "{out}");
    let md = fs::read_to_string(d.join("k.md")).unwrap();
    assert!(md.contains("- haiku (n 1): a1") && md.contains("- opus (n 2): b1, b2"), "{md}");
}

#[test]
fn a_bugfix_run_has_its_root_under_its_own_id() {
    let run = scratch("bugfix").join("b");
    let mut c = CaseW::new(run.join("store/BUG-1"), "sdc.bugfix", &[("test.reproduce", "write")], &[]);
    c.take("test.reproduce", true, &[], 5, "0", "repro_test", None, None, None);
    c.finish(Some("fixed"));
    let (code, doc, err) = metrics(&run);
    assert_eq!(code, 0, "{err}");
    assert_eq!(doc["metrics"]["end"], "closed: fixed");
    assert_eq!(node(&doc, "bugfix/test.reproduce")["real_takes"], 1);
}

fn sha(text: &str) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

const IDEA: &str = "Bill customers with invoices.";

fn record() -> String {
    format!(r#"format: sdc.experiment/1
id: e1
kind: retrospective
status: complete
created: 2026-11-21
title: Haiku against Opus as decider
purpose: Whether the decider model changes how often a rejected merge sends the story back.
hypothesis: "With the opus decider the edge story: merge.approve:rejected -> change.implement occurs at least once less per run."
inputs:
- name: invoices
  kind: idea
  text: {IDEA}
  sha256: {}
tree_commit: 8db4b93
target:
  kind: edge
  name: "story: merge.approve:rejected -> change.implement"
variable: decider.model
fixed:
  models.implement.model: claude-sonnet-5-5
variants:
- id: haiku
  settings:
    decider.model: haiku
  runs: [a1, a2, a3]
- id: opus
  settings:
    decider.model: opus
  runs: [b1, b2, b3]
baseline: haiku
treatment: opus
runs_per_variant: 3
pass_rule: clean-delivery
primary:
  metric: "edge.story: merge.approve:rejected -> change.implement.count"
  direction: decrease
  min_effect: 1
guards:
- metric: run.delivered
  worse: decrease
  max_worsening: 0
- metric: run.cost_usd
  worse: increase
  max_worsening_ratio: 0.5
decision_rule:
  rule: complete-separation
  text: held when every opus run is below every haiku run and the medians differ by at least 1.
facts:
- the runs are written by the test
adoption:
  decision: pending
  commit: null
"#, sha(IDEA))
}

#[test]
fn a_record_is_checked_reported_and_indexed_from_its_run_summaries() {
    let d = scratch("record");
    for (name, r, m) in [("a1", 2, "haiku"), ("a2", 1, "haiku"), ("a3", 2, "haiku"), ("b1", 0, "opus"), ("b2", 0, "opus"), ("b3", 0, "opus")] {
        story_run(&d.join("runs").join(name), r, m);
    }
    fs::create_dir_all(d.join("experiments")).unwrap();
    fs::write(d.join("experiments/e1.yaml"), record()).unwrap();
    let (code, out) = experiment(&["check", "e1", "--runs-dir", "runs"], &d);
    assert_eq!(code, 0, "{out}");
    // Before the report: no report.md, no INDEX.md, no verdict.
    let (code, out) = experiment(&["check", "--all"], &d);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("report.md: missing") && out.contains("INDEX.md: missing") && out.contains("`result.verdict` is absent"), "{out}");
    let (code, out) = experiment(&["report", "e1", "--runs-dir", "runs"], &d);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("e1: held"), "{out}");
    let y: serde_yaml::Value = serde_yaml::from_str(&fs::read_to_string(d.join("experiments/e1.yaml")).unwrap()).unwrap();
    let res = &y["result"];
    assert_eq!(res["verdict"].as_str(), Some("held"));
    assert_eq!(res["separation"].as_str(), Some("expected"));
    assert_eq!(res["p_one_sided"].as_f64(), Some(0.05));
    assert_eq!(res["recommendation"].as_str(), Some("run prospectively"), "a retrospective verdict decides nothing");
    assert_eq!(res["variants"][0]["primary"]["median"].as_f64(), Some(2.0));
    assert_eq!(res["variants"][1]["clean_passes"].as_u64(), Some(3));
    // Summaries: one per ended run, no cases, no absolute path.
    let s = fs::read_to_string(d.join("experiments/e1/runs/a1.json")).unwrap();
    let sd: Value = serde_json::from_str(&s).unwrap();
    assert!(sd.get("cases").is_none() && sd["format"] == "sdc.run-metrics/1");
    assert!(!s.contains(&format!("/{}/", "home")) && !s.contains(d.to_str().unwrap()), "{s}");
    let md = fs::read_to_string(d.join("experiments/e1/report.md")).unwrap();
    assert!(md.contains("| clean passes | 3 | 3 |") && md.contains("held: opus lowers"), "{md}");
    let index = fs::read_to_string(d.join("experiments/INDEX.md")).unwrap();
    assert!(index.contains("| [e1](e1/report.md) | 2026-11-21 | retrospective |") && index.contains("| haiku 3, opus 3 | complete | held |"), "{index}");
    // The report needs no run directory: with runs/ gone, check --all still holds.
    fs::rename(d.join("runs"), d.join("runs.away")).unwrap();
    let (code, out) = experiment(&["check", "--all"], &d);
    assert_eq!(code, 0, "{out}");
    // A hand edit of the report, or a stale index, fails check --all.
    fs::write(d.join("experiments/e1/report.md"), md.replace("held", "adopted")).unwrap();
    fs::write(d.join("experiments/INDEX.md"), "# Experiments\n").unwrap();
    let (code, out) = experiment(&["check", "--all"], &d);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("report.md: stale") && out.contains("INDEX.md: stale"), "{out}");
    fs::rename(d.join("runs.away"), d.join("runs")).unwrap();
    // A run made with other settings than its variant says is refused.
    fs::write(d.join("experiments/e1.yaml"), record().replace("runs: [b1, b2, b3]", "runs: [b1, b2, a3]")).unwrap();
    let (code, out) = experiment(&["check", "e1", "--runs-dir", "runs"], &d);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("run a3 has decider.model = haiku"), "{out}");
    // Too few runs: inconclusive, whatever the figures.
    fs::write(d.join("experiments/e1.yaml"), record().replace("runs: [b1, b2, b3]", "runs: [b1, b2]")).unwrap();
    let (code, out) = experiment(&["report", "e1", "--runs-dir", "runs", "--print"], &d);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("e1: inconclusive (too few ended runs"), "{out}");
    // A record with a home path, or an input whose hash is not its text's, fails its check.
    fs::write(d.join("experiments/e1.yaml"), record().replace(&sha(IDEA), &sha("x")) + &format!("links: [/{}/someone/notes]\n", "home")).unwrap();
    let (code, out) = experiment(&["check", "e1"], &d);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("absolute home path") && out.contains("is not the sha256 of `text`"), "{out}");
}

#[test]
fn a_launch_is_refused_without_a_pinned_commit_and_printed_from_the_record() {
    let d = scratch("launch");
    fs::create_dir_all(d.join("experiments")).unwrap();
    let rec = format!(r#"format: sdc.experiment/1
id: p1
kind: prospective
status: planned
created: 2026-11-21
title: Effort low against high
purpose: Whether effort changes the implement loop.
inputs:
- {{name: invoices, kind: idea, text: {IDEA}, sha256: {}}}
tree_commit: null
fixed:
  models.writer.model: claude-sonnet-5-5
  models.implement.model: claude-sonnet-5-5
  models.review.model: claude-sonnet-5-5
  models.other.model: claude-sonnet-5-5
  decider.kind: haiku
  decider.model: claude-opus-5-5
  decider.escalate: final
  decider.max_decider_calls: 40
  adapter: metaharness
  max_llm_calls: 150
  autonomy: A5
variants:
- {{id: low, settings: {{efforts.implement.effort: low}}, runs: []}}
- {{id: high, settings: {{efforts.implement.effort: high}}, runs: []}}
runs_per_variant: 1
pass_rule: clean-delivery
decision_rule: {{rule: descriptive, text: t}}
"#, sha(IDEA));
    fs::write(d.join("experiments/p1.yaml"), &rec).unwrap();
    let (code, out) = experiment(&["run", "p1"], &d);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("pins no tree_commit"), "{out}");
    fs::write(d.join("experiments/p1.yaml"), rec.replace("tree_commit: null", "tree_commit: 8db4b9338f1d8d32c85b9cb2e014dac56980fac0")).unwrap();
    let (code, out) = experiment(&["run", "p1", "--print", "--builds-dir", "b"], &d);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("p1.low.1 (sdc-exp-p1-low-1):") && out.contains("p1.high.1 (sdc-exp-p1-high-1):"), "{out}");
    assert!(out.contains("  SDC_LLM_EFFORT_IMPLEMENT=high") && out.contains("  SDC_LLM_MODEL_DECIDER=claude-opus-5-5"), "{out}");
    assert!(out.contains("8db4b9338f1d8d32c85b9cb2e014dac56980fac0/scripts/run-trial.sh --autopilot p1.high.1 Bill customers with invoices."), "{out}");
    // A setting the launcher cannot pass is refused at check time.
    fs::write(d.join("experiments/p1.yaml"), rec.replace("  autonomy: A5\n", "  autonomy: A5\n  bindings: x\n")).unwrap();
    let (code, out) = experiment(&["check", "p1"], &d);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("bindings is no launch setting"), "{out}");
}

/// An oracle file of `sdc-conform observe`; `counts` None for a run never measured (k, f, u null).
fn oracle(run: &Path, result: &str, counts: Option<(u64, u64, u64, u64, u64)>, n: Option<u64>, cause: Option<&str>) {
    let (k, f, u, agreement, tables) = match counts {
        Some((k, f, u, a, t)) => (json!(k), json!(f), json!(u), json!(a), json!(t)),
        None => (Value::Null, Value::Null, Value::Null, Value::Null, json!(0)),
    };
    write_json(&run.join("oracle.json"), &json!({"format": "sdc.oracle/1", "run": "run-a",
        "checks": [{"id": "conformance", "result": result, "k": k, "f": f, "d": if counts.is_some() { json!(0) } else { Value::Null }, "u": u, "n": n, "agreement": agreement, "tables_counted": tables, "cause": cause}],
        "conformance": {"result": result, "k": k, "f": f, "d": if counts.is_some() { json!(0) } else { Value::Null }, "u": u, "n": n, "agreement": agreement, "tables_counted": tables, "cause": cause,
            "measured": counts.is_some()}}));
}

#[test]
fn the_conformance_figure_of_an_observed_run_is_a_run_metric() {
    let run = scratch("conformance").join("run-a");
    story_run(&run, 0, "decider:haiku");
    let (code, doc, err) = metrics(&run);
    assert_eq!(code, 0, "{err}");
    assert_eq!(doc["metrics"]["conformance"], "not observed");
    assert_eq!(doc["metrics"]["conformance_k"], Value::Null);
    oracle(&run, "contradicted", Some((13, 3, 1, 15, 3)), Some(17), None);
    let (_, doc, _) = metrics(&run);
    let m = &doc["metrics"];
    assert_eq!(m["conformance"], "13 passed, 3 failed, 0 disputed, 1 not measurable of 17; 3 tables agree on 15 of 17");
    assert_eq!([&m["conformance_k"], &m["conformance_f"], &m["conformance_u"], &m["conformance_n"]], [&json!(13), &json!(3), &json!(1), &json!(17)]);
    assert_eq!([&m["conformance_agreement"], &m["conformance_tables"]], [&json!(15), &json!(3)]);
    assert_eq!(m["conformance_result"], "contradicted");
    assert_eq!(m["conformance_measured"], true);
    let o = Command::new(env!("CARGO_BIN_EXE_sdc-run-metrics")).arg(&run).output().unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(text.contains("\n  conformance: 13 passed, 3 failed, 0 disputed, 1 not measurable of 17; 3 tables agree on 15 of 17 (contradicted)\n"), "{text}");
    oracle(&run, "unknown", Some((6, 0, 1, 7, 1)), Some(7), Some("not_measurable"));
    let (_, doc, _) = metrics(&run);
    assert_eq!(doc["metrics"]["conformance"], "6 passed, 0 failed, 0 disputed, 1 not measurable of 7; 1 table, unknown: not_measurable");
    assert_eq!(doc["metrics"]["conformance_measured"], true);
    oracle(&run, "unknown", None, Some(32), Some("table_missing"));
    let (_, doc, _) = metrics(&run);
    assert_eq!(doc["metrics"]["conformance"], "not measured (table_missing), 32 scenarios");
    assert_eq!(doc["metrics"]["conformance_k"], Value::Null);
    assert_eq!(doc["metrics"]["conformance_result"], "unknown");
    assert_eq!(doc["metrics"]["conformance_measured"], false);
    oracle(&run, "unknown", None, None, Some("ess_missing"));
    let (_, doc, _) = metrics(&run);
    assert_eq!(doc["metrics"]["conformance"], "not measured (ess_missing)");
    write_json(&run.join("oracle.json"), &json!({"format": "sdc-eval.oracle/1", "checks": [{"id": "x", "result": "held"}]}));
    let (_, doc, _) = metrics(&run);
    assert_eq!(doc["metrics"]["conformance"], "not observed", "an oracle file without the conformance check");
}
