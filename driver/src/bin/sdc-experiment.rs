//! Groups of runs and experiment records (docs/workflow-metrics.md).
//!
//! ```text
//! sdc-experiment compare --group <label>=<run dir>[,<run dir>...] --group ... [--metrics <prefix>,...]
//!                        [--include-open] --out <report.md> [--json <report.json>]
//! sdc-experiment compare --by <settings key> --runs <run dir>,<run dir>,... [same options]
//! sdc-experiment check <record.yaml | id> [--runs-dir <dir>]
//! sdc-experiment check --all [--experiments <dir>]
//! sdc-experiment report <record.yaml | id> [--runs-dir <dir>] [--print]
//! sdc-experiment index [--experiments <dir>]
//! sdc-experiment progress [--experiments <dir>]
//! sdc-experiment run <record.yaml | id> [--runs-dir <dir>] [--builds-dir <dir>] [--max-parallel <n>]
//!                    [--stagger <seconds>] [--login-file <path>] [--print]
//! sdc-experiment status <record.yaml | id> [--runs-dir <dir>]
//! sdc-experiment stop <record.yaml | id> --reason <text> [--run <name>] [--runs-dir <dir>]
//! ```
//!
//! `compare` gives, per metric of `sdc.run-metrics/1` (run, node, edge, decision class), each
//! group's median, min, max and n, the difference of medians against the first group, and
//! whether the two ranges overlap (then no effect is claimed).
//!
//! An experiment record (`experiments/<id>.yaml`, `sdc.experiment/1`) is the one way a real run
//! is started and accounted for. `check` validates a record and, with `--runs-dir`, that every
//! listed run was made with its variant's settings; `check --all` validates every record and
//! the operator's targets (`experiments/targets.yaml`, `sdc.targets/1`) and fails when a report,
//! a verdict or `experiments/INDEX.md` is stale. `report` writes, per ended
//! run, a sanitized summary to `experiments/<id>/runs/<run>.json` (no prompts, no transcripts, no
//! absolute paths), computes the result from those summaries, puts it into the record, and writes
//! `experiments/<id>/report.md` and `experiments/INDEX.md`. `run` launches a prospective record:
//! it builds the recorded commit once in a build directory keyed by the commit, starts variants ×
//! n runs as transient systemd user units with the environment taken from the record only, names
//! them `<id>.<variant>.<n>` and writes `experiment.json` into each run directory. `status` lists
//! the runs of a record; `stop` writes the stop record of each open run and stops its unit, which
//! sends the signal to the run's process. A variant with `runner: loom` starts `runbook run` of the
//! build (docs/workflow-metrics.md § 7.1): the base environment, no `SDC_` variable, the settings
//! in a file. A variant with `runner: sdc`, or with no `runner`, belongs to the old driver, which
//! is gone: `check` and `report` still read such a record, `run` refuses it.
//! `progress` writes the progress ledger from `experiments/releases.yaml` and the records
//! (`sdc-experiment/progress.rs`); `report` rewrites it too, and `check --all` fails when it is
//! stale. An id is read as `experiments/<id>.yaml`; `--runs-dir` defaults to `runs`. Exit 0, 1 a record
//! that fails its check (or a refused launch), 2 a run directory or a record that cannot be read,
//! 64 usage.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde_json::{json, Value as J};
use serde_yaml::{Mapping, Value as Y};
use sha2::{Digest, Sha256};

#[path = "../run_metrics.rs"]
#[allow(dead_code)]
mod run_metrics;
#[path = "../run_timeline.rs"]
#[allow(dead_code)]
mod run_timeline;
#[path = "../run_kind.rs"]
#[allow(dead_code)]
mod run_kind;

use run_metrics::{choose, flat_metrics, metric_of, summary};

#[path = "sdc-experiment/chart.rs"]
mod chart;
#[path = "sdc-experiment/progress.rs"]
mod progress;

const USAGE: &str = "usage: sdc-experiment compare (--group <label>=<run dir>[,...])... | --by <key> --runs <dir>,... [--metrics <prefix>,...] [--include-open] --out <md> [--json <file>]
       sdc-experiment check <record.yaml | id> [--runs-dir <dir>] | --all [--experiments <dir>]
       sdc-experiment report <record.yaml | id> [--runs-dir <dir>] [--print]
       sdc-experiment index [--experiments <dir>]
       sdc-experiment progress [--experiments <dir>]
       sdc-experiment run <record.yaml | id> [--runs-dir <dir>] [--builds-dir <dir>] [--max-parallel <n>] [--stagger <s>] [--login-file <path>] [--print]
       sdc-experiment status <record.yaml | id> [--runs-dir <dir>]
       sdc-experiment stop <record.yaml | id> --reason <text> [--run <name>] [--runs-dir <dir>]";

struct Usage(String);

/// One loaded run: its name, its metrics document and the flat figures.
struct Run {
    name: String,
    doc: J,
    flat: BTreeMap<String, f64>,
}

fn load_run(dir: &Path) -> Result<Run, String> {
    // A free-running agent's run (sdc-free-agent) has its own metrics in the same shape.
    if let Some(doc) = run_timeline::free_metrics(dir).map_err(|run_metrics::Fail(m)| m)? {
        return Ok(run_of(doc));
    }
    let doc = run_metrics::load(dir).map_err(|run_metrics::Fail(m)| m)?;
    Ok(run_of(doc))
}

fn run_of(doc: J) -> Run {
    let flat = flat_metrics(&doc);
    Run { name: doc["run"].as_str().unwrap_or("run").to_string(), doc, flat }
}

fn finished(r: &Run) -> bool {
    r.doc["metrics"]["finished"] == true
}

fn fmt(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        let s = format!("{x:.4}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn signed(x: f64) -> String {
    if x > 0.0 { format!("+{}", fmt(x)) } else { fmt(x) }
}

fn sha256_hex(text: &str) -> String {
    Sha256::digest(text.as_bytes()).iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

// ---- compare -----------------------------------------------------------------------------------

#[derive(Clone)]
struct Stat {
    n: usize,
    median: f64,
    min: f64,
    max: f64,
}

fn stat(values: &[f64]) -> Option<Stat> {
    if values.is_empty() {
        return None;
    }
    let (median, min, max) = summary(values);
    Some(Stat { n: values.len(), median, min, max })
}

/// `overlap`, or `separate` with the exact two-sided chance of a complete separation of two
/// groups of that size when the group labels do not matter: 2 / C(n1 + n2, n1).
fn separation(a: &Stat, b: &Stat) -> (bool, Option<f64>) {
    let separate = a.max < b.min || b.max < a.min;
    (separate, separate.then(|| (2.0 / choose(a.n + b.n, a.n)).min(1.0)))
}

fn compare(args: &[String]) -> Result<ExitCode, Usage> {
    let mut groups: Vec<(String, Vec<PathBuf>)> = vec![];
    let (mut by, mut runs, mut out, mut json_out, mut prefixes, mut include_open) =
        (None::<String>, vec![], None::<PathBuf>, None::<PathBuf>, vec![], false);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().cloned().ok_or_else(|| Usage(format!("{a} needs a value")));
        match a.as_str() {
            "--group" => {
                let v = value()?;
                let (l, dirs) = v.split_once('=').filter(|(l, d)| !l.is_empty() && !d.is_empty())
                    .ok_or_else(|| Usage(format!("--group {v}: expected <label>=<run dir>[,...]")))?;
                if groups.iter().any(|(x, _)| x == l) {
                    return Err(Usage(format!("--group: label {l} given twice")));
                }
                groups.push((l.to_string(), dirs.split(',').map(PathBuf::from).collect()));
            }
            "--by" => by = Some(value()?),
            "--runs" => runs = value()?.split(',').map(PathBuf::from).collect(),
            "--out" => out = Some(PathBuf::from(value()?)),
            "--json" => json_out = Some(PathBuf::from(value()?)),
            "--metrics" => prefixes = value()?.split(',').map(String::from).collect(),
            "--include-open" => include_open = true,
            x => return Err(Usage(format!("unknown argument {x}"))),
        }
    }
    let out = out.ok_or_else(|| Usage("--out is required".into()))?;
    // Load.
    let mut loaded: Vec<(String, Vec<Run>)> = vec![];
    let mut excluded: Vec<J> = vec![];
    let mut take = |dir: &Path| -> Result<Option<Run>, String> {
        let r = load_run(dir)?;
        if !include_open && !finished(&r) {
            excluded.push(json!({"run": r.name, "why": r.doc["metrics"]["end"]}));
            return Ok(None);
        }
        Ok(Some(r))
    };
    if let Some(key) = &by {
        if !groups.is_empty() || runs.is_empty() {
            return Err(Usage("--by needs --runs and no --group".into()));
        }
        let mut map: BTreeMap<String, Vec<Run>> = BTreeMap::new();
        for d in &runs {
            match take(d) {
                Ok(Some(r)) => {
                    let v = r.doc["settings"][key.as_str()].as_str().unwrap_or("not recorded").to_string();
                    map.entry(v).or_default().push(r);
                }
                Ok(None) => {}
                Err(m) => {
                    eprintln!("sdc-experiment: {m}");
                    return Ok(ExitCode::from(2));
                }
            }
        }
        loaded = map.into_iter().collect();
    } else {
        for (l, dirs) in &groups {
            let mut rs = vec![];
            for d in dirs {
                match take(d) {
                    Ok(Some(r)) => rs.push(r),
                    Ok(None) => {}
                    Err(m) => {
                        eprintln!("sdc-experiment: {m}");
                        return Ok(ExitCode::from(2));
                    }
                }
            }
            loaded.push((l.clone(), rs));
        }
    }
    if loaded.len() < 2 {
        return Err(Usage("a comparison needs at least two groups".into()));
    }
    let (md, doc) = compare_report(&loaded, &prefixes, &excluded, by.as_deref());
    if let Err(e) = fs::write(&out, md) {
        eprintln!("sdc-experiment: cannot write {}: {e}", out.display());
        return Ok(ExitCode::from(1));
    }
    if let Some(j) = json_out {
        if let Err(e) = fs::write(&j, serde_json::to_string_pretty(&doc).unwrap_or_default() + "\n") {
            eprintln!("sdc-experiment: cannot write {}: {e}", j.display());
            return Ok(ExitCode::from(1));
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn compare_report(groups: &[(String, Vec<Run>)], prefixes: &[String], excluded: &[J], by: Option<&str>) -> (String, J) {
    let mut names: Vec<String> = groups.iter().flat_map(|(_, rs)| rs.iter().flat_map(|r| r.flat.keys().cloned())).collect();
    names.sort();
    names.dedup();
    names.retain(|n| prefixes.is_empty() || prefixes.iter().any(|p| n.starts_with(p.as_str())));
    let mut md = String::new();
    let _ = writeln!(md, "# Run groups\n");
    if let Some(k) = by {
        let _ = writeln!(md, "Grouped by the setting `{k}` (last line of each run's `run-settings.jsonl`).\n");
    }
    for (l, rs) in groups {
        let _ = writeln!(md, "- {l} (n {}): {}", rs.len(), rs.iter().map(|r| r.name.as_str()).collect::<Vec<_>>().join(", "));
    }
    for e in excluded {
        let _ = writeln!(md, "- excluded, not finished: {} ({})", e["run"].as_str().unwrap_or(""), e["why"].as_str().unwrap_or(""));
    }
    let _ = writeln!(md, "\nDefinitions: `docs/workflow-metrics.md`. Per group: median [min, max] (n). `separate` only when the two ranges do not overlap; p is the exact two-sided chance of so complete a separation with exchangeable runs, 2 / C(n1 + n2, n1). `overlap`: no effect is claimed.\n");
    let first = &groups[0].0;
    let _ = write!(md, "| metric |");
    for (l, _) in groups {
        let _ = write!(md, " {l} |");
    }
    for (l, _) in &groups[1..] {
        let _ = write!(md, " {l} - {first} | ranges |");
    }
    let _ = writeln!(md, "\n|{}", "---|".repeat(1 + groups.len() + 2 * (groups.len() - 1)));
    let mut rows = vec![];
    for name in &names {
        let stats: Vec<Option<Stat>> = groups.iter().map(|(_, rs)| {
            let v: Vec<f64> = rs.iter().filter_map(|r| metric_of(&r.doc, &r.flat, name)).collect();
            stat(&v)
        }).collect();
        let _ = write!(md, "| `{name}` |");
        for s in &stats {
            let _ = write!(md, " {} |", s.as_ref().map_or("-".into(), |s| format!("{} [{}, {}] ({})", fmt(s.median), fmt(s.min), fmt(s.max), s.n)));
        }
        let mut diffs = vec![];
        for (k, s) in stats.iter().enumerate().skip(1) {
            match (&stats[0], s) {
                (Some(a), Some(b)) => {
                    let (sep, p) = separation(a, b);
                    let mark = if sep { format!("separate (p {})", fmt((p.unwrap_or(1.0) * 1000.0).round() / 1000.0)) } else { "overlap".into() };
                    let _ = write!(md, " {} | {mark} |", signed(b.median - a.median));
                    diffs.push(json!({"group": groups[k].0, "against": first, "median_difference": b.median - a.median,
                        "separate": sep, "p_two_sided": p}));
                }
                _ => {
                    let _ = write!(md, " n/a | n/a |");
                    diffs.push(json!({"group": groups[k].0, "against": first, "median_difference": null, "separate": null}));
                }
            }
        }
        let _ = writeln!(md);
        rows.push(json!({"metric": name,
            "groups": groups.iter().zip(&stats).map(|((l, _), s)| json!({"group": l,
                "n": s.as_ref().map_or(0, |s| s.n), "median": s.as_ref().map(|s| s.median),
                "min": s.as_ref().map(|s| s.min), "max": s.as_ref().map(|s| s.max)})).collect::<Vec<_>>(),
            "differences": diffs}));
    }
    let doc = json!({"format": "sdc.run-groups/1", "by": by,
        "groups": groups.iter().map(|(l, rs)| json!({"group": l, "runs": rs.iter().map(|r| r.name.clone()).collect::<Vec<_>>()})).collect::<Vec<_>>(),
        "excluded": excluded, "metrics": rows});
    (md, doc)
}
// ---- experiment records ------------------------------------------------------------------------

fn ystr<'a>(v: &'a Y, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Y::as_str)
}

fn yscalar(v: &Y) -> String {
    match v {
        Y::String(s) => s.clone(),
        Y::Number(n) => n.to_string(),
        Y::Bool(b) => b.to_string(),
        Y::Null => "null".into(),
        other => serde_yaml::to_string(other).unwrap_or_default().trim().to_string(),
    }
}

fn yseq(v: &Y, k: &str) -> Vec<Y> {
    v.get(k).and_then(Y::as_sequence).cloned().unwrap_or_default()
}

fn record_path(arg: &str) -> PathBuf {
    let p = PathBuf::from(arg);
    if arg.ends_with(".yaml") || arg.ends_with(".yml") || p.exists() {
        p
    } else {
        Path::new("experiments").join(format!("{arg}.yaml"))
    }
}

fn experiments_dir_of(path: &Path) -> PathBuf {
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

fn read_text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

fn read_record(path: &Path) -> Result<(Y, String), String> {
    let t = read_text(path)?;
    let y = serde_yaml::from_str(&t).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Ok((y, t))
}

fn write_record(path: &Path, r: &Y) -> Result<(), String> {
    let text = serde_yaml::to_string(r).unwrap_or_default();
    fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// Sets `key` in the record's top-level mapping, before `before` when the key is new.
fn set_key(r: &mut Y, key: &str, value: Y, before: &str) {
    let Some(m) = r.as_mapping_mut() else { return };
    let k = Y::String(key.into());
    if m.contains_key(&k) {
        m.insert(k, value);
        return;
    }
    let mut out = Mapping::new();
    let mut placed = false;
    for (x, v) in m.iter() {
        if !placed && x == &Y::String(before.into()) {
            out.insert(k.clone(), value.clone());
            placed = true;
        }
        out.insert(x.clone(), v.clone());
    }
    if !placed {
        out.insert(k, value);
    }
    *m = out;
}

const METRIC_PREFIXES: [&str; 4] = ["run.", "node.", "edge.", "decision."];
const STATUSES: [&str; 4] = ["planned", "running", "complete", "abandoned"];
/// The one pass rule: the run closed, delivered, and needed no reopen, forced step or unstick.
const PASS_RULE: &str = "clean-delivery";
/// The settings a prospective record may set and the environment variable each one becomes.
const LAUNCH: [(&str, &str); 33] = [
    ("implement_mode", "SDC_IMPLEMENT_MODE"),
    ("review_mode", "SDC_REVIEW_MODE"),
    ("implement_coverage", "SDC_IMPLEMENT_COVERAGE"),
    ("models.writer.model", "SDC_LLM_MODEL_WRITER"),
    ("models.implement.model", "SDC_LLM_MODEL_IMPLEMENT"),
    ("models.review.model", "SDC_LLM_MODEL_REVIEW"),
    ("models.other.model", "SDC_LLM_MODEL_OTHER"),
    ("decider.model", "SDC_LLM_MODEL_DECIDER"),
    ("efforts.writer.effort", "SDC_LLM_EFFORT_WRITER"),
    ("efforts.implement.effort", "SDC_LLM_EFFORT_IMPLEMENT"),
    ("efforts.review.effort", "SDC_LLM_EFFORT_REVIEW"),
    ("efforts.other.effort", "SDC_LLM_EFFORT_OTHER"),
    ("efforts.decider.effort", "SDC_LLM_EFFORT_DECIDER"),
    ("efforts.requester.effort", "SDC_LLM_EFFORT_REQUESTER"),
    ("decider.kind", "SDC_DECIDER"),
    ("decider.escalate", "SDC_DECIDER_ESCALATE"),
    ("decider.max_decider_calls", "SDC_MAX_DECIDER_CALLS"),
    ("decider.adapter", "SDC_DECIDER_ADAPTER"),
    ("review.adapter", "SDC_REVIEW_ADAPTER"),
    ("adapter", "SDC_LLM_ADAPTER"),
    ("max_llm_calls", "SDC_MAX_LLM_CALLS"),
    ("max_wall_seconds", "SDC_MAX_WALL_SECONDS"),
    ("autonomy", "SDC_AUTONOMY"),
    ("conformance", "SDC_CONFORMANCE"),
    ("rule_approvals", "SDC_RULE_APPROVALS"),
    ("flags.dry", ""),
    ("flags.recorded", ""),
    ("free.model", ""),
    ("free.effort", ""),
    ("free.adapter", ""),
    ("free.max_usd", ""),
    ("free.max_wall_s", ""),
    ("free.max_turns", ""),
];
/// What a launched variant with `runner: free-agent` sets (sdc-free-agent run). Its money cap `free.max_usd`
/// is optional: none named means no money cap.
const REQUIRED_FREE: [&str; 5] = ["free.model", "free.effort", "free.adapter", "free.max_wall_s", "free.max_turns"];
/// What a launched variant with `runner: loom` sets (`runbook run`): the confinement (no
/// default), the models of the four node classes, the
/// kind of the decider, the autonomy level and the wall-time ceiling. The money caps (`loom.cap_micro_usd`,
/// `max_llm_calls`) are optional (`OPTIONAL_LOOM`): none named means no limit. A `model` decider also sets
/// `decider.model`, a `rule` decider `decider.verdict`. The way to Loom is one of two (see
/// `LOOM_CHILD`).
const REQUIRED_LOOM: [&str; 8] = ["loom.confinement", "models.writer.model",
    "models.implement.model", "models.review.model", "models.other.model", "decider.kind", "autonomy", "max_wall_seconds"];
/// The way to Loom is the binary as a child process (`loom.bin` with its digest `loom.sha256`),
/// or, with `loom.in_process: 'true'`, Loom as a library of the build (`runbook run
/// --in-process`): then neither of these is set.
const LOOM_CHILD: [&str; 2] = ["loom.bin", "loom.sha256"];
/// `loom` settings that are optional: the money caps (a record names one or leaves it out), the path of the login file the model adapter reads, the way
/// to Loom that is the library, and the members of `runbook.loom-run-settings/1` that only some
/// deciders and adapters take.
const OPTIONAL_LOOM: [&str; 18] = ["loom.cap_micro_usd", "max_llm_calls", "conformance", "loom.login_file", "loom.in_process", "decider.model", "decider.verdict", "decider.escalate", "decider.max_decider_calls",
    "decider.adapter", "adapter", "review.adapter", "flags.dry", "flags.recorded", "flags.on_divergence", "flags.live_adapter", "flags.live_cap", "flags.source_tree"];
/// The members of a `model` decider that a `rule` decider does not have.
const MODEL_ONLY_LOOM: [&str; 4] = ["decider.model", "decider.escalate", "decider.max_decider_calls", "decider.adapter"];
/// What the shell programs the bindings run (`sdc-tool.sh`, the adapters) need on
/// `PATH`; the decider of `runbook run` starts none of them.
const LOOM_TOOLS: [&str; 2] = ["jq", "yq"];
const RUNNERS: [&str; 3] = ["sdc", "free-agent", "loom"];
const REQUIRED_LAUNCH: [&str; 10] = ["models.writer.model", "models.implement.model", "models.review.model", "models.other.model",
    "decider.kind", "decider.model", "decider.escalate", "decider.max_decider_calls", "adapter", "autonomy"];
/// Launch settings that `run-settings.jsonl` does not record, so no run can be checked against them.
const NOT_RECORDED: [&str; 6] = ["decider.adapter", "review.adapter", "flags.dry", "flags.recorded", "loom.bin", "loom.login_file"];

/// Why a setting of the old driver is refused for a `loom` variant, or `None` when it is no
/// setting of the old driver. Each one is a decision (docs/workflow-metrics.md, "Runner loom").
fn loom_refusal(key: &str) -> Option<&'static str> {
    Some(match key {
        "implement_mode" | "review_mode" | "implement_coverage" | "rule_approvals" =>
            "it is a switch of the old driver and Loom's bindings carry no such switch",
        k if k.starts_with("efforts.") => "the record format does not carry an effort to the runner yet (the runner takes `efforts` in its settings), so the class defaults of tools/node-classes.json apply",
        k if k.starts_with("free.") => "it is a setting of runner free-agent",
        _ => return None,
    })
}

/// The replay settings of a `loom` variant and the option of `runbook run` each one is: `flags.recorded`
/// (a run directory, absolute or a name under the runs directory, or a recording) re-drives it; the
/// others only go with it.
const REPLAY_FLAGS: [(&str, &str); 5] = [
    ("flags.recorded", "--recorded"),
    ("flags.on_divergence", "--on-divergence"),
    ("flags.live_adapter", "--live-adapter"),
    ("flags.live_cap", "--live-cap"),
    ("flags.source_tree", "--source-tree"),
];

/// The refusal of an input kind a loom variant cannot start from. A `seed` input is a bug-fix case
/// that `runbook run --bugfix <seed directory>` opens (root `BUG-1` on `sdc-bugfix`); an `idea` input
/// is the case `IDEA-1`.
fn loom_input_refusal(input_kind: Option<&str>) -> Option<&'static str> {
    (!matches!(input_kind, Some("idea" | "seed"))).then_some("a loom variant starts from an idea input")
}

/// The runbook a loom launch runs for the input: the bug-fix runbook for a `seed` input, else `sdc`.
fn loom_runbook(input: &Y) -> &'static str {
    if ystr(input, "kind") == Some("seed") { "sdc-bugfix" } else { "sdc" }
}

/// The problems of a `loom` variant's settings.
fn check_loom_settings(v: &Variant, p: &mut Vec<String>) {
    for k in REQUIRED_LOOM {
        if !v.settings.contains_key(k) {
            p.push(format!("variant {:?}: the settings do not set {k} (a loom launch needs the way to Loom, the confinement, every class's model, the decider's kind, the autonomy level and the wall-time ceiling; a money cap is optional)", v.id));
        }
    }
    match v.settings.get("loom.in_process").map(String::as_str) {
        None => {
            for k in LOOM_CHILD {
                if !v.settings.contains_key(k) {
                    p.push(format!("variant {:?}: the settings do not set {k} (the Loom binary and its digest, or loom.in_process: 'true' for Loom as a library)", v.id));
                }
            }
        }
        Some("true") => {
            for k in LOOM_CHILD.iter().filter(|k| v.settings.contains_key(**k)) {
                p.push(format!("variant {:?}: {k} and loom.in_process both name the way to Loom: a variant runs the binary (loom.bin with loom.sha256) or the library (loom.in_process: 'true'), not both", v.id));
            }
        }
        Some(_) => p.push(format!("variant {:?}: loom.in_process must be 'true' (leave it out to run the Loom binary)", v.id)),
    }
    let one_of = |k: &str, allowed: &[&str], p: &mut Vec<String>, what: &str| {
        if v.settings.get(k).is_some_and(|x| !allowed.contains(&x.as_str())) {
            p.push(format!("variant {:?}: {k} must be {what}", v.id));
        }
    };
    one_of("decider.kind", &["model", "rule"], p, "model or rule (haiku is the old driver's name of model; a scripted decider is a test double of the runner and no setting of a record)");
    one_of("autonomy", &["A0", "A1", "A2", "A3", "A4", "A5"], p, "one of A0 to A5 (a decider answers only at A5)");
    one_of("decider.verdict", &["approve", "reject"], p, "approve or reject");
    one_of("decider.escalate", &["final", "stop"], p, "final or stop");
    one_of("adapter", &["metaharness", "claude-cli"], p, "metaharness or claude-cli");
    one_of("decider.adapter", &["metaharness", "claude-cli"], p, "metaharness or claude-cli");
    one_of("review.adapter", &["claude-cli"], p, "claude-cli (the only review adapter expressible under Loom)");
    one_of("flags.dry", &["true"], p, "'true' (leave it out for a run that calls models)");
    one_of("flags.on_divergence", &["stop", "nearest", "live"], p, "stop, nearest or live");
    if let Some(k) = REPLAY_FLAGS.iter().skip(1).map(|(k, _)| *k).find(|k| v.settings.contains_key(*k)).filter(|_| !v.settings.contains_key("flags.recorded")) {
        p.push(format!("variant {:?}: {k} goes with flags.recorded (`runbook run --recorded`)", v.id));
    }
    if v.settings.get("flags.recorded").is_some_and(|x| x.is_empty()) {
        p.push(format!("variant {:?}: flags.recorded must name a run directory or a recording", v.id));
    }
    if v.settings.contains_key("flags.dry") {
        for k in ["adapter", "decider.adapter"].iter().filter(|k| v.settings.contains_key(**k)) {
            p.push(format!("variant {:?}: {k} is not set with flags.dry: `runbook run --dry` runs every model node and a model decider on the adapter dry", v.id));
        }
    }
    match v.settings.get("decider.kind").map(String::as_str) {
        Some("model") => {
            if !v.settings.contains_key("decider.model") {
                p.push(format!("variant {:?}: the settings do not set decider.model (a model decider needs its model)", v.id));
            }
            if v.settings.contains_key("decider.verdict") {
                p.push(format!("variant {:?}: decider.verdict is a setting of a rule decider, not of kind model", v.id));
            }
        }
        Some("rule") => {
            if !v.settings.contains_key("decider.verdict") {
                p.push(format!("variant {:?}: the settings do not set decider.verdict (a rule decider approves or rejects every decision)", v.id));
            }
            for k in MODEL_ONLY_LOOM.iter().filter(|k| v.settings.contains_key(**k)) {
                p.push(format!("variant {:?}: {k} is a setting of a model decider, not of kind rule", v.id));
            }
        }
        _ => {}
    }
    for k in v.settings.keys() {
        if REQUIRED_LOOM.contains(&k.as_str()) || OPTIONAL_LOOM.contains(&k.as_str()) || LOOM_CHILD.contains(&k.as_str()) {
            continue;
        }
        match loom_refusal(k) {
            Some(why) => p.push(format!("variant {:?}: {k} is refused for a loom variant: {why}", v.id)),
            None => p.push(format!("variant {:?}: {k} is no loom launch setting", v.id)),
        }
    }
    let whole = |k: &str, least: u64| v.settings.get(k).is_none_or(|x| x.parse::<u64>().is_ok_and(|n| n >= least));
    // A money cap a record names is a whole number; 0 is allowed and spends nothing.
    for k in ["loom.cap_micro_usd", "max_llm_calls"] {
        if !whole(k, 0) {
            p.push(format!("variant {:?}: {k} must be a whole number (0 allows nothing)", v.id));
        }
    }
    for k in ["max_wall_seconds", "decider.max_decider_calls"] {
        if !whole(k, 1) {
            p.push(format!("variant {:?}: {k} must be a whole number of at least 1", v.id));
        }
    }
    // A cap on live calls is named or not; 0 is a cap that lets no live call start.
    if !whole("flags.live_cap", 0) {
        p.push(format!("variant {:?}: flags.live_cap must be a whole number (0 allows no live call)", v.id));
    }
    if v.settings.get("conformance").is_some_and(|c| c != "off" && c != "observe") {
        p.push(format!("variant {:?}: conformance must be off or observe", v.id));
    }
    if v.settings.get("loom.confinement").is_some_and(|c| c != "local" && c != "substrate") {
        p.push(format!("variant {:?}: loom.confinement must be local or substrate (there is no default)", v.id));
    }
    if v.settings.get("loom.sha256").is_some_and(|s| !is_hex(s, 64, 64)) {
        p.push(format!("variant {:?}: loom.sha256 must be the 64 hex digits of the binary's sha256", v.id));
    }
    for k in ["loom.bin", "loom.login_file"] {
        if v.settings.get(k).is_some_and(|b| !(b.starts_with('/') || b.starts_with("~/"))) {
            p.push(format!("variant {:?}: {k} must be an absolute path or start with ~/", v.id));
        }
    }
}
/// Environment variables a launched run gets besides the record's: the login's, never an SDC_ one.
const BASE_ENV: [&str; 11] = ["HOME", "PATH", "USER", "LOGNAME", "LANG", "XDG_RUNTIME_DIR", "XDG_CONFIG_HOME", "XDG_STATE_HOME",
    "XDG_CACHE_HOME", "XDG_DATA_HOME", "DBUS_SESSION_BUS_ADDRESS"];

#[derive(Clone)]
struct RunRef {
    run: String,
    input: Option<String>,
    tree: Option<String>,
    note: Option<String>,
}

#[derive(Clone)]
struct Variant {
    id: String,
    /// The variant's own settings.
    own: BTreeMap<String, String>,
    /// `fixed` with the variant's own settings over it.
    settings: BTreeMap<String, String>,
    runs: Vec<RunRef>,
    /// `sdc` (the default) or `free-agent`: what runs the idea.
    runner: String,
    /// The record whose runs the variant lists; such a variant is not launched.
    from: Option<String>,
    /// The name of a launched run, `{n}` the ordinal; default `<id>.<variant>.<n>`.
    run_name: Option<String>,
}

fn settings_map(v: Option<&Y>) -> BTreeMap<String, String> {
    v.and_then(Y::as_mapping).into_iter().flatten().map(|(k, x)| (yscalar(k), yscalar(x))).collect()
}

fn run_refs(v: &Y) -> Vec<RunRef> {
    yseq(v, "runs").iter().filter_map(|x| match x {
        Y::String(s) => Some(RunRef { run: s.clone(), input: None, tree: None, note: None }),
        Y::Mapping(_) => ystr(x, "run").map(|s| RunRef { run: s.to_string(), input: ystr(x, "input").map(String::from),
            tree: x.get("tree").map(yscalar), note: ystr(x, "note").map(String::from) }),
        _ => None,
    }).collect()
}

fn variants(r: &Y) -> Vec<Variant> {
    let fixed = settings_map(r.get("fixed"));
    yseq(r, "variants").iter().map(|v| {
        let own = settings_map(v.get("settings"));
        let mut settings = fixed.clone();
        settings.extend(own.clone());
        Variant { id: ystr(v, "id").unwrap_or("").to_string(), own, settings, runs: run_refs(v),
            runner: ystr(v, "runner").unwrap_or("sdc").to_string(), from: ystr(v, "from").map(String::from),
            run_name: ystr(v, "run_name").map(String::from) }
    }).collect()
}

fn has_hypothesis(r: &Y) -> bool {
    ystr(r, "hypothesis").is_some_and(|h| !h.trim().is_empty())
}

/// The baseline variants: `baseline` names one, or lists several (each compared on its own).
fn baselines(r: &Y) -> Vec<String> {
    match r.get("baseline") {
        Some(Y::String(s)) => vec![s.clone()],
        Some(Y::Sequence(v)) => v.iter().filter_map(|x| x.as_str().map(String::from)).collect(),
        _ => vec![],
    }
}

/// The primary metrics: `primary` is one, or a list (each decided by the rule).
fn primaries(r: &Y) -> Vec<Y> {
    match r.get("primary") {
        Some(Y::Sequence(v)) => v.clone(),
        Some(m @ Y::Mapping(_)) => vec![m.clone()],
        _ => vec![],
    }
}

/// Whether the record compares against several baselines or on several primary metrics.
fn multi(r: &Y) -> bool {
    matches!(r.get("baseline"), Some(Y::Sequence(_))) || matches!(r.get("primary"), Some(Y::Sequence(_)))
}

fn one_line(s: &str) -> bool {
    !s.trim().is_empty() && !s.contains('\n')
}

fn is_hex(s: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn rpv_of(r: &Y) -> usize {
    r.get("runs_per_variant").and_then(Y::as_u64).unwrap_or(0) as usize
}

/// The problems of a record: every one a sentence naming the field.
fn check_record(r: &Y, path: &Path, raw: &str) -> Vec<String> {
    let mut p = vec![];
    for k in ["format", "id", "kind", "status", "created", "title", "purpose", "inputs", "tree_commit", "variants",
        "runs_per_variant", "pass_rule", "decision_rule"] {
        if r.get(k).is_none() {
            p.push(format!("`{k}` is missing"));
        }
    }
    if ystr(r, "format") != Some("sdc.experiment/1") {
        p.push("`format` must be sdc.experiment/1".into());
    }
    let id = ystr(r, "id").unwrap_or("");
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        if stem != id {
            p.push(format!("`id` {id:?} is not the file name {stem:?}"));
        }
    }
    let kind = ystr(r, "kind").unwrap_or("");
    let prospective = kind == "prospective";
    if !matches!(kind, "retrospective" | "prospective") {
        p.push("`kind` must be retrospective or prospective".into());
    }
    let status = ystr(r, "status").unwrap_or("");
    if !STATUSES.contains(&status) {
        p.push("`status` must be planned, running, complete or abandoned".into());
    }
    if r.get("created").map(yscalar).is_none_or(|c| c.len() != 10) {
        p.push("`created` must be a date, YYYY-MM-DD".into());
    }
    if !ystr(r, "title").is_some_and(one_line) {
        p.push("`title` must be one line".into());
    }
    if !ystr(r, "purpose").is_some_and(one_line) {
        p.push("`purpose` must say on one line what the runs are for".into());
    }
    if raw.contains("/home/") || raw.contains("/Users/") {
        p.push("the record holds an absolute home path".into());
    }
    // Inputs.
    let inputs = yseq(r, "inputs");
    if inputs.is_empty() {
        p.push("`inputs` needs at least one input (the idea text or the seed)".into());
    }
    let mut input_names = vec![];
    for i in &inputs {
        let name = ystr(i, "name").unwrap_or("").to_string();
        if name.is_empty() || input_names.contains(&name) {
            p.push(format!("an input needs a unique `name` ({name:?})"));
        }
        input_names.push(name.clone());
        let ik = ystr(i, "kind").unwrap_or("");
        if !matches!(ik, "idea" | "seed" | "fixture") {
            p.push(format!("input {name:?}: `kind` must be idea, seed or fixture"));
        }
        let text = ystr(i, "text").unwrap_or("");
        if text.trim().is_empty() {
            p.push(format!("input {name:?}: `text` must hold the idea text, the bug report or the fixture's name"));
        }
        let want = sha256_hex(text);
        if ystr(i, "sha256") != Some(want.as_str()) {
            p.push(format!("input {name:?}: `sha256` is not the sha256 of `text` ({want})"));
        }
        if ik == "seed" && !ystr(i, "seed").is_some_and(|s| !s.is_empty() && !s.starts_with('/')) {
            p.push(format!("input {name:?}: a seed needs `seed`, its directory relative to the tree"));
        }
    }
    if prospective && inputs.len() != 1 {
        p.push("a prospective record has exactly one input".into());
    }
    // Tree.
    match r.get("tree_commit") {
        Some(Y::String(s)) if s == "several" => {
            if prospective {
                p.push("a prospective record names one `tree_commit`".into());
            }
            if yseq(r, "trees").iter().filter(|t| t.as_str().is_some()).count() == 0 {
                p.push("`tree_commit: several` needs `trees`, one line per tree and its runs".into());
            }
        }
        Some(Y::String(s)) if prospective && !is_hex(s, 40, 40) => p.push("`tree_commit` must be the full 40-character commit".into()),
        Some(Y::String(s)) if !is_hex(s, 7, 40) => p.push("`tree_commit` must be a commit or `several`".into()),
        Some(Y::String(_)) => {}
        Some(Y::Null) if prospective && status == "planned" => {}
        _ => p.push("`tree_commit` must be a commit (null only while a prospective record is planned)".into()),
    }
    // Variants and runs. A `fixed` or `settings` block is a mapping: one that is not never reads as empty,
    // which would drop a cap written in it.
    let not_map = |b: Option<&Y>| b.is_some_and(|x| !x.is_null() && !x.is_mapping());
    if not_map(r.get("fixed")) {
        p.push("`fixed` must be a mapping of settings".into());
    }
    for (i, y) in yseq(r, "variants").iter().enumerate() {
        if not_map(y.get("settings")) {
            p.push(format!("variant {}: `settings` must be a mapping of settings", i + 1));
        }
    }
    let vs = variants(r);
    let hyp = has_hypothesis(r);
    if vs.is_empty() || (hyp && vs.len() < 2) {
        p.push("`variants` needs at least one variant, two with a hypothesis".into());
    }
    let mut ids = vec![];
    let mut run_names = BTreeSet::new();
    for (v, y) in vs.iter().zip(yseq(r, "variants")) {
        if v.id.is_empty() || ids.contains(&v.id) {
            p.push(format!("a variant needs a unique `id` ({:?})", v.id));
        }
        ids.push(v.id.clone());
        if y.get("runs").and_then(Y::as_sequence).is_none() {
            p.push(format!("variant {:?}: `runs` must be a list (empty before the runs)", v.id));
        }
        for rr in &v.runs {
            if !run_names.insert(rr.run.clone()) {
                p.push(format!("run {} is listed twice", rr.run));
            }
            match &rr.input {
                Some(i) if !input_names.contains(i) => p.push(format!("run {}: input {i:?} is not in `inputs`", rr.run)),
                None if inputs.len() > 1 => p.push(format!("run {}: with several inputs each run names its `input`", rr.run)),
                _ => {}
            }
        }
        if !RUNNERS.contains(&v.runner.as_str()) {
            p.push(format!("variant {:?}: `runner` must be sdc, free-agent or loom", v.id));
        }
        if v.from.as_deref().is_some_and(|f| f.trim().is_empty()) {
            p.push(format!("variant {:?}: `from` must name the record whose runs it lists", v.id));
        }
        if v.run_name.as_deref().is_some_and(|n| !n.contains("{n}") || n.contains('/')) {
            p.push(format!("variant {:?}: `run_name` must hold {{n}} and no /", v.id));
        }
        if prospective && v.from.is_none() && v.runner == "free-agent" {
            for k in REQUIRED_FREE {
                if !v.settings.contains_key(k) {
                    p.push(format!("variant {:?}: the settings do not set {k} (a free-agent launch needs the model, effort, adapter and the time and turn bounds; a money cap is optional)", v.id));
                }
            }
            // A money cap a record names is a finite number of at least 0; 0 spends nothing.
            if v.settings.get("free.max_usd").is_some_and(|x| !x.parse::<f64>().is_ok_and(|n| n.is_finite() && n >= 0.0)) {
                p.push(format!("variant {:?}: free.max_usd must be a finite number of at least 0 (0 allows nothing)", v.id));
            }
            // Every key is one `free_argv` reads: a cap under a misspelt name is refused, never dropped.
            for k in v.settings.keys().filter(|k| !REQUIRED_FREE.contains(&k.as_str()) && k.as_str() != "free.max_usd") {
                p.push(format!("variant {:?}: {k} is no free-agent launch setting", v.id));
            }
        } else if v.runner == "loom" {
            if prospective && v.from.is_none() {
                check_loom_settings(v, &mut p);
                for why in inputs.iter().filter_map(|i| loom_input_refusal(ystr(i, "kind"))) {
                    p.push(format!("variant {:?}: {why}", v.id));
                }
            }
        } else if prospective {
            for k in REQUIRED_LAUNCH {
                if !v.settings.contains_key(k) {
                    p.push(format!("variant {:?}: the settings do not set {k} (a launch needs every class's model, the decider, the adapter and the autonomy; a call cap is optional)", v.id));
                }
            }
            for k in v.settings.keys() {
                if !LAUNCH.iter().any(|(x, _)| x == k) {
                    p.push(format!("variant {:?}: {k} is no launch setting", v.id));
                }
            }
        }
    }
    if rpv_of(r) == 0 {
        p.push("`runs_per_variant` must be a whole number of at least 1".into());
    }
    if ystr(r, "pass_rule") != Some(PASS_RULE) {
        p.push(format!("`pass_rule` must be {PASS_RULE} (closed, delivered, no reopen, forced step or unstick)"));
    }
    let rule = r.get("decision_rule").cloned().unwrap_or(Y::Null);
    match (hyp, ystr(&rule, "rule")) {
        (true, Some("complete-separation")) | (false, Some("descriptive")) => {}
        (true, _) => p.push("`decision_rule.rule` must be complete-separation with a hypothesis".into()),
        (false, _) => p.push("`decision_rule.rule` must be descriptive without a hypothesis".into()),
    }
    if ystr(&rule, "text").is_none_or(|t| t.trim().is_empty()) {
        p.push("`decision_rule.text` must say the rule in words".into());
    }
    for k in ["facts", "links", "trees"] {
        if r.get(k).is_some_and(|x| x.as_sequence().is_none_or(|s| s.iter().any(|e| e.as_str().is_none()))) {
            p.push(format!("`{k}` must be a list of lines"));
        }
    }
    if let Some(l) = r.get("launch") {
        if l.get("max_parallel").is_some_and(|x| x.as_u64().is_none_or(|n| n == 0)) {
            p.push("`launch.max_parallel` must be at least 1".into());
        }
        if l.get("stagger_s").is_some_and(|x| x.as_u64().is_none()) {
            p.push("`launch.stagger_s` must be a whole number of seconds".into());
        }
    }
    if hyp {
        check_hypothesis(r, &vs, &mut p);
    }
    if let Some(res) = r.get("result").filter(|x| !x.is_null()) {
        if !matches!(ystr(res, "verdict"), Some("held" | "contradicted" | "inconclusive" | "descriptive")) {
            p.push("`result.verdict` must be held, contradicted, inconclusive or descriptive".into());
        }
    }
    p
}

/// The checks of a record with a hypothesis: one variable, a target, a primary metric, guards.
fn check_hypothesis(r: &Y, vs: &[Variant], p: &mut Vec<String>) {
    for k in ["target", "variable", "baseline", "treatment", "primary", "guards", "adoption"] {
        if r.get(k).is_none() {
            p.push(format!("`{k}` is missing (a record with a hypothesis needs it)"));
        }
    }
    let hyp = ystr(r, "hypothesis").unwrap_or("");
    if hyp.contains('\n') || !hyp.trim_end().ends_with('.') || hyp.trim_end().trim_end_matches('.').contains(". ") {
        p.push("`hypothesis` must be one sentence on one line, ending with a full stop".into());
    }
    let target = r.get("target").cloned().unwrap_or(Y::Null);
    let tname = ystr(&target, "name").unwrap_or("");
    if !matches!(ystr(&target, "kind"), Some("node" | "edge" | "run" | "decision")) || tname.is_empty() {
        p.push("`target` needs `kind` (node, edge, run or decision) and `name`".into());
    } else if !hyp.contains(tname) {
        p.push(format!("`hypothesis` does not name the target {tname:?}"));
    }
    let variable = ystr(r, "variable").unwrap_or("");
    if variable.is_empty() {
        p.push("`variable` must name one settings key".into());
    }
    let bases = baselines(r);
    let treat = ystr(r, "treatment").unwrap_or("");
    if variable == "runner" {
        // What runs the idea is the variable: each runner has its own settings, so the variants
        // differ in them too; `known_confounds` says what is not held equal.
        let runner_of = |id: &str| vs.iter().find(|v| v.id == id).map(|v| v.runner.clone());
        for b in &bases {
            if runner_of(b).is_some() && runner_of(b) == runner_of(treat) {
                p.push(format!("the treatment {treat:?} and the baseline {b:?} must differ in the runner"));
            }
        }
        if yseq(r, "known_confounds").is_empty() {
            p.push("a record whose variable is the runner lists in `known_confounds` what is not held equal".into());
        }
    } else {
        let mut values = vec![];
        let mut other_keys: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for v in vs {
            match v.own.get(variable) {
                Some(x) => values.push(x.clone()),
                None => p.push(format!("variant {:?}: `settings` does not set the variable {variable:?}", v.id)),
            }
            for (k, x) in &v.own {
                if k != variable {
                    other_keys.entry(k.clone()).or_default().push(x.clone());
                }
            }
        }
        let mut dedup = values.clone();
        dedup.sort();
        dedup.dedup();
        if dedup.len() != values.len() {
            p.push("the variants must differ in the variable".into());
        }
        for (k, xs) in &other_keys {
            if xs.len() != vs.len() || xs.iter().any(|x| x != &xs[0]) {
                p.push(format!("variants differ in {k:?} as well as in the variable: one variable per experiment"));
            }
        }
    }
    if bases.is_empty() || bases.iter().any(|b| !vs.iter().any(|v| &v.id == b)) {
        p.push("`baseline` must name a variant (or be a list of variants)".into());
    }
    if !vs.iter().any(|v| v.id == treat) {
        p.push("`treatment` must name a variant".into());
    }
    if bases.iter().any(|b| b == treat) {
        p.push("`baseline` and `treatment` must differ".into());
    }
    let metric_ok = |m: &str| METRIC_PREFIXES.iter().any(|x| m.starts_with(x)) && m.rsplit_once('.').is_some_and(|(_, f)| !f.is_empty());
    let prims = primaries(r);
    if prims.is_empty() {
        p.push("`primary` must be a metric with direction and min_effect (or a list of them)".into());
    }
    for primary in &prims {
        if !ystr(primary, "metric").is_some_and(metric_ok) {
            p.push("`primary.metric` must be a metric name (run., node., edge. or decision.<...>.<field>)".into());
        }
        if !matches!(ystr(primary, "direction"), Some("increase" | "decrease")) {
            p.push("`primary.direction` must be increase or decrease".into());
        }
        if primary.get("min_effect").and_then(Y::as_f64).is_none_or(|x| x < 0.0) {
            p.push("`primary.min_effect` must be a number of at least 0 (the size the hypothesis names)".into());
        }
        if let (Some(m), Some(t)) = (ystr(primary, "metric"), Some(tname).filter(|t| !t.is_empty())) {
            if !m.contains(t) && ystr(&target, "kind") != Some("run") {
                p.push(format!("`primary.metric` {m:?} is not a metric of the target {t:?}"));
            }
        }
    }
    for g in yseq(r, "guards") {
        if !ystr(&g, "metric").is_some_and(metric_ok) {
            p.push("a guard needs a `metric` name".into());
        }
        if !matches!(ystr(&g, "worse"), Some("increase" | "decrease")) {
            p.push("a guard needs `worse`: increase or decrease (the direction that is worse)".into());
        }
        if g.get("max_worsening").and_then(Y::as_f64).is_none() && g.get("max_worsening_ratio").and_then(Y::as_f64).is_none() {
            p.push("a guard needs `max_worsening` (absolute) or `max_worsening_ratio` (share of the baseline median)".into());
        }
    }
    let adoption = r.get("adoption").cloned().unwrap_or(Y::Null);
    match ystr(&adoption, "decision") {
        Some("pending" | "dropped") => {}
        Some("adopted") if ystr(&adoption, "commit").is_some_and(|c| !c.is_empty()) => {}
        Some("adopted") => p.push("`adoption.commit` must name the commit that adopted the change".into()),
        _ => p.push("`adoption.decision` must be pending, adopted or dropped".into()),
    }
}

// ---- run summaries -----------------------------------------------------------------------------

fn summaries_dir(exp: &Path, id: &str) -> PathBuf {
    exp.join(id).join("runs")
}

fn has_abs_path(s: &str) -> bool {
    ["/home/", "/Users/", "/root/", "/tmp/", "/var/", "/run/user/", "/dev/shm/"].iter().any(|p| s.contains(p))
}

fn sanitize(v: &mut J) {
    match v {
        J::String(s) if has_abs_path(s) => *s = "<absolute path removed>".into(),
        J::Array(a) => a.iter_mut().for_each(sanitize),
        J::Object(m) => m.values_mut().for_each(sanitize),
        _ => {}
    }
}

/// The committed summary of one ended run: `sdc.run-metrics/1` without `cases`, every string that
/// holds an absolute path replaced. The metrics document holds no prompt and no transcript.
fn summary_doc(doc: &J) -> J {
    let mut d = doc.clone();
    if let Some(m) = d.as_object_mut() {
        m.remove("cases");
        m.insert("summary".into(), json!("sdc.run-metrics/1 without `cases`; strings holding an absolute path removed"));
    }
    sanitize(&mut d);
    d
}

/// One line per element of a top-level array, the rest compact: small diffs, small files.
fn summary_text(d: &J) -> String {
    let mut s = String::from("{\n");
    let m = d.as_object().cloned().unwrap_or_default();
    let n = m.len();
    for (i, (k, v)) in m.iter().enumerate() {
        let _ = write!(s, "  {}: ", J::String(k.clone()));
        match v {
            J::Array(a) if !a.is_empty() => {
                s.push_str("[\n");
                let lines: Vec<String> = a.iter().map(|e| format!("    {e}")).collect();
                s.push_str(&lines.join(",\n"));
                s.push_str("\n  ]");
            }
            _ => s.push_str(&v.to_string()),
        }
        s.push_str(if i + 1 < n { ",\n" } else { "\n" });
    }
    s.push_str("}\n");
    s
}

fn load_summary(path: &Path) -> Result<Option<Run>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let doc: J = serde_json::from_str(&read_text(path)?).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Ok(Some(run_of(doc)))
}

/// Writes the summary of every listed run that has ended; removes the summary of a run that is
/// open again (reopened). Runs with no directory keep their summary. Returns one line per change.
fn refresh_summaries(r: &Y, exp: &Path, runs_dir: &Path) -> Result<Vec<String>, String> {
    let id = ystr(r, "id").unwrap_or("");
    let dir = summaries_dir(exp, id);
    let mut log = vec![];
    for v in variants(r) {
        for rr in &v.runs {
            let rd = runs_dir.join(&rr.run);
            if !rd.is_dir() {
                continue;
            }
            let run = load_run(&rd)?;
            let path = dir.join(format!("{}.json", rr.run));
            if finished(&run) {
                let text = summary_text(&summary_doc(&run.doc));
                if fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
                    fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
                    fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
                    log.push(format!("summary {}", path.display()));
                }
            } else if path.is_file() {
                fs::remove_file(&path).map_err(|e| format!("cannot remove {}: {e}", path.display()))?;
                log.push(format!("removed {} (the run is open again)", path.display()));
            }
        }
    }
    Ok(log)
}

/// Per variant: the variant, its runs with a summary, and the listed runs without one.
type Loaded = Vec<(Variant, Vec<Run>, Vec<String>)>;

fn load_variants(r: &Y, exp: &Path) -> Result<Loaded, String> {
    let dir = summaries_dir(exp, ystr(r, "id").unwrap_or(""));
    let mut out = vec![];
    for v in variants(r) {
        let mut rs = vec![];
        let mut missing = vec![];
        for rr in &v.runs {
            match load_summary(&dir.join(format!("{}.json", rr.run)))? {
                Some(x) if finished(&x) => rs.push(x),
                _ => missing.push(rr.run.clone()),
            }
        }
        out.push((v, rs, missing));
    }
    Ok(out)
}

/// Two values of one setting name the same thing: `decider.kind` `haiku` is the old name of
/// `model`, the LLM decider (D149), so records and runs from before and after read alike.
fn same_setting(key: &str, a: &str, b: &str) -> bool {
    let name = |v: &str| if key == "decider.kind" && v == "haiku" { "model".to_string() } else { v.to_string() };
    name(a) == name(b)
}

/// Mismatches between a variant's settings (with `fixed`) and what each run recorded, read from
/// its summary, else from its run directory under `runs_dir`.
fn check_runs(r: &Y, exp: &Path, runs_dir: Option<&Path>) -> (Vec<String>, Vec<String>) {
    let mut problems = vec![];
    let mut notes = vec![];
    let dir = summaries_dir(exp, ystr(r, "id").unwrap_or(""));
    let confounds: Vec<String> = yseq(r, "known_confounds").iter().filter_map(|c| ystr(c, "key").map(String::from)).collect();
    for v in variants(r) {
        for rr in &v.runs {
            let doc = match load_summary(&dir.join(format!("{}.json", rr.run))) {
                Ok(Some(x)) => x.doc,
                Ok(None) => match runs_dir.map(|d| d.join(&rr.run)).filter(|d| d.is_dir()) {
                    Some(d) => match load_run(&d) {
                        Ok(x) => x.doc,
                        Err(m) => {
                            problems.push(format!("variant {:?}: run {}: {m}", v.id, rr.run));
                            continue;
                        }
                    },
                    None => {
                        if runs_dir.is_some() {
                            problems.push(format!("variant {:?}: run {} has no summary and no run directory", v.id, rr.run));
                        }
                        continue;
                    }
                },
                Err(m) => {
                    problems.push(m);
                    continue;
                }
            };
            for (k, x) in &v.settings {
                // A loom run records the adapters too; it never records the binary's path or the login's.
                let skipped = if v.runner == "loom" { ["loom.bin", "loom.login_file"].contains(&k.as_str()) || REPLAY_FLAGS.iter().any(|(f, _)| f == k) } else { NOT_RECORDED.contains(&k.as_str()) };
                if skipped {
                    continue;
                }
                let got = doc["settings"][k.as_str()].as_str().unwrap_or("not recorded").to_string();
                if !same_setting(k, &got, x) {
                    let line = format!("variant {:?}: run {} has {k} = {got}, the record says {x}", v.id, rr.run);
                    if confounds.contains(k) { notes.push(format!("{line} (a known confound)")) } else { problems.push(line) }
                }
            }
        }
    }
    (problems, notes)
}

// ---- results -----------------------------------------------------------------------------------

fn clean_pass(r: &Run) -> bool {
    let m = &r.doc["metrics"];
    m["closed"] == true && m["delivered"] == true && m["clean"] == true
}

fn values_of(runs: &[Run], metric: &str) -> BTreeMap<String, f64> {
    runs.iter().filter_map(|r| metric_of(&r.doc, &r.flat, metric).map(|v| (r.name.clone(), v))).collect()
}

fn stat_json(v: &BTreeMap<String, f64>) -> J {
    let xs: Vec<f64> = v.values().copied().collect();
    match stat(&xs) {
        Some(s) => json!({"n": s.n, "median": s.median, "min": s.min, "max": s.max, "values": v}),
        None => json!({"n": 0, "median": null, "min": null, "max": null, "values": v}),
    }
}

/// The result of a record over its run summaries: per variant the counts, and with a hypothesis
/// the verdict by the complete-separation rule.
fn compute_result(r: &Y, loaded: &Loaded) -> J {
    let variant_docs: Vec<J> = loaded.iter().map(|(v, rs, missing)| json!({"id": v.id, "listed": v.runs.len(), "ended": rs.len(),
        "clean_passes": rs.iter().filter(|x| clean_pass(x)).count(), "without_summary": missing})).collect();
    if !has_hypothesis(r) {
        return json!({"computed_by": "sdc-experiment report", "verdict": "descriptive",
            "why": "no hypothesis: the record describes its runs and decides nothing", "variants": variant_docs});
    }
    if multi(r) {
        return compute_multi(r, loaded, variant_docs);
    }
    let primary = r.get("primary").cloned().unwrap_or(Y::Null);
    let metric = ystr(&primary, "metric").unwrap_or("").to_string();
    let direction = ystr(&primary, "direction").unwrap_or("decrease").to_string();
    let min_effect = primary.get("min_effect").and_then(Y::as_f64).unwrap_or(0.0);
    let rpv = rpv_of(r).max(1);
    let base = ystr(r, "baseline").unwrap_or("").to_string();
    let treat = ystr(r, "treatment").unwrap_or("").to_string();
    let empty = vec![];
    let runs_of = |id: &str| loaded.iter().find(|(v, _, _)| v.id == id).map_or(&empty, |(_, rs, _)| rs);
    let (b_runs, t_runs) = (runs_of(&base), runs_of(&treat));
    let bv = values_of(b_runs, &metric);
    let tv = values_of(t_runs, &metric);
    let bs = stat(&bv.values().copied().collect::<Vec<_>>());
    let ts = stat(&tv.values().copied().collect::<Vec<_>>());
    let mut why;
    let (verdict, separation, p, diff) = match (&bs, &ts) {
        (Some(b), Some(t)) if b.n >= rpv && t.n >= rpv => {
            let diff = t.median - b.median;
            let expected = if direction == "decrease" { t.max < b.min } else { t.min > b.max };
            let reversed = if direction == "decrease" { t.min > b.max } else { t.max < b.min };
            let p = 1.0 / choose(b.n + t.n, t.n);
            if expected && diff.abs() >= min_effect {
                why = format!("{treat} {} {metric} against {base} by {} (medians), ranges separate in the expected direction, n {} and {}, one-sided exact p {}",
                    if direction == "decrease" { "lowers" } else { "raises" }, fmt(diff.abs()), b.n, t.n, fmt((p * 1000.0).round() / 1000.0));
                ("held", "expected", Some(p), Some(diff))
            } else if expected {
                why = format!("the ranges separate in the expected direction but the median difference {} is below the hypothesised {}", fmt(diff.abs()), fmt(min_effect));
                ("contradicted", "expected", Some(p), Some(diff))
            } else if reversed {
                why = format!("the ranges separate against the expected direction (median difference {})", signed(diff));
                ("contradicted", "reversed", Some(p), Some(diff))
            } else {
                why = format!("the ranges overlap (median difference {}): no effect is claimed", signed(diff));
                ("inconclusive", "overlap", None, Some(diff))
            }
        }
        _ => {
            why = format!("too few ended runs with the metric: {base} {} and {treat} {}, the rule needs {rpv} each",
                bs.as_ref().map_or(0, |s| s.n), ts.as_ref().map_or(0, |s| s.n));
            ("inconclusive", "n", None, bs.as_ref().zip(ts.as_ref()).map(|(b, t)| t.median - b.median))
        }
    };
    let mut guard_docs = vec![];
    let mut guards_ok = true;
    let mut guards_known = true;
    for g in yseq(r, "guards") {
        let gm = ystr(&g, "metric").unwrap_or("").to_string();
        let worse_up = ystr(&g, "worse") == Some("increase");
        let gb = values_of(b_runs, &gm);
        let gt = values_of(t_runs, &gm);
        let sb = stat(&gb.values().copied().collect::<Vec<_>>());
        let st = stat(&gt.values().copied().collect::<Vec<_>>());
        let (state, worsening) = match (&sb, &st) {
            (Some(b), Some(t)) => {
                let w = if worse_up { t.median - b.median } else { b.median - t.median };
                let bound = g.get("max_worsening").and_then(Y::as_f64)
                    .or_else(|| g.get("max_worsening_ratio").and_then(Y::as_f64).map(|x| x * b.median.abs()))
                    .unwrap_or(0.0);
                if w <= bound + 1e-12 { ("holds", Some(w)) } else { ("fails", Some(w)) }
            }
            _ => ("unknown", None),
        };
        guards_ok &= state != "fails";
        guards_known &= state != "unknown";
        guard_docs.push(json!({"metric": gm, "state": state, "worsening": worsening, "baseline": stat_json(&gb), "treatment": stat_json(&gt)}));
    }
    let retrospective = ystr(r, "kind") == Some("retrospective");
    let recommendation = match (verdict, guards_ok) {
        ("contradicted", _) | (_, false) => "drop",
        ("held", true) if retrospective => "run prospectively",
        ("held", true) if guards_known => "adopt",
        _ => "run more",
    };
    if retrospective {
        why.push_str("; retrospective: the hypothesis was written after the runs, so this verdict proposes a hypothesis and decides nothing");
    }
    let mut variant_docs = variant_docs;
    for (d, (_, rs, _)) in variant_docs.iter_mut().zip(loaded) {
        d["primary"] = stat_json(&values_of(rs, &metric));
    }
    json!({
        "computed_by": "sdc-experiment report",
        "metric": metric, "direction": direction, "min_effect": min_effect,
        "variants": variant_docs,
        "median_difference": diff, "separation": separation, "p_one_sided": p,
        "guards": guard_docs, "guards_ok": guards_ok,
        "verdict": verdict, "why": why, "recommendation": recommendation,
    })
}

/// The complete-separation rule on one metric between a baseline's and the treatment's runs:
/// verdict, separation, exact one-sided p, median difference and why.
fn separate(bv: &BTreeMap<String, f64>, tv: &BTreeMap<String, f64>, direction: &str, min_effect: f64, rpv: usize) -> (&'static str, &'static str, Option<f64>, Option<f64>, String) {
    let bs = stat(&bv.values().copied().collect::<Vec<_>>());
    let ts = stat(&tv.values().copied().collect::<Vec<_>>());
    match (&bs, &ts) {
        (Some(b), Some(t)) if b.n >= rpv && t.n >= rpv => {
            let diff = t.median - b.median;
            let expected = if direction == "decrease" { t.max < b.min } else { t.min > b.max };
            let reversed = if direction == "decrease" { t.min > b.max } else { t.max < b.min };
            let p = 1.0 / choose(b.n + t.n, t.n);
            if expected && diff.abs() >= min_effect {
                ("held", "expected", Some(p), Some(diff), format!("ranges separate in the expected direction, median difference {}, n {} and {}, one-sided exact p {}",
                    signed(diff), b.n, t.n, fmt((p * 1000.0).round() / 1000.0)))
            } else if expected {
                ("contradicted", "expected", Some(p), Some(diff), format!("ranges separate as expected but the median difference {} is below {}", fmt(diff.abs()), fmt(min_effect)))
            } else if reversed {
                ("contradicted", "reversed", Some(p), Some(diff), format!("ranges separate against the expected direction (median difference {})", signed(diff)))
            } else {
                ("inconclusive", "overlap", None, Some(diff), format!("ranges overlap (median difference {}): no effect is claimed", signed(diff)))
            }
        }
        _ => ("inconclusive", "n", None, bs.as_ref().zip(ts.as_ref()).map(|(b, t)| t.median - b.median),
            format!("too few ended runs with the metric: {} and {}, the rule needs {rpv} each", bs.as_ref().map_or(0, |s| s.n), ts.as_ref().map_or(0, |s| s.n))),
    }
}

/// The result of a record with several baselines or several primary metrics: the rule on every
/// pair; held when every pair holds, contradicted when one is contradicted, else inconclusive.
fn compute_multi(r: &Y, loaded: &Loaded, mut variant_docs: Vec<J>) -> J {
    let rpv = rpv_of(r).max(1);
    let treat = ystr(r, "treatment").unwrap_or("").to_string();
    let empty = vec![];
    let runs_of = |id: &str| loaded.iter().find(|(v, _, _)| v.id == id).map_or(&empty, |(_, rs, _)| rs);
    let t_runs = runs_of(&treat);
    let mut comps = vec![];
    let mut whys = vec![];
    let mut verdicts = vec![];
    for primary in primaries(r) {
        let metric = ystr(&primary, "metric").unwrap_or("").to_string();
        let direction = ystr(&primary, "direction").unwrap_or("increase").to_string();
        let min_effect = primary.get("min_effect").and_then(Y::as_f64).unwrap_or(0.0);
        let tv = values_of(t_runs, &metric);
        for b in baselines(r) {
            let bv = values_of(runs_of(&b), &metric);
            let (verdict, sep, p, diff, why) = separate(&bv, &tv, &direction, min_effect, rpv);
            whys.push(format!("{metric}, {treat} against {b}: {verdict} ({why})"));
            verdicts.push(verdict);
            comps.push(json!({"metric": metric, "direction": direction, "min_effect": min_effect, "baseline": b, "treatment": treat,
                "verdict": verdict, "separation": sep, "p_one_sided": p, "median_difference": diff, "why": why,
                "baseline_values": stat_json(&bv), "treatment_values": stat_json(&tv)}));
        }
        for (d, (_, rs, _)) in variant_docs.iter_mut().zip(loaded) {
            d["primaries"][metric.as_str()] = stat_json(&values_of(rs, &metric));
        }
    }
    let verdict = if !verdicts.is_empty() && verdicts.iter().all(|v| *v == "held") {
        "held"
    } else if verdicts.contains(&"contradicted") {
        "contradicted"
    } else {
        "inconclusive"
    };
    let mut guard_docs = vec![];
    let mut guards_ok = true;
    for g in yseq(r, "guards") {
        let gm = ystr(&g, "metric").unwrap_or("").to_string();
        let worse_up = ystr(&g, "worse") == Some("increase");
        let gt = values_of(t_runs, &gm);
        for b in baselines(r) {
            let gb = values_of(runs_of(&b), &gm);
            let sb = stat(&gb.values().copied().collect::<Vec<_>>());
            let st = stat(&gt.values().copied().collect::<Vec<_>>());
            let (state, worsening) = match (&sb, &st) {
                (Some(bs), Some(ts)) => {
                    let w = if worse_up { ts.median - bs.median } else { bs.median - ts.median };
                    let bound = g.get("max_worsening").and_then(Y::as_f64)
                        .or_else(|| g.get("max_worsening_ratio").and_then(Y::as_f64).map(|x| x * bs.median.abs())).unwrap_or(0.0);
                    if w <= bound + 1e-12 { ("holds", Some(w)) } else { ("fails", Some(w)) }
                }
                _ => ("unknown", None),
            };
            guards_ok &= state != "fails";
            guard_docs.push(json!({"metric": gm, "baseline": b, "state": state, "worsening": worsening, "baseline_values": stat_json(&gb), "treatment_values": stat_json(&gt)}));
        }
    }
    json!({
        "computed_by": "sdc-experiment report",
        "comparisons": comps, "variants": variant_docs,
        "guards": guard_docs, "guards_ok": guards_ok,
        "verdict": verdict,
        "why": format!("{} on every pair of primary metric and baseline is required for held; {}", "held", whys.join("; ")),
        "recommendation": "none: a comparison of runners, nothing to adopt",
    })
}

// ---- report.md and INDEX.md --------------------------------------------------------------------

const HEADLINE: [&str; 14] = ["run.cost_usd", "run.llm_calls", "run.decider_calls", "run.wall_s", "run.driver_s", "run.real_takes",
    "run.rework_takes", "run.looping_factor", "run.first_pass_yield", "run.rework_cost_usd", "run.furthest_stage_index",
    "run.findings_raised", "run.waiting_on_person_s", "run.clean"];

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn stat_cell(s: &Option<Stat>) -> String {
    s.as_ref().map_or("-".into(), |s| format!("{} [{}, {}] ({})", fmt(s.median), fmt(s.min), fmt(s.max), s.n))
}

fn short_tree(r: &Y) -> String {
    match r.get("tree_commit") {
        Some(Y::String(s)) if is_hex(s, 7, 40) => s[..7].to_string(),
        Some(Y::String(s)) => s.clone(),
        _ => "-".into(),
    }
}

/// The three loop nodes (writers and approvals that were taken again at least once) with the
/// highest median looping factor, then rework cost, and the three edges with the highest loop
/// cost, then count, over a variant's runs.
fn top_loops(rs: &[Run]) -> (Vec<String>, Vec<String>) {
    struct N { kind: String, lf: Vec<f64>, rework: u64, cost: f64 }
    let mut nodes: BTreeMap<String, N> = BTreeMap::new();
    struct E { kind: String, runs: usize, count: u64, cost: f64, secs: f64 }
    let mut edges: BTreeMap<String, E> = BTreeMap::new();
    for r in rs {
        for n in r.doc["nodes"].as_array().into_iter().flatten() {
            let kind = n["kind"].as_str().unwrap_or("");
            let loop_node = kind == "llm-writer" || kind == "approval" || (kind == "writer" && n["action"] != "amend.upstream");
            if !loop_node || n["real_takes"].as_u64().unwrap_or(0) == 0 {
                continue;
            }
            let e = nodes.entry(n["node"].as_str().unwrap_or("").to_string())
                .or_insert(N { kind: kind.into(), lf: vec![], rework: 0, cost: 0.0 });
            if let Some(x) = n["looping_factor"].as_f64() {
                e.lf.push(x);
            }
            e.rework += n["rework_takes"].as_u64().unwrap_or(0);
            e.cost += n["rework_cost_usd"].as_f64().unwrap_or(0.0);
        }
        for x in r.doc["edges"].as_array().into_iter().flatten() {
            let e = edges.entry(x["edge"].as_str().unwrap_or("").to_string())
                .or_insert(E { kind: x["kind"].as_str().unwrap_or("").into(), runs: 0, count: 0, cost: 0.0, secs: 0.0 });
            e.runs += 1;
            e.count += x["count"].as_u64().unwrap_or(0);
            e.cost += x["loop_cost_usd"].as_f64().unwrap_or(0.0);
            e.secs += x["loop_driver_s"].as_f64().unwrap_or(0.0);
        }
    }
    let mut ns: Vec<(String, N, Stat)> = nodes.into_iter().filter_map(|(k, n)| stat(&n.lf).map(|s| (k, n, s))).collect();
    ns.retain(|(_, n, _)| n.rework > 0);
    ns.sort_by(|a, b| b.2.median.total_cmp(&a.2.median).then(b.1.cost.total_cmp(&a.1.cost)).then(a.0.cmp(&b.0)));
    let mut es: Vec<(String, E)> = edges.into_iter().collect();
    es.sort_by(|a, b| b.1.cost.total_cmp(&a.1.cost).then(b.1.count.cmp(&a.1.count)).then(a.0.cmp(&b.0)));
    let round3 = |x: f64| (x * 1000.0).round() / 1000.0;
    (ns.iter().take(3).map(|(k, n, s)| format!("`{k}` ({}): looping factor median {} (max {}, n {}), rework takes {}, rework cost {} USD",
        n.kind, fmt(s.median), fmt(s.max), s.n, n.rework, fmt(round3(n.cost)))).collect(),
     es.iter().take(3).map(|(k, e)| format!("`{k}` ({}): count {} in {} runs, loop cost {} USD, loop driver {} s",
        e.kind, e.count, e.runs, fmt(round3(e.cost)), fmt(e.secs.round()))).collect())
}

fn render_report(r: &Y, loaded: &Loaded, result: &J) -> String {
    let id = ystr(r, "id").unwrap_or("");
    let mut md = String::new();
    let _ = writeln!(md, "# {id}: {}\n", ystr(r, "title").unwrap_or(""));
    let _ = writeln!(md, "Written by `sdc-experiment report {id}` from `experiments/{id}.yaml` and the run summaries in `experiments/{id}/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.\n");
    let _ = writeln!(md, "- Kind: {}; status: {}; created {}.", ystr(r, "kind").unwrap_or(""), ystr(r, "status").unwrap_or(""),
        r.get("created").map(yscalar).unwrap_or_default());
    let _ = writeln!(md, "- Purpose: {}", ystr(r, "purpose").unwrap_or(""));
    match ystr(r, "hypothesis").filter(|h| !h.trim().is_empty()) {
        Some(h) => { let _ = writeln!(md, "- Hypothesis: {h}"); }
        None => { let _ = writeln!(md, "- Hypothesis: none; the record describes its runs."); }
    }
    for i in yseq(r, "inputs") {
        let sha = ystr(&i, "sha256").unwrap_or("");
        let _ = writeln!(md, "- Input `{}` ({}{}, sha256 {}): {}", ystr(&i, "name").unwrap_or(""), ystr(&i, "kind").unwrap_or(""),
            ystr(&i, "seed").map(|s| format!(" `{s}`")).unwrap_or_default(), &sha[..sha.len().min(12)], ystr(&i, "text").unwrap_or("").trim());
    }
    let _ = writeln!(md, "- Tree: {}", match r.get("tree_commit") { Some(Y::String(s)) => format!("`{s}`"), _ => "not pinned yet".into() });
    for t in yseq(r, "trees").iter().filter_map(Y::as_str) {
        let _ = writeln!(md, "  - {t}");
    }
    let _ = writeln!(md, "- Runs per variant: {}; pass rule: {} (closed, delivered, no reopen, forced step or unstick).", rpv_of(r), ystr(r, "pass_rule").unwrap_or(""));
    let _ = writeln!(md, "- Decision rule ({}): {}", ystr(&r["decision_rule"], "rule").unwrap_or(""), ystr(&r["decision_rule"], "text").unwrap_or("").trim());
    // Settings.
    let keys: BTreeSet<String> = loaded.iter().flat_map(|(v, _, _)| v.settings.keys().cloned()).collect();
    if !keys.is_empty() {
        let _ = writeln!(md, "\n## Settings\n");
        let _ = writeln!(md, "| setting | {} |", loaded.iter().map(|(v, _, _)| v.id.as_str()).collect::<Vec<_>>().join(" | "));
        let _ = writeln!(md, "|---|{}", "---|".repeat(loaded.len()));
        for k in &keys {
            let _ = writeln!(md, "| `{k}` | {} |", loaded.iter().map(|(v, _, _)| cell(v.settings.get(k).map_or("-", String::as_str))).collect::<Vec<_>>().join(" | "));
        }
    }
    let confounds = yseq(r, "known_confounds");
    if !confounds.is_empty() {
        let _ = writeln!(md, "\nKnown confounds:\n");
        for c in &confounds {
            let _ = writeln!(md, "- `{}`: {}", ystr(c, "key").unwrap_or(""), ystr(c, "note").unwrap_or("").trim());
        }
    }
    // Runs.
    let _ = writeln!(md, "\n## Runs\n");
    let with_input = yseq(r, "inputs").len() > 1;
    let with_tree = loaded.iter().any(|(v, _, _)| v.runs.iter().any(|x| x.tree.is_some()));
    let _ = writeln!(md, "| run | variant |{}{} end | clean pass | cost USD | wall s | looping factor | note |",
        if with_input { " input |" } else { "" }, if with_tree { " tree |" } else { "" });
    let _ = writeln!(md, "|---|---|{}{}---|---|---|---|---|---|", if with_input { "---|" } else { "" }, if with_tree { "---|" } else { "" });
    for (v, rs, _) in loaded {
        for rr in &v.runs {
            let run = rs.iter().find(|x| x.name == rr.run);
            let m = run.map(|x| &x.doc["metrics"]);
            let num = |k: &str| m.and_then(|m| m[k].as_f64()).map_or("-".into(), |x| fmt((x * 1000.0).round() / 1000.0));
            let _ = writeln!(md, "| {} | {} |{}{} {} | {} | {} | {} | {} | {} |", rr.run, v.id,
                if with_input { format!(" {} |", rr.input.as_deref().unwrap_or("-")) } else { String::new() },
                if with_tree { format!(" {} |", rr.tree.as_deref().unwrap_or("-")) } else { String::new() },
                cell(m.and_then(|m| m["end"].as_str()).unwrap_or("no summary: not ended")),
                run.map_or("-", |x| if clean_pass(x) { "yes" } else { "no" }),
                num("cost_usd"), num("wall_s"), num("looping_factor"), cell(rr.note.as_deref().unwrap_or("")));
        }
    }
    // Per variant.
    let _ = writeln!(md, "\n## Per variant\n");
    let _ = writeln!(md, "Median [min, max] (n) over the ended runs.\n");
    let _ = writeln!(md, "| | {} |", loaded.iter().map(|(v, _, _)| v.id.as_str()).collect::<Vec<_>>().join(" | "));
    let _ = writeln!(md, "|---|{}", "---|".repeat(loaded.len()));
    let _ = writeln!(md, "| runs listed | {} |", loaded.iter().map(|(v, _, _)| v.runs.len().to_string()).collect::<Vec<_>>().join(" | "));
    let _ = writeln!(md, "| ended | {} |", loaded.iter().map(|(_, rs, _)| rs.len().to_string()).collect::<Vec<_>>().join(" | "));
    let _ = writeln!(md, "| clean passes | {} |", loaded.iter().map(|(_, rs, _)| rs.iter().filter(|x| clean_pass(x)).count().to_string()).collect::<Vec<_>>().join(" | "));
    for m in HEADLINE {
        let _ = writeln!(md, "| `{m}` | {} |", loaded.iter().map(|(_, rs, _)| {
            let v: Vec<f64> = rs.iter().filter_map(|x| metric_of(&x.doc, &x.flat, m)).collect();
            stat_cell(&stat(&v))
        }).collect::<Vec<_>>().join(" | "));
    }
    // Loops.
    let _ = writeln!(md, "\n## Loops\n");
    let _ = writeln!(md, "Per variant, over the ended runs: of the writer and approval nodes taken again at least once, the three with the highest median looping factor; and the three edges with the highest loop cost.\n");
    for (v, rs, _) in loaded {
        let (ns, es) = top_loops(rs);
        let _ = writeln!(md, "- {} (n {}):", v.id, rs.len());
        if ns.is_empty() && es.is_empty() {
            let _ = writeln!(md, "  - no node taken again, no edge");
        }
        for x in ns.iter().chain(&es) {
            let _ = writeln!(md, "  - {x}");
        }
    }
    // Verdict.
    let _ = writeln!(md, "\n## Verdict\n");
    let _ = writeln!(md, "{}: {}.", result["verdict"].as_str().unwrap_or(""), result["why"].as_str().unwrap_or(""));
    if has_hypothesis(r) && multi(r) {
        let _ = writeln!(md, "\nEach pair of primary metric and baseline (median [min, max] (n)):\n");
        let st = |v: &J| format!("{} [{}, {}] ({})", v["median"].as_f64().map_or("-".into(), |x| fmt((x * 1000.0).round() / 1000.0)),
            v["min"].as_f64().map_or("-".into(), |x| fmt((x * 1000.0).round() / 1000.0)),
            v["max"].as_f64().map_or("-".into(), |x| fmt((x * 1000.0).round() / 1000.0)), v["n"]);
        for c in result["comparisons"].as_array().into_iter().flatten() {
            let _ = writeln!(md, "- `{}` ({}), {} against {}: {}; {} {}, {} {}; {}", c["metric"].as_str().unwrap_or(""), c["direction"].as_str().unwrap_or(""),
                c["treatment"].as_str().unwrap_or(""), c["baseline"].as_str().unwrap_or(""), c["verdict"].as_str().unwrap_or(""),
                c["treatment"].as_str().unwrap_or(""), st(&c["treatment_values"]), c["baseline"].as_str().unwrap_or(""), st(&c["baseline_values"]),
                c["why"].as_str().unwrap_or(""));
        }
        for g in result["guards"].as_array().into_iter().flatten() {
            let _ = writeln!(md, "- guard `{}` against {}: {} (worsening {})", g["metric"].as_str().unwrap_or(""), g["baseline"].as_str().unwrap_or(""),
                g["state"].as_str().unwrap_or(""), g["worsening"].as_f64().map_or("-".into(), |x| fmt((x * 1000.0).round() / 1000.0)));
        }
    } else if has_hypothesis(r) {
        let _ = writeln!(md, "\nPrimary metric `{}` ({}, at least {}):\n", result["metric"].as_str().unwrap_or(""),
            result["direction"].as_str().unwrap_or(""), result["min_effect"]);
        for v in result["variants"].as_array().into_iter().flatten() {
            let p = &v["primary"];
            let _ = writeln!(md, "- {}: n {}, median {}, range [{}, {}]", v["id"].as_str().unwrap_or(""), p["n"], p["median"], p["min"], p["max"]);
        }
        for g in result["guards"].as_array().into_iter().flatten() {
            let _ = writeln!(md, "- guard `{}`: {} (worsening {})", g["metric"].as_str().unwrap_or(""), g["state"].as_str().unwrap_or(""),
                g["worsening"].as_f64().map_or("-".into(), |x| fmt((x * 1000.0).round() / 1000.0)));
        }
        let _ = writeln!(md, "- recommendation: {}", result["recommendation"].as_str().unwrap_or(""));
    }
    let facts: Vec<String> = yseq(r, "facts").iter().filter_map(|x| x.as_str().map(String::from)).collect();
    if !facts.is_empty() {
        let _ = writeln!(md, "\n## Facts\n");
        for f in facts {
            let _ = writeln!(md, "- {}", f.trim());
        }
    }
    let links: Vec<String> = yseq(r, "links").iter().filter_map(|x| x.as_str().map(String::from)).collect();
    if !links.is_empty() {
        let _ = writeln!(md, "\n## Links\n");
        for l in links {
            let _ = writeln!(md, "- {}", l.trim());
        }
    }
    md
}

fn record_paths(exp: &Path) -> Result<Vec<PathBuf>, String> {
    let mut v: Vec<PathBuf> = fs::read_dir(exp).map_err(|e| format!("cannot read {}: {e}", exp.display()))?
        .flatten().map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "yaml") && p.file_name().is_none_or(|n| n != progress::RELEASES && n != TARGETS))
        .collect();
    v.sort();
    Ok(v)
}

/// The operator's targets (`experiments/targets.yaml`, `sdc.targets/1`), not a record.
const TARGETS: &str = "targets.yaml";

fn is_metric(m: &str) -> bool {
    METRIC_PREFIXES.iter().any(|x| m.starts_with(x)) && m.rsplit_once('.').is_some_and(|(_, f)| !f.is_empty())
}

/// Each target: who set it and their words, a metric, a figure, the experiment it comes from, one
/// reference record per setting that exists, and guards on metrics that must not get worse.
fn check_targets(exp: &Path) -> Vec<String> {
    let path = exp.join(TARGETS);
    let Ok(raw) = fs::read_to_string(&path) else { return vec![] };
    let at = path.display().to_string();
    let doc: Y = match serde_yaml::from_str(&raw) {
        Ok(d) => d,
        Err(e) => return vec![format!("{at}: not YAML: {e}")],
    };
    let mut p = vec![];
    if ystr(&doc, "format") != Some("sdc.targets/1") {
        p.push("`format` must be sdc.targets/1".to_string());
    }
    let ts = yseq(&doc, "targets");
    if ts.is_empty() {
        p.push("`targets` must list at least one target".to_string());
    }
    let mut ids = BTreeSet::new();
    for t in &ts {
        let id = ystr(t, "id").unwrap_or("");
        let who = if id.is_empty() { "a target".to_string() } else { format!("target {id:?}") };
        if id.is_empty() || !ids.insert(id.to_string()) {
            p.push(format!("{who}: `id` must be present and unique"));
        }
        for k in ["set", "by", "said", "basis", "source_experiment", "product_check"] {
            if t.get(k).map(yscalar).is_none_or(|s| s.trim().is_empty()) {
                p.push(format!("{who}: `{k}` is missing"));
            }
        }
        if !ystr(t, "metric").is_some_and(is_metric) {
            p.push(format!("{who}: `metric` must be a metric name (run., node., edge. or decision.<...>.<field>)"));
        }
        if t.get("target").and_then(Y::as_f64).is_none_or(|x| x < 0.0) {
            p.push(format!("{who}: `target` must be a number of at least 0"));
        }
        let settings = yseq(t, "per_setting");
        if settings.is_empty() {
            p.push(format!("{who}: `per_setting` must list at least one setting"));
        }
        for s in &settings {
            let name = ystr(s, "setting").unwrap_or("");
            match ystr(s, "reference") {
                Some(r) if exp.join(format!("{r}.yaml")).is_file() => {}
                r => p.push(format!("{who}, setting {name:?}: `reference` {:?} names no record experiments/<id>.yaml", r.unwrap_or(""))),
            }
            for k in ["now", "target"] {
                if s.get(k).and_then(Y::as_f64).is_none() {
                    p.push(format!("{who}, setting {name:?}: `{k}` must be a number"));
                }
            }
        }
        let guards = yseq(t, "guards");
        if guards.is_empty() {
            p.push(format!("{who}: `guards` must list the metrics that must not get worse"));
        }
        for g in &guards {
            if !ystr(g, "metric").is_some_and(is_metric) || !matches!(ystr(g, "worse"), Some("increase" | "decrease")) {
                p.push(format!("{who}: a guard needs a `metric` name and `worse`: increase or decrease"));
            }
        }
    }
    p.into_iter().map(|x| format!("{at}: {x}")).collect()
}

fn render_index(exp: &Path) -> Result<String, String> {
    let mut md = String::from("# Experiments\n\nWritten by `sdc-experiment index` (and by every `sdc-experiment report`); `sdc-experiment check --all` fails when it is stale. One row per record `experiments/<id>.yaml`; the procedure is in `docs/workflow-metrics.md`.\n\n");
    md.push_str("| experiment | created | kind | purpose | tree | variants × runs | status | verdict |\n|---|---|---|---|---|---|---|---|\n");
    for p in record_paths(exp)? {
        let (r, _) = read_record(&p)?;
        let id = ystr(&r, "id").unwrap_or("");
        let prospective = ystr(&r, "kind") == Some("prospective");
        let vs = variants(&r)
            .iter()
            .map(|v| if prospective { format!("{} {}/{}", v.id, v.runs.len(), rpv_of(&r)) } else { format!("{} {}", v.id, v.runs.len()) })
            .collect::<Vec<_>>()
            .join(", ");
        let verdict = r.get("result").and_then(|x| ystr(x, "verdict")).unwrap_or("-");
        let _ = writeln!(md, "| [{id}]({id}/report.md) | {} | {} | {} | {} | {} | {} | {} |", r.get("created").map(yscalar).unwrap_or_default(),
            ystr(&r, "kind").unwrap_or(""), cell(ystr(&r, "purpose").unwrap_or("")), short_tree(&r), vs, ystr(&r, "status").unwrap_or(""), verdict);
    }
    Ok(md)
}

// ---- check -------------------------------------------------------------------------------------

/// Every record valid, its settings matched by its runs' summaries, its report and verdict
/// as the summaries give them, no stray summary, and INDEX.md current.
fn check_all(exp: &Path) -> Result<(Vec<String>, Vec<String>), String> {
    let mut problems = vec![];
    let mut notes = vec![];
    for p in record_paths(exp)? {
        let (r, raw) = read_record(&p)?;
        let at = p.display().to_string();
        let ps = check_record(&r, &p, &raw);
        if !ps.is_empty() {
            problems.extend(ps.into_iter().map(|x| format!("{at}: {x}")));
            continue;
        }
        let (rp, rn) = check_runs(&r, exp, None);
        problems.extend(rp.into_iter().map(|x| format!("{at}: {x}")));
        notes.extend(rn.into_iter().map(|x| format!("{at}: {x}")));
        let id = ystr(&r, "id").unwrap_or("");
        let loaded = load_variants(&r, exp)?;
        let result = compute_result(&r, &loaded);
        let report = exp.join(id).join("report.md");
        match fs::read_to_string(&report) {
            Ok(t) if t == render_report(&r, &loaded, &result) => {}
            Ok(_) => problems.push(format!("{}: stale; run sdc-experiment report {id}", report.display())),
            Err(_) => problems.push(format!("{}: missing; run sdc-experiment report {id}", report.display())),
        }
        let recorded = r.get("result").and_then(|x| ystr(x, "verdict"));
        if ystr(&r, "status") != Some("planned") && recorded != result["verdict"].as_str() {
            problems.push(format!("{at}: `result.verdict` is {} but the summaries give {}; run sdc-experiment report {id}",
                recorded.unwrap_or("absent"), result["verdict"].as_str().unwrap_or("")));
        }
        let listed: BTreeSet<String> = variants(&r).iter().flat_map(|v| v.runs.iter().map(|x| format!("{}.json", x.run))).collect();
        for e in fs::read_dir(summaries_dir(exp, id)).into_iter().flatten().flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if !listed.contains(&n) {
                problems.push(format!("{}: a summary of a run the record does not list", e.path().display()));
            }
            if let Ok(t) = fs::read_to_string(e.path()) {
                if has_abs_path(&t) {
                    problems.push(format!("{}: holds an absolute path", e.path().display()));
                }
            }
        }
    }
    let index = exp.join("INDEX.md");
    match fs::read_to_string(&index) {
        Ok(t) if t == render_index(exp)? => {}
        Ok(_) => problems.push(format!("{}: stale; run sdc-experiment index", index.display())),
        Err(_) => problems.push(format!("{}: missing; run sdc-experiment index", index.display())),
    }
    problems.extend(progress::check(exp)?);
    problems.extend(check_targets(exp));
    Ok((problems, notes))
}

fn print_problems(problems: &[String], notes: &[String], ok: &str) -> ExitCode {
    for n in notes {
        println!("note: {n}");
    }
    if problems.is_empty() {
        println!("{ok}: ok");
        ExitCode::SUCCESS
    } else {
        for x in problems {
            println!("{x}");
        }
        ExitCode::from(1)
    }
}

fn check(args: &[String]) -> Result<ExitCode, Usage> {
    let (mut rec, mut runs_dir, mut all, mut exp) = (None::<String>, None::<PathBuf>, false, PathBuf::from("experiments"));
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--runs-dir" => runs_dir = Some(PathBuf::from(it.next().ok_or_else(|| Usage("--runs-dir needs a directory".into()))?)),
            "--experiments" => exp = PathBuf::from(it.next().ok_or_else(|| Usage("--experiments needs a directory".into()))?),
            "--all" => all = true,
            x if x.starts_with("--") => return Err(Usage(format!("unknown argument {x}"))),
            x => rec = Some(x.to_string()),
        }
    }
    if all {
        if rec.is_some() {
            return Err(Usage("check --all takes no record".into()));
        }
        return Ok(match check_all(&exp) {
            Ok((p, n)) => print_problems(&p, &n, &exp.display().to_string()),
            Err(m) => {
                eprintln!("sdc-experiment: {m}");
                ExitCode::from(2)
            }
        });
    }
    let path = record_path(&rec.ok_or_else(|| Usage("check needs a record or --all".into()))?);
    let (r, raw) = match read_record(&path) {
        Ok(x) => x,
        Err(m) => {
            eprintln!("sdc-experiment: {m}");
            return Ok(ExitCode::from(2));
        }
    };
    let mut problems: Vec<String> = check_record(&r, &path, &raw).into_iter().map(|x| format!("{}: {x}", path.display())).collect();
    let (p, notes) = check_runs(&r, &experiments_dir_of(&path), runs_dir.as_deref());
    problems.extend(p.into_iter().map(|x| format!("{}: {x}", path.display())));
    Ok(print_problems(&problems, &notes, &path.display().to_string()))
}

// ---- report and index --------------------------------------------------------------------------

fn report(args: &[String]) -> Result<ExitCode, Usage> {
    let (mut rec, mut runs_dir, mut print_only) = (None::<String>, PathBuf::from("runs"), false);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--runs-dir" => runs_dir = PathBuf::from(it.next().ok_or_else(|| Usage("--runs-dir needs a directory".into()))?),
            "--print" => print_only = true,
            x if x.starts_with("--") => return Err(Usage(format!("unknown argument {x}"))),
            x => rec = Some(x.to_string()),
        }
    }
    let path = record_path(&rec.ok_or_else(|| Usage("report needs a record".into()))?);
    let exp = experiments_dir_of(&path);
    let (mut r, raw) = match read_record(&path) {
        Ok(x) => x,
        Err(m) => {
            eprintln!("sdc-experiment: {m}");
            return Ok(ExitCode::from(2));
        }
    };
    let problems = check_record(&r, &path, &raw);
    if !problems.is_empty() {
        for x in &problems {
            println!("{}: {x}", path.display());
        }
        return Ok(ExitCode::from(1));
    }
    let id = ystr(&r, "id").unwrap_or("").to_string();
    let step = || -> Result<(J, Loaded), String> {
        if !print_only && runs_dir.is_dir() {
            for l in refresh_summaries(&r, &exp, &runs_dir)? {
                println!("{l}");
            }
        }
        let loaded = load_variants(&r, &exp)?;
        Ok((compute_result(&r, &loaded), loaded))
    };
    let (result, loaded) = match step() {
        Ok(x) => x,
        Err(m) => {
            eprintln!("sdc-experiment: {m}");
            return Ok(ExitCode::from(2));
        }
    };
    println!("{id}: {} ({})", result["verdict"].as_str().unwrap_or(""), result["why"].as_str().unwrap_or(""));
    for v in result["variants"].as_array().into_iter().flatten() {
        println!("  {}: listed {}, ended {}, clean passes {}{}", v["id"].as_str().unwrap_or(""), v["listed"], v["ended"], v["clean_passes"],
            v.get("primary").map(|p| format!(", primary n {} median {} range [{}, {}]", p["n"], p["median"], p["min"], p["max"])).unwrap_or_default());
    }
    for g in result["guards"].as_array().into_iter().flatten() {
        println!("  guard {}: {} (worsening {})", g["metric"].as_str().unwrap_or(""), g["state"].as_str().unwrap_or(""), g["worsening"]);
    }
    if let Some(x) = result["recommendation"].as_str() {
        println!("  recommendation: {x}");
    }
    if print_only {
        return Ok(ExitCode::SUCCESS);
    }
    // A planned record has no runs yet, so no result.
    if ystr(&r, "status") == Some("planned") {
        if let Some(m) = r.as_mapping_mut() {
            m.remove("result");
        }
    } else {
        set_key(&mut r, "result", serde_yaml::to_value(&result).unwrap_or(Y::Null), "adoption");
    }
    let all_ended = loaded.iter().all(|(_, rs, _)| rs.len() >= rpv_of(&r));
    if ystr(&r, "kind") == Some("prospective") && ystr(&r, "status") == Some("running") && all_ended {
        set_key(&mut r, "status", Y::String("complete".into()), "created");
        println!("  status: complete (every variant has {} ended runs)", rpv_of(&r));
    }
    let report_path = exp.join(&id).join("report.md");
    let written = (|| -> Result<(), String> {
        write_record(&path, &r)?;
        fs::create_dir_all(exp.join(&id)).map_err(|e| format!("cannot create {}: {e}", exp.join(&id).display()))?;
        fs::write(&report_path, render_report(&r, &loaded, &result)).map_err(|e| format!("cannot write {}: {e}", report_path.display()))?;
        let index = render_index(&exp)?;
        fs::write(exp.join("INDEX.md"), index).map_err(|e| format!("cannot write INDEX.md: {e}"))
    })();
    match written {
        Ok(()) => {
            println!("wrote {}, {} and {}", path.display(), report_path.display(), exp.join("INDEX.md").display());
            match progress::write(&exp) {
                Ok(log) => log.iter().for_each(|l| println!("{l}")),
                Err(m) => println!("progress ledger not written: {m}"),
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(m) => {
            eprintln!("sdc-experiment: {m}");
            Ok(ExitCode::from(1))
        }
    }
}

fn index(args: &[String]) -> Result<ExitCode, Usage> {
    let mut exp = PathBuf::from("experiments");
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--experiments" => exp = PathBuf::from(it.next().ok_or_else(|| Usage("--experiments needs a directory".into()))?),
            x => return Err(Usage(format!("unknown argument {x}"))),
        }
    }
    match render_index(&exp).and_then(|t| fs::write(exp.join("INDEX.md"), t).map_err(|e| format!("cannot write INDEX.md: {e}"))) {
        Ok(()) => {
            println!("wrote {}", exp.join("INDEX.md").display());
            Ok(ExitCode::SUCCESS)
        }
        Err(m) => {
            eprintln!("sdc-experiment: {m}");
            Ok(ExitCode::from(2))
        }
    }
}

fn progress_cmd(args: &[String]) -> Result<ExitCode, Usage> {
    let mut exp = PathBuf::from("experiments");
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--experiments" => exp = PathBuf::from(it.next().ok_or_else(|| Usage("--experiments needs a directory".into()))?),
            x => return Err(Usage(format!("unknown argument {x}"))),
        }
    }
    if !exp.join(progress::RELEASES).is_file() {
        eprintln!("sdc-experiment: no {}", exp.join(progress::RELEASES).display());
        return Ok(ExitCode::from(2));
    }
    match progress::write(&exp) {
        Ok(log) => {
            log.iter().for_each(|l| println!("{l}"));
            println!("progress ledger: ok");
            Ok(ExitCode::SUCCESS)
        }
        Err(m) => {
            println!("{m}");
            Ok(ExitCode::from(1))
        }
    }
}

// ---- launcher ----------------------------------------------------------------------------------

/// One run a prospective record asks for: variant, ordinal, run name and systemd unit.
struct Planned {
    variant: Variant,
    ordinal: usize,
    name: String,
    unit: String,
}

fn unit_part(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

/// variants × n in the order they are started: ordinal 1 of every variant, then ordinal 2, so
/// the variants interleave in time.
fn planned_runs(r: &Y) -> Vec<Planned> {
    let id = ystr(r, "id").unwrap_or("");
    let vs = variants(r);
    let mut out = vec![];
    for n in 1..=rpv_of(r) {
        for v in vs.iter().filter(|v| v.from.is_none()) {
            let name = v.run_name.as_deref().map(|t| t.replace("{n}", &n.to_string())).unwrap_or_else(|| format!("{id}.{}.{n}", v.id));
            out.push(Planned { variant: v.clone(), ordinal: n, name,
                unit: format!("sdc-exp-{}-{}-{n}", unit_part(id), unit_part(&v.id)) });
        }
    }
    out
}

fn principal_of(r: &Y) -> String {
    r.get("launch").and_then(|l| ystr(l, "principal")).unwrap_or("agent:sdc-experiment").to_string()
}

/// The environment of one launched run: the login's base variables (never an SDC_ one), the runs
/// directory, the principal, and the record's settings.
fn launch_env(settings: &BTreeMap<String, String>, runs_dir: &Path, principal: &str, base: &dyn Fn(&str) -> Option<String>) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = BASE_ENV.iter().filter_map(|k| base(k).map(|v| (k.to_string(), v))).collect();
    env.push(("SDC_RUNS_DIR".into(), runs_dir.display().to_string()));
    env.push(("SDC_PRINCIPAL".into(), principal.to_string()));
    for (k, var) in LAUNCH {
        if let (false, Some(v)) = (var.is_empty(), settings.get(k)) {
            env.push((var.to_string(), v.clone()));
        }
    }
    env
}

/// The environment of a `loom` run's unit: the login's base variables and nothing else. No
/// `SDC_` variable, not even the runs directory or the principal: `runbook run` reads its
/// settings from the settings file and its paths from its arguments, and builds the environment
/// of Loom's programs itself.
///
/// `PATH` is the login's with the build's `driver/target/debug` put first. `runbook run`
/// gives `b10x-loom` (and so every bound program) its own `PATH`, and it is the one it was
/// started with: `sdc-reply` (the model decider and the model nodes), `sdc-findings` and
/// `sdc-bugfix` (the model nodes) are found there and nowhere else, because the copy of the
/// bindings in the run directory holds no `driver/target/debug`. A `decider.reply_bin` setting
/// would reach the decider alone; the directory on `PATH` reaches both. `jq` and `yq`
/// come from the login's `PATH` (`missing_loom_tools`).
fn loom_env(base: &dyn Fn(&str) -> Option<String>, build: &Path) -> Vec<(String, String)> {
    BASE_ENV.iter().map(|k| (*k, base(k)))
        .filter_map(|(k, v)| match (k, v) {
            ("PATH", v) => {
                let login = v.filter(|p| !p.is_empty()).unwrap_or_else(|| "/usr/bin:/bin".to_string());
                Some((k.to_string(), format!("{}:{login}", build.join("driver/target/debug").display())))
            }
            (_, v) => v.map(|v| (k.to_string(), v)),
        })
        .collect()
}

/// Whether a directory of `path` holds `tool` as an executable file.
fn on_path(path: &str, tool: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.split(':').filter(|d| !d.is_empty())
        .any(|d| fs::metadata(Path::new(d).join(tool)).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0))
}

/// The programs of `LOOM_TOOLS` that no directory of the unit's `PATH` holds as an executable file.
fn missing_loom_tools(path: &str) -> Vec<&'static str> {
    LOOM_TOOLS.iter().copied().filter(|t| !on_path(path, t)).collect()
}

/// The programs the answers file of the build's bindings (`bindings/tools/answers.yaml`) names by a
/// bare word (`requests`, `apply`, `then`) that the unit would not find: not in a directory of
/// `path` and not a binary the build will make (`driver/src/bin/<name>.rs`). A word with a `/` is
/// a file of the bindings copy and is not looked up. No file, or one that does not read, names none
/// (the runner refuses an unreadable file itself).
fn missing_answer_programs(build: &Path, path: &str) -> Vec<String> {
    let Some(doc) = fs::read_to_string(build.join("bindings/tools/answers.yaml")).ok().and_then(|t| serde_yaml::from_str::<Y>(&t).ok()) else {
        return Vec::new();
    };
    let mut names: BTreeSet<String> = BTreeSet::new();
    for entry in doc.get("answers").and_then(Y::as_sequence).into_iter().flatten() {
        for key in ["requests", "apply", "then"] {
            if let Some(word) = entry.get(key).and_then(Y::as_sequence).and_then(|w| w.first()).and_then(Y::as_str) {
                if !word.contains('/') {
                    names.insert(word.to_string());
                }
            }
        }
    }
    names.into_iter().filter(|n| !on_path(path, n) && !build.join("driver/src/bin").join(format!("{n}.rs")).is_file()).collect()
}

/// `~/x` as the launcher's `HOME` and `/x`; any other path as it is.
fn expand_home(path: &str, base: &dyn Fn(&str) -> Option<String>) -> Result<String, String> {
    match path.strip_prefix("~/") {
        Some(rest) => base("HOME").filter(|h| !h.is_empty()).map(|h| format!("{}/{rest}", h.trim_end_matches('/')))
            .ok_or_else(|| format!("refused: {path} needs HOME, which is not set")),
        None => Ok(path.to_string()),
    }
}

/// Whether a `loom` variant runs Loom as a library (`loom.in_process: 'true'`).
fn loom_in_process(settings: &BTreeMap<String, String>) -> bool {
    settings.get("loom.in_process").is_some_and(|x| x == "true")
}

/// The settings file of a `loom` run (`runbook.loom-run-settings/1`, TOML, `docs/loom-run-settings.md`):
/// the record's settings that `runbook run` reads, each in the one place the file has for it (the
/// record's `loom.confinement` is the file's `confinement`, and so on) while the run records them
/// as `run-settings.jsonl` under the record's own names. `loom.bin` is not in it (an argument). The
/// login file is a path, only when the record names one; `redact` puts the setting's name in its
/// place for printing. A text is quoted and escaped by the TOML library and a number must be a whole
/// number of 64 bits: one that is not is refused, never written as another value.
fn loom_settings_toml(name: &str, settings: &BTreeMap<String, String>, redact: bool, base: &dyn Fn(&str) -> Option<String>) -> Result<String, String> {
    use toml::{Table, Value as T};
    let get = |k: &str| settings.get(k).cloned().unwrap_or_default();
    let text = |k: &str| T::String(get(k));
    let num = |k: &str| -> Result<T, String> {
        let raw = get(k);
        raw.parse::<u64>().ok().and_then(|n| i64::try_from(n).ok()).map(T::Integer)
            .ok_or_else(|| format!("refused: {k} is {raw:?}, not a whole number of 64 bits, so the settings file cannot hold it"))
    };
    let mut top = Table::new();
    top.insert("format".into(), T::String("runbook.loom-run-settings/1".into()));
    top.insert("run".into(), T::String(name.into()));
    top.insert("autonomy".into(), text("autonomy"));
    if let Some(x) = settings.get("adapter") {
        top.insert("adapter".into(), T::String(x.clone()));
    }
    // The money caps are written only when the record names them: none named is no limit.
    if settings.contains_key("max_llm_calls") {
        top.insert("max_llm_calls".into(), num("max_llm_calls")?);
    }
    top.insert("max_wall_seconds".into(), num("max_wall_seconds")?);
    if loom_in_process(settings) {
        top.insert("in_process".into(), T::Boolean(true));
    } else {
        top.insert("loom_sha256".into(), text("loom.sha256"));
    }
    top.insert("confinement".into(), text("loom.confinement"));
    if settings.contains_key("loom.cap_micro_usd") {
        top.insert("cap_micro_usd".into(), num("loom.cap_micro_usd")?);
    }
    if let Some(l) = settings.get("loom.login_file") {
        let v = if redact { "<loom.login_file>".to_string() } else { expand_home(l, base)? };
        top.insert("login_file".into(), T::String(v));
    }
    // The decider: `kind` as the record says it. A model decider also says that Loom's output holds
    // no claims (`without_claims`: the run makes no claim about claims it does not have). The
    // principal of its answers is not written: the runner derives it from the kind.
    let kind = get("decider.kind");
    let mut decider = Table::new();
    decider.insert("kind".into(), T::String(kind.clone()));
    if kind == "rule" {
        decider.insert("verdict".into(), text("decider.verdict"));
    } else {
        decider.insert("model".into(), text("decider.model"));
        decider.insert("without_claims".into(), T::Boolean(true));
        for k in ["escalate", "adapter"] {
            if let Some(x) = settings.get(&format!("decider.{k}")) {
                decider.insert(k.into(), T::String(x.clone()));
            }
        }
        if settings.contains_key("decider.max_decider_calls") {
            decider.insert("max_decider_calls".into(), num("decider.max_decider_calls")?);
        }
    }
    top.insert("decider".into(), T::Table(decider));
    if let Some(x) = settings.get("review.adapter") {
        top.insert("review".into(), T::Table(Table::from_iter([("adapter".to_string(), T::String(x.clone()))])));
    }
    let mut models = Table::new();
    for c in ["writer", "implement", "review", "other"] {
        models.insert(c.into(), T::Table(Table::from_iter([("model".to_string(), text(&format!("models.{c}.model")))])));
    }
    top.insert("models".into(), T::Table(models));
    toml::to_string(&top).map_err(|e| format!("refused: the settings file cannot be written: {}", e.to_string().lines().next().unwrap_or("").chars().take(160).collect::<String>()))
}

/// The idea file of a `loom` run: `text: <the input's text>`, the shape of `work/idea.yaml`.
fn loom_idea_yaml(input: &Y) -> String {
    let mut m = Mapping::new();
    m.insert(Y::String("text".into()), Y::String(ystr(input, "text").unwrap_or("").trim().to_string()));
    serde_yaml::to_string(&Y::Mapping(m)).unwrap_or_default()
}

/// The files the launcher writes for one `loom` run, outside the run's directory (which
/// `runbook run` creates and refuses to find).
fn loom_launch_dir(builds: &Path, id: &str, name: &str) -> PathBuf {
    builds.parent().unwrap_or(builds).join("launch").join(id).join(name)
}

fn loom_runbook_bin(build: &Path) -> PathBuf {
    build.join("runbook").join("target").join("debug").join("runbook")
}

/// The command line of a `loom` run: `runbook run` of the build, its paths under the build's
/// tree and the spend ledger shared by every run of the record.
#[allow(clippy::too_many_arguments)]
fn loom_argv(build: &Path, id: &str, name: &str, settings: &BTreeMap<String, String>, runs_dir: &Path, root: &str, files: &Path,
             input: &Y, base: &dyn Fn(&str) -> Option<String>) -> Result<Vec<String>, String> {
    let g = |k: &str| settings.get(k).cloned().unwrap_or_default();
    let runbook = loom_runbook(input);
    let mut v: Vec<String> = vec![loom_runbook_bin(build).display().to_string(), "run".into(), runbook.into()];
    // the library is no option: without `--loom` Loom is a library of the build; the binary is
    // `--loom` with its digest
    let in_process = loom_in_process(settings);
    let mut add =|flag: &str, value: String| {
        v.push(flag.into());
        v.push(value);
    };
    add("--run", name.into());
    add("--runs-dir", runs_dir.display().to_string());
    if !in_process {
        add("--loom", expand_home(&g("loom.bin"), base)?);
        add("--loom-sha256", g("loom.sha256"));
    }
    add("--bindings", build.join("bindings").display().to_string());
    add("--protocols", build.join("protocol").display().to_string());
    add("--root-protocol", root.into());
    add("--case", if runbook == "sdc-bugfix" { "BUG-1" } else { "IDEA-1" }.into());
    if runbook == "sdc-bugfix" {
        add("--bugfix", build.join(ystr(input, "seed").unwrap_or_default()).display().to_string());
    }
    add("--intake", files.join("idea.yaml").display().to_string());
    add("--confinement", g("loom.confinement"));
    add("--settings", files.join("settings.toml").display().to_string());
    // A money cap is passed only when the record names it, and the ledger the runs share with it:
    // a record that names none starts a run with no money limit.
    let (cap, calls) = (settings.get("loom.cap_micro_usd"), settings.get("max_llm_calls"));
    if cap.is_some() || calls.is_some() {
        add("--spend-ledger", runs_dir.join(format!("{id}.loom-spend.json")).display().to_string());
    }
    if let Some(cap) = cap {
        add("--cap-micro-usd", cap.clone());
    }
    if let Some(calls) = calls {
        add("--max-llm-calls", calls.clone());
    }
    add("--max-wall-seconds", g("max_wall_seconds"));
    if let Some(c) = settings.get("conformance") {
        add("--conformance", c.clone());
    }
    if settings.get("flags.dry").is_some_and(|x| x == "true") {
        v.push("--dry".into());
    }
    let mut replay: Vec<String> = Vec::new();
    for (key, option) in REPLAY_FLAGS {
        if let Some(value) = settings.get(key) {
            // The recorded run is named by a path, or by its name under the runs directory.
            let value = if key == "flags.recorded" && !Path::new(value).is_absolute() { runs_dir.join(value).display().to_string() } else { value.clone() };
            replay.push(option.into());
            replay.push(value);
        }
    }
    v.extend(replay);
    Ok(v)
}

/// The unit's description: the record and the run, never a setting.
fn unit_description(id: &str, run: &str) -> String {
    format!("sdc experiment {id}: run {run}")
}

/// A file of the tree at `commit`, read from its build directory when there is one, else from the
/// repository of the working directory. `None`: the tree has no such file.
fn tree_file(build: &Path, commit: &str, rel: &str) -> Result<Option<String>, String> {
    if build.is_dir() {
        return match fs::read_to_string(build.join(rel)) {
            Ok(t) => Ok(Some(t)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("cannot read {}: {e}", build.join(rel).display())),
        };
    }
    let o = Command::new("git").args(["-C", ".", "show", &format!("{commit}:{rel}")]).env("LC_ALL", "C").output().map_err(|e| format!("git: {e}"))?;
    if o.status.success() {
        return Ok(Some(String::from_utf8_lossy(&o.stdout).into_owned()));
    }
    let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
    if err.contains("does not exist in") || err.contains("exists on disk, but not in") {
        return Ok(None);
    }
    Err(format!("refused: cannot read {rel} of tree_commit {commit}: {err}"))
}

/// The file of the `runbook` crate that makes `runbook run` run in process; a tree without it has
/// only the subcommand `loom-run`, which the launcher no longer starts.
const IN_PROCESS_RUN: &str = "runbook/src/loom_run/here.rs";

/// What a `loom` launch reads from the record's tree: that it has the `runbook/` crate and the
/// root protocol, `<id>@<revision>` of `protocol/<runbook>.protocol.yaml` (`sdc`, or `sdc-bugfix` for a seed input), as `runbook run` is told
/// to register it (a `.` in an id is a `-`, as Loom names a registration).
fn loom_root_protocol(build: &Path, commit: &str, runbook: &str) -> Result<String, String> {
    if tree_file(build, commit, "runbook/Cargo.toml")?.is_none() {
        return Err(format!("refused: tree_commit {commit} has no runbook/ crate (runbook/Cargo.toml): a loom run needs `runbook run` of that commit"));
    }
    if tree_file(build, commit, IN_PROCESS_RUN)?.is_none() {
        return Err(format!("refused: tree_commit {commit} has a runbook/ crate with no in-process `runbook run` ({IN_PROCESS_RUN} is missing): a loom run needs `runbook run` of that commit, an older build has only `runbook loom-run`"));
    }
    let text = tree_file(build, commit, &format!("protocol/{runbook}.protocol.yaml"))?
        .ok_or_else(|| format!("refused: tree_commit {commit} has no protocol/{runbook}.protocol.yaml, the root protocol of a run"))?;
    let y: Y = serde_yaml::from_str(&text).map_err(|e| format!("refused: protocol/{runbook}.protocol.yaml of {commit}: {e}"))?;
    let p = y.get("protocol").cloned().unwrap_or(Y::Null);
    match (ystr(&p, "id"), p.get("revision").and_then(Y::as_u64)) {
        (Some(id), Some(rev)) => Ok(format!("{}@{rev}", id.replace('.', "-"))),
        _ => Err(format!("refused: protocol/{runbook}.protocol.yaml of {commit} has no protocol.id and protocol.revision")),
    }
}

/// The command line of a free-running agent's run (`runner: free-agent`): sdc-free-agent of the
/// build, the idea text, the prompt and example spec of the build's tree, the record's settings.
fn free_argv(build: &Path, name: &str, input: &Y, settings: &BTreeMap<String, String>, runs_dir: &Path) -> Vec<String> {
    let g = |k: &str| settings.get(k).cloned().unwrap_or_default();
    let mut v: Vec<String> = vec![build.join("driver/target/debug/sdc-free-agent").display().to_string(), "run".into(),
        "--run-dir".into(), runs_dir.join(name).display().to_string(),
        "--idea".into(), ystr(input, "text").unwrap_or("").trim().to_string(),
        "--prompt".into(), build.join("driver/examples/sdc/free-agent/prompt.md").display().to_string(),
        "--example-spec".into(), build.join("driver/examples/sdc/fixtures/billing-spec").display().to_string()];
    for (flag, key) in [("--model", "free.model"), ("--effort", "free.effort"), ("--max-usd", "free.max_usd"),
        ("--max-wall-s", "free.max_wall_s"), ("--max-turns", "free.max_turns")] {
        // The money cap is optional: `--max-usd` is passed only when the record names it.
        if key == "free.max_usd" && !settings.contains_key(key) {
            continue;
        }
        v.push(flag.into());
        v.push(g(key));
    }
    v
}

/// The message of a refusal to start a run of the old driver (a variant with `runner: sdc` or with
/// no `runner`): one line, naming the setting that starts a run today.
fn old_driver_refusal(variant: &str) -> String {
    format!("refused: variant {variant:?} has runner sdc or no runner: that is a record of the old driver, which is gone; a run starts with `runner: loom` (`runbook run`)")
}

/// `experiment.json` of one launched run (sdc.experiment-run/1).
fn experiment_json(id: &str, p: &Planned, commit: &str, input_sha: &str, started_at: &str) -> J {
    json!({"format": "sdc.experiment-run/1", "experiment": id, "variant": p.variant.id, "ordinal": p.ordinal, "run": p.name,
        "tree_commit": commit, "input_sha256": input_sha,
        "settings_sha256": sha256_hex(&serde_json::to_string(&p.variant.settings).unwrap_or_default()),
        "started_at": started_at, "retrospective": false})
}

/// RFC 3339 UTC of whole seconds since the epoch.
fn rfc3339(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Civil date from days since 1970-01-01 (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    rfc3339(secs)
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let o = Command::new("git").arg("-C").arg(dir).args(args).output().map_err(|e| format!("git: {e}"))?;
    if !o.status.success() {
        return Err(format!("git {} in {}: {}", args.join(" "), dir.display(), String::from_utf8_lossy(&o.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn default_builds_dir() -> PathBuf {
    let state = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("state")))
        .unwrap_or_else(|| PathBuf::from(".state"));
    state.join("sdc-experiment").join("builds")
}

fn absolute(p: &Path) -> PathBuf {
    if p.is_absolute() { p.to_path_buf() } else { std::env::current_dir().map(|c| c.join(p)).unwrap_or_else(|_| p.to_path_buf()) }
}

/// The crates a build builds, each a cargo job of its own (the pool kills a job over 10 minutes),
/// with its cargo arguments; a `loom` record also needs the `runbook` crate, whose binary is
/// `runbook run`.
fn build_steps(loom: bool) -> Vec<(&'static str, &'static [&'static str])> {
    let mut v: Vec<(&'static str, &'static [&'static str])> =
        vec![("driver", &["build", "--bins"][..]), ("goals", &["build"][..]), ("watch", &["build", "--release"][..])];
    if loom {
        v.push(("runbook", &["build", "--locked"][..]));
    }
    v
}

/// Whether a run directory under `runs_dir` was launched on `commit`.
fn runs_on_build(runs_dir: &Path, commit: &str) -> bool {
    fs::read_dir(runs_dir).into_iter().flatten().flatten().any(|e| {
        fs::read_to_string(e.path().join("experiment.json")).ok()
            .and_then(|t| serde_json::from_str::<J>(&t).ok())
            .is_some_and(|j| j["tree_commit"] == commit)
    })
}

/// The build directory of `commit`: a shared clone checked out at the commit, built once. A
/// directory at another commit or with changes is refused; a built one is never rebuilt, and an
/// unbuilt one that runs already used is refused rather than built in place.
fn prepare_build(repo: &Path, builds: &Path, commit: &str, runs_dir: &Path, loom: bool) -> Result<PathBuf, String> {
    let dir = builds.join(commit);
    let marker = builds.join(format!("{commit}.built.json"));
    if !dir.exists() {
        fs::create_dir_all(builds).map_err(|e| format!("cannot create {}: {e}", builds.display()))?;
        let common = absolute(&repo.join(git(repo, &["rev-parse", "--git-common-dir"])?));
        let o = Command::new("git").args(["clone", "--quiet", "--shared", "--no-checkout"]).arg(&common).arg(&dir).output()
            .map_err(|e| format!("git clone: {e}"))?;
        if !o.status.success() {
            return Err(format!("git clone into {}: {}", dir.display(), String::from_utf8_lossy(&o.stderr).trim()));
        }
        git(&dir, &["checkout", "--quiet", "--detach", commit])?;
    }
    let head = git(&dir, &["rev-parse", "HEAD"])?;
    if head != commit {
        return Err(format!("refused: the build directory {} is at {head}, the record says {commit}", dir.display()));
    }
    let dirty = git(&dir, &["status", "--porcelain"])?;
    if !dirty.is_empty() {
        return Err(format!("refused: the build directory {} has changes:\n{dirty}", dir.display()));
    }
    // A build record from before the first `loom` record lacks the `runbook` crate: that crate is
    // built beside the others (its own target directory, the others' binaries untouched).
    let record: Option<J> = fs::read_to_string(&marker).ok().and_then(|t| serde_json::from_str(&t).ok());
    let mut built: Vec<String> = record.as_ref().and_then(|r| r["crates"].as_array())
        .map(|a| a.iter().filter_map(|c| c.as_str().map(String::from)).collect()).unwrap_or_default();
    if marker.is_file() {
        if !loom || built.iter().any(|c| c == "runbook") {
            return Ok(dir);
        }
    } else if runs_on_build(runs_dir, commit) {
        return Err(format!("refused: runs exist on {commit} but its build directory has no build record; it is not rebuilt in place"));
    }
    for (krate, args) in build_steps(loom) {
        let manifest = dir.join(krate).join("Cargo.toml");
        if !manifest.is_file() || built.iter().any(|c| c == krate) {
            continue;
        }
        println!("building {krate} at {commit} ...");
        let st = Command::new("cargo").args(args).arg("--manifest-path").arg(&manifest).status().map_err(|e| format!("cargo: {e}"))?;
        if !st.success() {
            return Err(format!("cargo {} for {krate} at {commit} failed ({st})", args.join(" ")));
        }
        built.push(krate.to_string());
    }
    let built_at = record.as_ref().and_then(|r| r["built_at"].as_str().map(String::from)).unwrap_or_else(now_rfc3339);
    let doc = json!({"format": "sdc.experiment-build/1", "commit": commit, "built_at": built_at, "crates": built});
    fs::write(&marker, serde_json::to_string_pretty(&doc).unwrap_or_default() + "\n").map_err(|e| format!("cannot write {}: {e}", marker.display()))?;
    Ok(dir)
}

fn unit_state(unit: &str) -> String {
    Command::new("systemctl").args(["--user", "show", "-p", "ActiveState", "--value", unit]).output().ok()
        .filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
}

fn add_run_to_record(r: &mut Y, variant: &str, name: &str) {
    let Some(vs) = r.get_mut("variants").and_then(Y::as_sequence_mut) else { return };
    for v in vs.iter_mut() {
        if ystr(v, "id") != Some(variant) {
            continue;
        }
        if let Some(runs) = v.get_mut("runs").and_then(Y::as_sequence_mut) {
            if !runs.iter().any(|x| x.as_str() == Some(name) || ystr(x, "run") == Some(name)) {
                runs.push(Y::String(name.into()));
            }
        }
    }
}

fn sleep_s(s: u64) {
    std::thread::sleep(std::time::Duration::from_secs(s));
}

struct LaunchArgs {
    rec: String,
    runs_dir: PathBuf,
    builds: PathBuf,
    max_parallel: Option<usize>,
    stagger: Option<u64>,
    print: bool,
    /// The login the model adapter reads, named when the runs are started and in no record:
    /// `loom.login_file` of every `loom` variant for this launch only.
    login_file: Option<String>,
}

fn run_cmd(args: &[String]) -> Result<ExitCode, Usage> {
    let mut a = LaunchArgs { rec: String::new(), runs_dir: PathBuf::from("runs"), builds: default_builds_dir(), max_parallel: None, stagger: None, print: false, login_file: None };
    let mut it = args.iter();
    while let Some(x) = it.next() {
        let mut value = || it.next().cloned().ok_or_else(|| Usage(format!("{x} needs a value")));
        match x.as_str() {
            "--runs-dir" => a.runs_dir = PathBuf::from(value()?),
            "--builds-dir" => a.builds = PathBuf::from(value()?),
            "--max-parallel" => a.max_parallel = Some(value()?.parse().ok().filter(|n| *n > 0).ok_or_else(|| Usage("--max-parallel needs a number of at least 1".into()))?),
            "--stagger" => a.stagger = Some(value()?.parse().map_err(|_| Usage("--stagger needs seconds".into()))?),
            "--print" => a.print = true,
            "--login-file" => {
                let l = value()?;
                if !(l.starts_with('/') || l.starts_with("~/")) {
                    return Err(Usage("--login-file needs an absolute path or one that starts with ~/".into()));
                }
                a.login_file = Some(l);
            }
            y if y.starts_with("--") => return Err(Usage(format!("unknown argument {y}"))),
            y => a.rec = y.to_string(),
        }
    }
    if a.rec.is_empty() {
        return Err(Usage("run needs a record".into()));
    }
    Ok(match launch(&a) {
        Ok(()) => ExitCode::SUCCESS,
        Err((code, m)) => {
            eprintln!("sdc-experiment: {m}");
            ExitCode::from(code)
        }
    })
}

/// What one run is started with: the unit's whole environment, the command and, for a `loom`
/// run, the files written before it starts.
struct RunPlan {
    env: Vec<(String, String)>,
    argv: Vec<String>,
    loom: Option<LoomFiles>,
}

struct LoomFiles {
    dir: PathBuf,
    idea: String,
    settings: String,
}

fn launch(a: &LaunchArgs) -> Result<(), (u8, String)> {
    let path = record_path(&a.rec);
    let (mut r, raw) = read_record(&path).map_err(|m| (2, m))?;
    if ystr(&r, "kind") == Some("prospective") {
        if let Some(p) = planned_runs(&r).iter().find(|p| p.variant.runner == "sdc") {
            return Err((1, old_driver_refusal(&p.variant.id)));
        }
    }
    let problems = check_record(&r, &path, &raw);
    if !problems.is_empty() {
        return Err((1, format!("refused: {} fails its check:\n{}", path.display(), problems.join("\n"))));
    }
    if ystr(&r, "kind") != Some("prospective") {
        return Err((1, "refused: only a prospective record is launched; a retrospective record lists runs that exist".into()));
    }
    if !matches!(ystr(&r, "status"), Some("planned" | "running")) {
        return Err((1, format!("refused: the record is {}", ystr(&r, "status").unwrap_or(""))));
    }
    let commit = ystr(&r, "tree_commit").filter(|c| is_hex(c, 40, 40)).ok_or((1, "refused: the record pins no tree_commit".to_string()))?.to_string();
    let input = yseq(&r, "inputs").first().cloned().unwrap_or(Y::Null);
    let input_sha = ystr(&input, "sha256").unwrap_or("").to_string();
    let id = ystr(&r, "id").unwrap_or("").to_string();
    let runs_dir = absolute(&a.runs_dir);
    let builds = absolute(&a.builds);
    let build = builds.join(&commit);
    let launch_cfg = r.get("launch").cloned().unwrap_or(Y::Null);
    let max_parallel = a.max_parallel.or_else(|| launch_cfg.get("max_parallel").and_then(Y::as_u64).map(|n| n as usize)).unwrap_or(3);
    let stagger = a.stagger.or_else(|| launch_cfg.get("stagger_s").and_then(Y::as_u64)).unwrap_or(30);
    let principal = principal_of(&r);
    let plan = planned_runs(&r);
    let base = |k: &str| std::env::var(k).ok();
    let any_loom = plan.iter().any(|p| p.variant.runner == "loom");
    // A `loom` record's tree must hold the runbook crate; its root protocol is read from the tree.
    let loom_root = if any_loom { Some(loom_root_protocol(&build, &commit, loom_runbook(&input)).map_err(|m| (1, m))?) } else { None };
    let run_plan = |p: &Planned, redact: bool| -> Result<RunPlan, String> {
        let s = &p.variant.settings;
        Ok(match p.variant.runner.as_str() {
            "loom" => {
                let mut s = s.clone();
                if let Some(login) = &a.login_file {
                    s.insert("loom.login_file".to_string(), login.clone());
                }
                let s = &s;
                let files = loom_launch_dir(&builds, &id, &p.name);
                let argv = loom_argv(&build, &id, &p.name, s, &runs_dir, loom_root.as_deref().unwrap_or_default(), &files, &input, &base)?;
                let settings = loom_settings_toml(&p.name, s, redact, &base)?;
                RunPlan { env: loom_env(&base, &build), argv, loom: Some(LoomFiles { dir: files, idea: loom_idea_yaml(&input), settings }) }
            }
            "free-agent" => RunPlan { env: launch_env(s, &runs_dir, &principal, &base), argv: free_argv(&build, &p.name, &input, s, &runs_dir), loom: None },
            _ => return Err(old_driver_refusal(&p.variant.id)),
        })
    };
    if a.print {
        println!("experiment {id}: {} runs, tree {commit}, build {}, at most {max_parallel} at once, {stagger} s apart", plan.len(), build.display());
        println!("build: {}", build_steps(any_loom).iter().map(|(k, args)| format!("{k} (cargo {})", args.join(" "))).collect::<Vec<_>>().join(", "));
        for p in &plan {
            let rp = run_plan(p, true).map_err(|m| (1, m))?;
            println!("{} ({}):", p.name, p.unit);
            match &rp.loom {
                Some(l) => {
                    println!("  runner: loom");
                    if loom_in_process(&p.variant.settings) {
                        println!("  loom: in process (--in-process: Loom as a library of the build, no b10x-loom binary, no loom.bin, no loom.sha256)");
                    }
                    println!("  description: {}", unit_description(&id, &p.name));
                    for (k, v) in &rp.env {
                        println!("  env {k}={v}");
                    }
                    println!("  needs on PATH: {} (the shell programs of the bindings); sdc-reply, sdc-findings and sdc-bugfix from {}",
                        LOOM_TOOLS.join(", "), build.join("driver/target/debug").display());
                    let missing = missing_loom_tools(rp.env.iter().find(|(k, _)| k == "PATH").map_or("", |(_, v)| v));
                    if !missing.is_empty() {
                        println!("  missing on PATH: {}", missing.join(", "));
                    }
                    if p.variant.settings.contains_key("loom.login_file") || a.login_file.is_some() {
                        println!("  login: loom.login_file is set (named in the settings file; the launcher never reads it)");
                    }
                    println!("  idea file: {}", l.dir.join("idea.yaml").display());
                    println!("  settings file: {}", l.dir.join("settings.toml").display());
                    println!("  settings:");
                    for line in l.settings.lines() {
                        if line.is_empty() {
                            println!();
                        } else {
                            println!("    {line}");
                        }
                    }
                }
                None => {
                    for (k, v) in rp.env.iter().filter(|(k, _)| k.starts_with("SDC_")) {
                        println!("  {k}={v}");
                    }
                }
            }
            println!("  {}", rp.argv.join(" "));
        }
        return Ok(());
    }
    if any_loom {
        let path = base("PATH").unwrap_or_default();
        let missing = missing_loom_tools(&path);
        if !missing.is_empty() {
            return Err((1, format!("refused: {} not on PATH: the shell programs of the bindings of a loom run need {} and exit 127 without them", missing.join(", "), LOOM_TOOLS.join(", "))));
        }
        let unit_path = format!("{}:{path}", build.join("driver/target/debug").display());
        let missing = missing_answer_programs(&build, &unit_path);
        if !missing.is_empty() {
            return Err((1, format!("refused: {} not on PATH: `bindings/tools/answers.yaml` of a loom run needs {} and the run stops at the first question or finding without them", missing.join(", "), missing.join(", "))));
        }
    }
    let repo = PathBuf::from(git(Path::new("."), &["rev-parse", "--show-toplevel"]).map_err(|m| (1, m))?);
    let build = prepare_build(&repo, &builds, &commit, &runs_dir, any_loom).map_err(|m| (1, m))?;
    if any_loom && !loom_runbook_bin(&build).is_file() {
        return Err((1, format!("refused: {} was not built: a loom run starts `runbook run`", loom_runbook_bin(&build).display())));
    }
    fs::create_dir_all(&runs_dir).map_err(|e| (1, format!("cannot create {}: {e}", runs_dir.display())))?;
    let mut started = 0;
    for p in &plan {
        let dir = runs_dir.join(&p.name);
        if dir.join("experiment.json").is_file() {
            println!("{}: started before, skipped", p.name);
            continue;
        }
        if dir.exists() {
            return Err((1, format!("refused: {} exists without experiment.json", dir.display())));
        }
        while plan.iter().filter(|q| matches!(unit_state(&q.unit).as_str(), "active" | "activating" | "deactivating")).count() >= max_parallel {
            sleep_s(30);
        }
        if started > 0 {
            sleep_s(stagger);
        }
        let _ = Command::new("systemctl").args(["--user", "reset-failed", &p.unit]).output();
        let RunPlan { env, argv, loom } = run_plan(p, false).map_err(|m| (1, m))?;
        let free = p.variant.runner == "free-agent";
        // The files of a `loom` run go outside its directory, which `runbook run` creates.
        if let Some(l) = &loom {
            let write = |name: &str, text: String| fs::create_dir_all(&l.dir).and_then(|()| fs::write(l.dir.join(name), text))
                .map_err(|e| (1, format!("cannot write {}: {e}", l.dir.join(name).display())));
            write("idea.yaml", l.idea.clone())?;
            write("settings.toml", l.settings.clone())?;
        }
        // sdc-free-agent writes free-agent.json; `runbook run` makes the run directory itself.
        let marker = if free { dir.join("free-agent.json") } else { dir.clone() };
        // A free run's home holds a copy of the login: the unit removes it on every stop, also when
        // the launcher is killed or crashes and runs no destructor.
        let stop_post: Vec<String> =
            if free { vec![format!("--property=ExecStopPost=/usr/bin/rm -rf -- {}", dir.join("home").display())] } else { vec![] };
        let st = Command::new("systemd-run")
            .args(["--user", "--quiet", "--unit", &p.unit])
            .args(&stop_post)
            .arg("--description")
            .arg(unit_description(&id, &p.name))
            .arg(format!("--working-directory={}", build.display()))
            .args(["/usr/bin/env", "-i"])
            .args(env.iter().map(|(k, v)| format!("{k}={v}")))
            .args(&argv)
            .status()
            .map_err(|e| (1, format!("systemd-run: {e}")))?;
        if !st.success() {
            return Err((1, format!("systemd-run for {} failed ({st})", p.name)));
        }
        let started_at = now_rfc3339();
        // The record goes in once the run has made its directory.
        let mut waited = 0;
        while !marker.exists() && waited < 120 {
            sleep_s(1);
            waited += 1;
        }
        if !marker.exists() {
            let _ = Command::new("systemctl").args(["--user", "stop", &p.unit]).output();
            return Err((1, format!("{} did not create {} within 120 s; its unit {} was stopped", p.name, dir.display(), p.unit)));
        }
        let doc = experiment_json(&id, p, &commit, &input_sha, &started_at);
        fs::write(dir.join("experiment.json"), serde_json::to_string_pretty(&doc).unwrap_or_default() + "\n")
            .map_err(|e| (1, format!("cannot write experiment.json for {}: {e}", p.name)))?;
        add_run_to_record(&mut r, &p.variant.id, &p.name);
        set_key(&mut r, "status", Y::String("running".into()), "created");
        write_record(&path, &r).map_err(|m| (1, m))?;
        started += 1;
        println!("started {} as {} at {started_at}", p.name, p.unit);
    }
    println!("experiment {id}: {started} runs started, {} planned; sdc-experiment status {id} lists them", plan.len());
    Ok(())
}

fn status_cmd(args: &[String]) -> Result<ExitCode, Usage> {
    let (mut rec, mut runs_dir) = (None::<String>, PathBuf::from("runs"));
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--runs-dir" => runs_dir = PathBuf::from(it.next().ok_or_else(|| Usage("--runs-dir needs a directory".into()))?),
            x if x.starts_with("--") => return Err(Usage(format!("unknown argument {x}"))),
            x => rec = Some(x.to_string()),
        }
    }
    let path = record_path(&rec.ok_or_else(|| Usage("status needs a record".into()))?);
    let exp = experiments_dir_of(&path);
    let (r, _) = match read_record(&path) {
        Ok(x) => x,
        Err(m) => {
            eprintln!("sdc-experiment: {m}");
            return Ok(ExitCode::from(2));
        }
    };
    let id = ystr(&r, "id").unwrap_or("").to_string();
    let mut rows: Vec<(String, String, String)> = variants(&r).iter()
        .flat_map(|v| v.runs.iter().map(|x| (x.run.clone(), v.id.clone(), String::new())).collect::<Vec<_>>()).collect();
    if ystr(&r, "kind") == Some("prospective") {
        for p in planned_runs(&r) {
            match rows.iter_mut().find(|x| x.0 == p.name) {
                Some(x) => x.2 = p.unit.clone(),
                None => rows.push((p.name.clone(), p.variant.id.clone(), p.unit.clone())),
            }
        }
    }
    println!("{id} ({}, {}):", ystr(&r, "kind").unwrap_or(""), ystr(&r, "status").unwrap_or(""));
    println!("{:<34} {:<10} {:<12} {:>9} {:>7}  end", "run", "variant", "unit", "cost USD", "wall s");
    for (run, variant, unit) in rows {
        let dir = runs_dir.join(&run);
        // A directory the runner has just made holds nothing to read yet (not started). One that
        // holds `outcome.json` (a `runbook run` that ended) and cannot be read is said so.
        let mut unreadable = None;
        let doc = if dir.is_dir() {
            match run_metrics::load(&dir) {
                Ok(d) => Some(d),
                Err(e) => {
                    if dir.join("outcome.json").is_file() {
                        unreadable = Some(format!("unreadable: {}", e.0));
                    }
                    None
                }
            }
        } else {
            load_summary(&summaries_dir(&exp, &id).join(format!("{run}.json"))).ok().flatten().map(|x| x.doc)
        };
        let state = if unit.is_empty() { "-".to_string() } else { Some(unit_state(&unit)).filter(|s| !s.is_empty()).unwrap_or_else(|| "none".into()) };
        let (cost, wall, end) = match &doc {
            Some(d) => (d["metrics"]["cost_usd"].as_f64().map_or("-".into(), |x| format!("{x:.3}")),
                d["metrics"]["wall_s"].as_i64().map_or("-".into(), |x| x.to_string()), d["metrics"]["end"].as_str().unwrap_or("").to_string()),
            None => ("-".into(), "-".into(), unreadable.unwrap_or_else(|| "not started".into())),
        };
        println!("{run:<34} {variant:<10} {state:<12} {cost:>9} {wall:>7}  {end}");
    }
    Ok(ExitCode::SUCCESS)
}

fn stop_cmd(args: &[String]) -> Result<ExitCode, Usage> {
    let (mut rec, mut runs_dir, mut reason, mut only) = (None::<String>, PathBuf::from("runs"), None::<String>, None::<String>);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().cloned().ok_or_else(|| Usage(format!("{a} needs a value")));
        match a.as_str() {
            "--runs-dir" => runs_dir = PathBuf::from(value()?),
            "--reason" => reason = Some(value()?),
            "--run" => only = Some(value()?),
            x if x.starts_with("--") => return Err(Usage(format!("unknown argument {x}"))),
            x => rec = Some(x.to_string()),
        }
    }
    let reason = reason.filter(|r| !r.trim().is_empty()).ok_or_else(|| Usage("stop needs --reason".into()))?;
    let path = record_path(&rec.ok_or_else(|| Usage("stop needs a record".into()))?);
    let (mut r, _) = match read_record(&path) {
        Ok(x) => x,
        Err(m) => {
            eprintln!("sdc-experiment: {m}");
            return Ok(ExitCode::from(2));
        }
    };
    let runs_dir = absolute(&runs_dir);
    let principal = principal_of(&r);
    let mut code = ExitCode::SUCCESS;
    let units: BTreeMap<String, String> = planned_runs(&r).into_iter().map(|p| (p.name, p.unit)).collect();
    for v in variants(&r) {
        for rr in v.runs.iter().filter(|x| only.as_ref().is_none_or(|o| o == &x.run)) {
            let dir = runs_dir.join(&rr.run);
            // The one path: the stop record first, so that a run the signal cannot reach still
            // reads as stopped; then the unit below, which the launcher started the run in:
            // systemd sends SIGTERM to the run's process. Never a process name.
            let record = dir.join("run-stopped.json");
            if record.is_file() {
                println!("{}: already stopped ({} untouched)", rr.run, record.display());
            } else if dir.is_dir() && !run_metrics::load(&dir).is_ok_and(|d| d["metrics"]["finished"] == true) {
                let doc = json!({"format": "sdc.run-stopped/1", "at": now_rfc3339(), "by": principal, "reason": reason, "verdict_at_stop": null, "case": null});
                match fs::write(&record, serde_json::to_string_pretty(&doc).unwrap_or_default() + "\n") {
                    Ok(()) => println!("{}: stopped ({reason})", rr.run),
                    Err(e) => {
                        eprintln!("sdc-experiment: cannot write {}: {e}", record.display());
                        code = ExitCode::from(1);
                    }
                }
            } else {
                println!("{}: not open, nothing to stop", rr.run);
            }
            if let Some(u) = units.get(&rr.run).filter(|u| matches!(unit_state(u).as_str(), "active" | "activating")) {
                let _ = Command::new("systemctl").args(["--user", "stop", u]).status();
                println!("{}: unit {u} stopped", rr.run);
            }
        }
    }
    if only.is_none() && ystr(&r, "status") != Some("complete") {
        set_key(&mut r, "status", Y::String("abandoned".into()), "created");
        let mut m = Mapping::new();
        m.insert(Y::String("at".into()), Y::String(now_rfc3339()));
        m.insert(Y::String("by".into()), Y::String(principal));
        m.insert(Y::String("reason".into()), Y::String(reason));
        set_key(&mut r, "stopped", Y::Mapping(m), "result");
        if let Err(m) = write_record(&path, &r) {
            eprintln!("sdc-experiment: {m}");
            code = ExitCode::from(1);
        } else {
            println!("{}: status abandoned", path.display());
        }
    }
    Ok(code)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let res = match args.first().map(String::as_str) {
        Some("compare") => compare(&args[1..]),
        Some("check") => check(&args[1..]),
        Some("report") => report(&args[1..]),
        Some("index") => index(&args[1..]),
        Some("run") => run_cmd(&args[1..]),
        Some("status") => status_cmd(&args[1..]),
        Some("stop") => stop_cmd(&args[1..]),
        Some("progress") => progress_cmd(&args[1..]),
        _ => Err(Usage("a subcommand: compare, check, report, index, progress, run, status or stop".into())),
    };
    match res {
        Ok(c) => c,
        Err(Usage(m)) => {
            eprintln!("sdc-experiment: {m}\n{USAGE}");
            ExitCode::from(64)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDEA: &str = "A to-do list web app for me and my partner that also works on the train without a connection";

    fn prospective() -> String {
        format!(r#"
format: sdc.experiment/1
id: x
kind: prospective
status: planned
created: 2026-11-21
title: Effort low against high on the implement class
purpose: Whether more effort on change.implement lowers its looping factor.
hypothesis: With high effort the looping factor of story/change.implement falls by 0.5.
inputs:
  - {{name: todo, kind: idea, text: "{IDEA}", sha256: {}}}
tree_commit: 8db4b9338f1d8d32c85b9cb2e014dac56980fac0
target: {{kind: node, name: story/change.implement}}
variable: efforts.implement.effort
fixed:
  models.writer.model: claude-sonnet-5-5
  models.implement.model: claude-sonnet-5-5
  models.review.model: claude-sonnet-5-5
  models.other.model: claude-sonnet-5-5
  decider.kind: haiku
  decider.model: claude-opus-5-5
  decider.escalate: final
  decider.max_decider_calls: 40
  decider.adapter: claude-cli
  adapter: metaharness
  max_llm_calls: 150
  autonomy: A5
variants:
  - {{id: low, settings: {{efforts.implement.effort: low}}, runs: []}}
  - {{id: high, settings: {{efforts.implement.effort: high}}, runs: []}}
baseline: low
treatment: high
runs_per_variant: 2
pass_rule: clean-delivery
primary: {{metric: node.story/change.implement.looping_factor, direction: decrease, min_effect: 0.5}}
guards: [{{metric: run.delivered, worse: decrease, max_worsening: 0}}]
decision_rule: {{rule: complete-separation, text: t}}
adoption: {{decision: pending, commit: null}}
"#, sha256_hex(IDEA))
    }

    fn targets_dir(name: &str, targets: &str) -> PathBuf {
        let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("test-tmp").join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("baseline-0.1.0.yaml"), "format: sdc.experiment/1\n").unwrap();
        fs::write(d.join(TARGETS), targets).unwrap();
        d
    }

    const TARGETS_OK: &str = "format: sdc.targets/1
targets:
- id: cost
  set: 2026-11-21
  by: human:operator
  said: so, this is our target then 10% costs
  metric: run.cost_usd
  target: 0.47
  basis: median of three free runs
  source_experiment: free-agent-0.1.0
  product_check: the oracle once runs write one
  per_setting:
  - {setting: mixed, reference: baseline-0.1.0, now: 2.55, target: 0.47}
  guards:
  - {metric: run.delivered, worse: decrease}
";

    #[test]
    fn targets_pass_and_name_what_is_wrong() {
        let d = targets_dir("ok", TARGETS_OK);
        assert_eq!(check_targets(&d), Vec::<String>::new());
        assert!(record_paths(&d).unwrap().iter().all(|p| p.file_name().is_some_and(|n| n != TARGETS)));
        let bad = TARGETS_OK.replace("metric: run.cost_usd", "metric: cost").replace("reference: baseline-0.1.0", "reference: nothing")
            .replace("worse: decrease", "worse: lower").replace("  said: so, this is our target then 10% costs\n", "");
        let d2 = targets_dir("bad", &bad);
        let p = check_targets(&d2).join("\n");
        for want in ["`said` is missing", "`metric` must be a metric name", "names no record", "a guard needs"] {
            assert!(p.contains(want), "{want} not in {p}");
        }
        let _ = fs::remove_dir_all(&d);
        let _ = fs::remove_dir_all(&d2);
    }

    #[test]
    fn separation_needs_disjoint_ranges() {
        let s = |v: &[f64]| stat(v).unwrap();
        assert!(!separation(&s(&[1.0, 2.0, 3.0]), &s(&[3.0, 4.0, 5.0])).0);
        let (sep, p) = separation(&s(&[1.0, 2.0, 3.0]), &s(&[4.0, 5.0, 6.0]));
        assert!(sep);
        assert_eq!(p, Some(0.1));
    }

    #[test]
    fn a_prospective_record_passes_and_must_change_one_variable() {
        let text = prospective();
        let y: Y = serde_yaml::from_str(&text).unwrap();
        let p = check_record(&y, Path::new("experiments/x.yaml"), &text);
        assert!(p.is_empty(), "{p:?}");
        let text = text.replace("{efforts.implement.effort: high}", "{efforts.implement.effort: high, decider.model: b}");
        let y: Y = serde_yaml::from_str(&text).unwrap();
        let p = check_record(&y, Path::new("experiments/x.yaml"), &text);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(p[0].contains("decider.model"));
    }

    #[test]
    fn a_record_is_refused_for_a_wrong_hash_a_short_commit_and_a_home_path() {
        let text = prospective().replace("8db4b9338f1d8d32c85b9cb2e014dac56980fac0", "8db4b93")
            .replace(&sha256_hex(IDEA), &sha256_hex("other")) + &format!("links: [/{}/someone/x]\n", "home");
        let y: Y = serde_yaml::from_str(&text).unwrap();
        let p = check_record(&y, Path::new("experiments/x.yaml"), &text).join("\n");
        assert!(p.contains("full 40-character commit"), "{p}");
        assert!(p.contains("`sha256` is not the sha256"), "{p}");
        assert!(p.contains("absolute home path"), "{p}");
    }

    #[test]
    fn the_launch_takes_its_environment_from_the_record_only() {
        let y: Y = serde_yaml::from_str(&prospective()).unwrap();
        let plan = planned_runs(&y);
        assert_eq!(plan.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["x.low.1", "x.high.1", "x.low.2", "x.high.2"]);
        assert_eq!(plan[1].unit, "sdc-exp-x-high-1");
        let base = |k: &str| match k { "HOME" => Some("/h".to_string()), "SDC_LLM_MODEL" => Some("leak".to_string()), _ => None };
        let env = launch_env(&plan[1].variant.settings, Path::new("/r"), &principal_of(&y), &base);
        let get = |k: &str| env.iter().find(|(x, _)| x == k).map(|(_, v)| v.as_str());
        assert_eq!(get("HOME"), Some("/h"));
        assert_eq!(get("SDC_LLM_MODEL"), None);
        assert_eq!(get("SDC_LLM_EFFORT_IMPLEMENT"), Some("high"));
        assert_eq!(get("SDC_LLM_MODEL_DECIDER"), Some("claude-opus-5-5"));
        assert_eq!(get("SDC_DECIDER_ADAPTER"), Some("claude-cli"));
        assert_eq!(get("SDC_MAX_DECIDER_CALLS"), Some("40"));
        assert_eq!(get("SDC_RUNS_DIR"), Some("/r"));
        assert_eq!(get("SDC_PRINCIPAL"), Some("agent:sdc-experiment"));
        let j = experiment_json("x", &plan[1], "8db4b9338f1d8d32c85b9cb2e014dac56980fac0", "abc", "2026-11-21T12:00:00Z");
        let keys: Vec<&String> = j.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["experiment", "format", "input_sha256", "ordinal", "retrospective", "run", "settings_sha256", "started_at", "tree_commit", "variant"]);
        assert_eq!(j["settings_sha256"].as_str().unwrap().len(), 64);
    }

    #[test]
    fn a_record_turns_the_conformance_observation_on_explicitly() {
        let base = |_: &str| None;
        let off = launch_env(&BTreeMap::new(), Path::new("/r"), "agent:x", &base);
        assert!(!off.iter().any(|(k, _)| k == "SDC_CONFORMANCE"));
        let on = BTreeMap::from([("conformance".to_string(), "observe".to_string())]);
        let env = launch_env(&on, Path::new("/r"), "agent:x", &base);
        assert!(env.iter().any(|(k, v)| k == "SDC_CONFORMANCE" && v == "observe"));
    }

    #[test]
    fn a_summary_drops_cases_and_absolute_paths() {
        let doc = json!({"format": "sdc.run-metrics/1", "run": "r", "cases": [{"case": "IDEA-1"}],
            "settings": {"bindings": format!("/{}/someone/x.json", "home"), "adapter": "metaharness"}, "nodes": [{"node": "a"}, {"node": "b"}], "edges": []});
        let s = summary_doc(&doc);
        assert!(s.get("cases").is_none());
        assert_eq!(s["settings"]["bindings"], "<absolute path removed>");
        assert_eq!(s["settings"]["adapter"], "metaharness");
        let t = summary_text(&s);
        assert_eq!(serde_json::from_str::<J>(&t).unwrap(), s);
        assert!(t.contains("\n    {\"node\":\"a\"},\n    {\"node\":\"b\"}\n  ]"), "{t}");
    }

    #[test]
    fn the_loom_settings_file_holds_the_login_path_only_when_the_record_names_one() {
        let mut s: BTreeMap<String, String> = [("models.writer.model", "w"), ("models.implement.model", "i"), ("models.review.model", "r"),
            ("models.other.model", "o"), ("decider.model", "d"), ("max_llm_calls", "9"), ("max_wall_seconds", "60"), ("loom.sha256", "ab"),
            ("loom.confinement", "local"), ("loom.cap_micro_usd", "7"), ("loom.bin", "/opt/b")]
            .iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        s.insert("decider.kind".into(), "model".into());
        s.insert("autonomy".into(), "A5".into());
        let base = |k: &str| (k == "HOME").then(|| "/h".to_string());
        let read = |s: &BTreeMap<String, String>, redact: bool| -> toml::Table { toml::from_str(&loom_settings_toml("r1", s, redact, &base).unwrap()).unwrap() };
        let doc = read(&s, false);
        assert_eq!(doc["models"]["implement"]["model"].as_str(), Some("i"));
        assert_eq!(doc["cap_micro_usd"].as_integer(), Some(7));
        assert_eq!(doc["max_llm_calls"].as_integer(), Some(9));
        assert!(!doc.contains_key("login_file") && !doc.contains_key("loom") && !doc.contains_key("bin"), "{doc}");
        s.insert("loom.login_file".into(), "~/.marker/login.json".into());
        assert_eq!(read(&s, false)["login_file"].as_str(), Some("/h/.marker/login.json"));
        assert_eq!(read(&s, true)["login_file"].as_str(), Some("<loom.login_file>"));
        assert!(loom_settings_toml("r1", &s, false, &|_: &str| None).is_err(), "~/ needs HOME");
        let env = loom_env(&|k: &str| (k == "PATH" || k == "SDC_RUNS_DIR").then(|| "x".to_string()), Path::new("/b"));
        assert_eq!(env, [("PATH".to_string(), "/b/driver/target/debug:x".to_string())]);
    }

    /// A record's settings that set every member the writer knows, each text with a quote, a
    /// backslash, an apostrophe and a letter outside ASCII, in `odd`.
    fn every_member(odd: &str, in_process: bool, kind: &str) -> BTreeMap<String, String> {
        let mut s: BTreeMap<String, String> = [
            ("models.writer.model", odd), ("models.implement.model", odd), ("models.review.model", odd), ("models.other.model", odd),
            ("decider.model", odd), ("decider.escalate", odd), ("decider.adapter", odd), ("decider.verdict", odd), ("adapter", odd),
            ("review.adapter", odd), ("autonomy", odd), ("loom.confinement", odd), ("loom.sha256", odd), ("loom.login_file", odd),
            ("decider.max_decider_calls", "40"), ("max_llm_calls", "9"), ("max_wall_seconds", "60"), ("loom.cap_micro_usd", "7"),
        ]
        .iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        s.insert("decider.kind".into(), kind.into());
        if in_process {
            s.insert("loom.in_process".into(), "true".into());
        }
        s
    }

    #[test]
    fn every_member_of_the_settings_file_reads_back_as_the_value_written() {
        let odd = "a\"b\\c'd\u{e9}\u{4e2d}";
        let base = |k: &str| (k == "HOME").then(|| "/h".to_string());
        for (in_process, kind) in [(false, "model"), (true, "model"), (false, "rule")] {
            let s = every_member(odd, in_process, kind);
            let text = loom_settings_toml("r1", &s, false, &base).unwrap();
            let doc: toml::Table = toml::from_str(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
            let at = |path: &[&str]| -> toml::Value {
                let mut v = toml::Value::Table(doc.clone());
                for k in path {
                    v = v.get(*k).unwrap_or_else(|| panic!("{path:?} is not in\n{text}")).clone();
                }
                v
            };
            let text_of = |path: &[&str]| at(path).as_str().map(str::to_owned).unwrap_or_else(|| panic!("{path:?} is no text in\n{text}"));
            assert_eq!(text_of(&["format"]), "runbook.loom-run-settings/1");
            assert_eq!(text_of(&["run"]), "r1");
            for role in ["writer", "implement", "review", "other"] {
                assert_eq!(text_of(&["models", role, "model"]), odd, "models.{role}.model");
            }
            assert_eq!(text_of(&["autonomy"]), odd);
            assert_eq!(text_of(&["adapter"]), odd);
            assert_eq!(text_of(&["review", "adapter"]), odd);
            assert_eq!(text_of(&["decider", "kind"]), kind);
            if kind == "rule" {
                assert_eq!(text_of(&["decider", "verdict"]), odd);
            } else {
                assert_eq!(text_of(&["decider", "model"]), odd);
                assert_eq!(text_of(&["decider", "escalate"]), odd);
                assert_eq!(text_of(&["decider", "adapter"]), odd);
                assert_eq!(at(&["decider", "max_decider_calls"]).as_integer(), Some(40));
                assert_eq!(at(&["decider", "without_claims"]).as_bool(), Some(true));
            }
            assert_eq!(at(&["max_llm_calls"]).as_integer(), Some(9));
            assert_eq!(at(&["max_wall_seconds"]).as_integer(), Some(60));
            assert_eq!(at(&["cap_micro_usd"]).as_integer(), Some(7));
            assert_eq!(text_of(&["confinement"]), odd);
            assert_eq!(text_of(&["login_file"]), odd);
            if in_process {
                assert_eq!(at(&["in_process"]).as_bool(), Some(true));
                assert!(!doc.contains_key("loom_sha256"), "{text}");
            } else {
                assert_eq!(text_of(&["loom_sha256"]), odd);
                assert!(!doc.contains_key("in_process"), "{text}");
            }
            assert!(!doc.contains_key("loom"), "{text}");
        }
    }

    #[test]
    fn a_login_path_with_a_quote_a_backslash_and_a_letter_outside_ascii_is_the_committed_odd_path_fixture() {
        // `runbook/tests/fixtures/loom-run-settings-odd-path.toml` is read by the runner's reader too
        // (`runbook/src/loom_run/settings.rs`): the two crates agree on this text, byte for byte.
        let mut s: BTreeMap<String, String> = [
            ("models.writer.model", "claude-sonnet-5-5"), ("models.implement.model", "claude-sonnet-5-5"), ("models.review.model", "claude-sonnet-5-5"),
            ("models.other.model", "claude-sonnet-5-5"), ("decider.model", "claude-opus-5-5"), ("decider.kind", "model"), ("decider.escalate", "final"),
            ("decider.adapter", "claude-cli"), ("decider.max_decider_calls", "40"), ("adapter", "metaharness"), ("review.adapter", "claude-cli"),
            ("autonomy", "A5"), ("max_llm_calls", "100"), ("max_wall_seconds", "3600"), ("loom.confinement", "local"), ("loom.cap_micro_usd", "5000000"),
            ("loom.sha256", "d80830ea5b1b2c3d4e5f60718293a4b5c6d7e8f90123456789abcdef01234567"),
        ]
        .iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        s.insert("loom.login_file".into(), "/opt/we\"ird\\dir/\u{fc}n\u{ef}/login.json".into());
        let text = loom_settings_toml("lx.low.1", &s, false, &|_: &str| None).unwrap();
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../runbook/tests/fixtures/loom-run-settings-odd-path.toml");
        let committed = fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert_eq!(text, committed);
    }

    #[test]
    fn a_number_that_is_not_a_whole_number_is_refused_by_the_writer_and_not_written_as_a_null() {
        let base = |_: &str| None;
        for bad in ["", "x", "1.5", "-3", "18446744073709551616", "9223372036854775808"] {
            let mut s = every_member("x", false, "model");
            s.insert("loom.cap_micro_usd".into(), bad.into());
            let e = loom_settings_toml("r1", &s, false, &base).unwrap_err();
            assert!(e.contains("loom.cap_micro_usd"), "{bad:?}: {e}");
        }
    }

    #[test]
    fn a_money_cap_is_in_the_settings_file_and_the_command_only_when_the_record_names_it() {
        let base = |_: &str| None;
        let words = |s: &BTreeMap<String, String>| {
            let input: Y = serde_yaml::from_str("{kind: idea, text: x}").unwrap();
            loom_argv(Path::new("/b"), "lx", "lx.low.1", s, Path::new("/r"), "sdc@1", Path::new("/f"), &input, &base).unwrap().join(" ")
        };
        let mut s = every_member("x", false, "model");
        s.insert("loom.bin".into(), "/opt/b".into());
        let text = loom_settings_toml("r1", &s, false, &base).unwrap();
        assert!(text.contains("cap_micro_usd = 7") && text.contains("max_llm_calls = 9"), "{text}");
        let w = words(&s);
        assert!(w.contains("--spend-ledger /r/lx.loom-spend.json --cap-micro-usd 7 --max-llm-calls 9 --max-wall-seconds 60"), "{w}");
        for (drop, ledger) in [(vec!["loom.cap_micro_usd", "max_llm_calls"], false), (vec!["max_llm_calls"], true), (vec!["loom.cap_micro_usd"], true)] {
            let mut s = s.clone();
            for k in &drop {
                s.remove(*k);
            }
            let text = loom_settings_toml("r1", &s, false, &base).unwrap();
            let w = words(&s);
            assert_eq!(w.contains("--spend-ledger"), ledger, "{drop:?}: {w}");
            for (k, option, member) in [("loom.cap_micro_usd", "--cap-micro-usd", "cap_micro_usd"), ("max_llm_calls", "--max-llm-calls", "max_llm_calls")] {
                let named = !drop.contains(&k);
                assert_eq!(w.contains(option), named, "{drop:?}: {w}");
                assert_eq!(text.contains(&format!("{member} = ")), named, "{drop:?}: {text}");
            }
            assert!(w.contains("--max-wall-seconds 60") && text.contains("max_wall_seconds = 60"), "{drop:?}: {w}\n{text}");
        }
    }

    #[test]
    fn a_money_cap_of_zero_is_a_cap_that_the_record_names() {
        let base = |_: &str| None;
        let mut s = every_member("x", false, "model");
        s.insert("loom.cap_micro_usd".into(), "0".into());
        s.insert("max_llm_calls".into(), "0".into());
        let text = loom_settings_toml("r1", &s, false, &base).unwrap();
        assert!(text.contains("cap_micro_usd = 0") && text.contains("max_llm_calls = 0"), "{text}");
    }

    #[test]
    fn rfc3339_of_known_instants() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(1_795_259_185), "2026-11-21T11:06:25Z");
        assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
    }
}
