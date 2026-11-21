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
//! sdc-experiment progress [--experiments <dir>] [--with-conformance]
//! sdc-experiment run <record.yaml | id> [--runs-dir <dir>] [--builds-dir <dir>] [--max-parallel <n>]
//!                    [--stagger <seconds>] [--print]
//! sdc-experiment status <record.yaml | id> [--runs-dir <dir>]
//! sdc-experiment stop <record.yaml | id> --reason <text> [--run <name>] [--runs-dir <dir>] [--builds-dir <dir>]
//! ```
//!
//! `compare` gives, per metric of `sdc.run-metrics/1` (run, node, edge, decision class), each
//! group's median, min, max and n, the difference of medians against the first group, and
//! whether the two ranges overlap (then no effect is claimed).
//!
//! An experiment record (`experiments/<id>.yaml`, `sdc.experiment/1`) is the one way a real run
//! is started and accounted for. `check` validates a record and, with `--runs-dir`, that every
//! listed run was made with its variant's settings; `check --all` validates every record and
//! fails when a report, a verdict or `experiments/INDEX.md` is stale. `report` writes, per ended
//! run, a sanitized summary to `experiments/<id>/runs/<run>.json` (no prompts, no transcripts, no
//! absolute paths), computes the result from those summaries, puts it into the record, and writes
//! `experiments/<id>/report.md` and `experiments/INDEX.md`. `run` launches a prospective record:
//! it builds the recorded commit once in a build directory keyed by the commit, starts variants ×
//! n runs as transient systemd user units with the environment taken from the record only, names
//! them `<id>.<variant>.<n>` and writes `experiment.json` into each run directory. `status` lists
//! the runs of a record; `stop` stops them with run-trial.sh's stop command and a reason.
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

use run_metrics::{choose, flat_metrics, metric_of, summary};

#[path = "sdc-experiment/chart.rs"]
mod chart;
#[path = "sdc-experiment/progress.rs"]
mod progress;

const USAGE: &str = "usage: sdc-experiment compare (--group <label>=<run dir>[,...])... | --by <key> --runs <dir>,... [--metrics <prefix>,...] [--include-open] --out <md> [--json <file>]
       sdc-experiment check <record.yaml | id> [--runs-dir <dir>] | --all [--experiments <dir>]
       sdc-experiment report <record.yaml | id> [--runs-dir <dir>] [--print]
       sdc-experiment index [--experiments <dir>]
       sdc-experiment progress [--experiments <dir>] [--with-conformance]
       sdc-experiment run <record.yaml | id> [--runs-dir <dir>] [--builds-dir <dir>] [--max-parallel <n>] [--stagger <s>] [--print]
       sdc-experiment status <record.yaml | id> [--runs-dir <dir>]
       sdc-experiment stop <record.yaml | id> --reason <text> [--run <name>] [--runs-dir <dir>] [--builds-dir <dir>]";

struct Usage(String);

/// One loaded run: its name, its metrics document and the flat figures.
struct Run {
    name: String,
    doc: J,
    flat: BTreeMap<String, f64>,
}

fn load_run(dir: &Path) -> Result<Run, String> {
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
const LAUNCH: [(&str, &str); 22] = [
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
    ("flags.dry", ""),
    ("flags.recorded", ""),
];
const REQUIRED_LAUNCH: [&str; 11] = ["models.writer.model", "models.implement.model", "models.review.model", "models.other.model",
    "decider.kind", "decider.model", "decider.escalate", "decider.max_decider_calls", "adapter", "max_llm_calls", "autonomy"];
/// Launch settings that `run-settings.jsonl` does not record, so no run can be checked against them.
const NOT_RECORDED: [&str; 4] = ["decider.adapter", "review.adapter", "flags.dry", "flags.recorded"];
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
        Variant { id: ystr(v, "id").unwrap_or("").to_string(), own, settings, runs: run_refs(v) }
    }).collect()
}

fn has_hypothesis(r: &Y) -> bool {
    ystr(r, "hypothesis").is_some_and(|h| !h.trim().is_empty())
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
    // Variants and runs.
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
        if prospective {
            for k in REQUIRED_LAUNCH {
                if !v.settings.contains_key(k) {
                    p.push(format!("variant {:?}: the settings do not set {k} (a launch needs every class's model, the decider, the adapter, the call cap and the autonomy)", v.id));
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
    for k in ["baseline", "treatment"] {
        if !vs.iter().any(|v| Some(v.id.as_str()) == ystr(r, k)) {
            p.push(format!("`{k}` must name a variant"));
        }
    }
    if ystr(r, "baseline").is_some() && ystr(r, "baseline") == ystr(r, "treatment") {
        p.push("`baseline` and `treatment` must differ".into());
    }
    let metric_ok = |m: &str| METRIC_PREFIXES.iter().any(|x| m.starts_with(x)) && m.rsplit_once('.').is_some_and(|(_, f)| !f.is_empty());
    let primary = r.get("primary").cloned().unwrap_or(Y::Null);
    if !ystr(&primary, "metric").is_some_and(metric_ok) {
        p.push("`primary.metric` must be a metric name (run., node., edge. or decision.<...>.<field>)".into());
    }
    if !matches!(ystr(&primary, "direction"), Some("increase" | "decrease")) {
        p.push("`primary.direction` must be increase or decrease".into());
    }
    if primary.get("min_effect").and_then(Y::as_f64).is_none_or(|x| x < 0.0) {
        p.push("`primary.min_effect` must be a number of at least 0 (the size the hypothesis names)".into());
    }
    if let (Some(m), Some(t)) = (ystr(&primary, "metric"), Some(tname).filter(|t| !t.is_empty())) {
        if !m.contains(t) && ystr(&target, "kind") != Some("run") {
            p.push(format!("`primary.metric` {m:?} is not a metric of the target {t:?}"));
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
                    Some(d) => match run_metrics::load(&d) {
                        Ok(x) => x,
                        Err(run_metrics::Fail(m)) => {
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
                if NOT_RECORDED.contains(&k.as_str()) {
                    continue;
                }
                let got = doc["settings"][k.as_str()].as_str().unwrap_or("not recorded").to_string();
                if &got != x {
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
    if has_hypothesis(r) {
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
        .filter(|p| p.extension().is_some_and(|x| x == "yaml") && p.file_name().is_none_or(|n| n != progress::RELEASES))
        .collect();
    v.sort();
    Ok(v)
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
            match progress::write(&exp, false) {
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
    let mut conformance = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--with-conformance" => conformance = true,
            "--experiments" => exp = PathBuf::from(it.next().ok_or_else(|| Usage("--experiments needs a directory".into()))?),
            x => return Err(Usage(format!("unknown argument {x}"))),
        }
    }
    if !exp.join(progress::RELEASES).is_file() {
        eprintln!("sdc-experiment: no {}", exp.join(progress::RELEASES).display());
        return Ok(ExitCode::from(2));
    }
    match progress::write(&exp, conformance) {
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
        for v in &vs {
            out.push(Planned { variant: v.clone(), ordinal: n, name: format!("{id}.{}.{n}", v.id),
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

fn launch_argv(build: &Path, name: &str, input: &Y, settings: &BTreeMap<String, String>, runs_dir: &Path) -> Vec<String> {
    let mut argv = vec![build.join("scripts").join("run-trial.sh").display().to_string()];
    let dry = settings.get("flags.dry").is_some_and(|v| v == "true");
    if dry {
        argv.push("--dry".into());
    } else if settings.get("autonomy").is_some_and(|a| a == "A5") {
        argv.push("--autopilot".into());
    }
    argv.push(name.to_string());
    let text = ystr(input, "text").unwrap_or("").trim().to_string();
    match ystr(input, "kind") {
        Some("idea") if !dry => argv.push(text),
        Some("seed") => {
            argv.push("--bugfix".into());
            argv.push(build.join(ystr(input, "seed").unwrap_or("")).display().to_string());
            argv.push(text);
        }
        _ => {}
    }
    if let Some(src) = settings.get("flags.recorded").filter(|s| !s.is_empty() && s.as_str() != "null") {
        argv.push("--recorded".into());
        argv.push(runs_dir.join(src).display().to_string());
    }
    argv
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
fn prepare_build(repo: &Path, builds: &Path, commit: &str, runs_dir: &Path) -> Result<PathBuf, String> {
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
    if marker.is_file() {
        return Ok(dir);
    }
    if runs_on_build(runs_dir, commit) {
        return Err(format!("refused: runs exist on {commit} but its build directory has no build record; it is not rebuilt in place"));
    }
    let mut built = vec![];
    for (krate, args) in [("driver", &["build", "--bins"][..]), ("goals", &["build"][..]), ("watch", &["build", "--release"][..])] {
        let manifest = dir.join(krate).join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        println!("building {krate} at {commit} ...");
        let st = Command::new("cargo").args(args).arg("--manifest-path").arg(&manifest).status().map_err(|e| format!("cargo: {e}"))?;
        if !st.success() {
            return Err(format!("cargo {} for {krate} at {commit} failed ({st})", args.join(" ")));
        }
        built.push(krate);
    }
    let doc = json!({"format": "sdc.experiment-build/1", "commit": commit, "built_at": now_rfc3339(), "crates": built});
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
}

fn run_cmd(args: &[String]) -> Result<ExitCode, Usage> {
    let mut a = LaunchArgs { rec: String::new(), runs_dir: PathBuf::from("runs"), builds: default_builds_dir(), max_parallel: None, stagger: None, print: false };
    let mut it = args.iter();
    while let Some(x) = it.next() {
        let mut value = || it.next().cloned().ok_or_else(|| Usage(format!("{x} needs a value")));
        match x.as_str() {
            "--runs-dir" => a.runs_dir = PathBuf::from(value()?),
            "--builds-dir" => a.builds = PathBuf::from(value()?),
            "--max-parallel" => a.max_parallel = Some(value()?.parse().ok().filter(|n| *n > 0).ok_or_else(|| Usage("--max-parallel needs a number of at least 1".into()))?),
            "--stagger" => a.stagger = Some(value()?.parse().map_err(|_| Usage("--stagger needs seconds".into()))?),
            "--print" => a.print = true,
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

fn launch(a: &LaunchArgs) -> Result<(), (u8, String)> {
    let path = record_path(&a.rec);
    let (mut r, raw) = read_record(&path).map_err(|m| (2, m))?;
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
    if a.print {
        println!("experiment {id}: {} runs, tree {commit}, build {}, at most {max_parallel} at once, {stagger} s apart", plan.len(), build.display());
        for p in &plan {
            let env = launch_env(&p.variant.settings, &runs_dir, &principal, &base);
            println!("{} ({}):", p.name, p.unit);
            for (k, v) in env.iter().filter(|(k, _)| k.starts_with("SDC_")) {
                println!("  {k}={v}");
            }
            println!("  {}", launch_argv(&build, &p.name, &input, &p.variant.settings, &runs_dir).join(" "));
        }
        return Ok(());
    }
    let repo = PathBuf::from(git(Path::new("."), &["rev-parse", "--show-toplevel"]).map_err(|m| (1, m))?);
    let build = prepare_build(&repo, &builds, &commit, &runs_dir).map_err(|m| (1, m))?;
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
        let env = launch_env(&p.variant.settings, &runs_dir, &principal, &base);
        let argv = launch_argv(&build, &p.name, &input, &p.variant.settings, &runs_dir);
        let st = Command::new("systemd-run")
            .args(["--user", "--quiet", "--unit", &p.unit, "--description"])
            .arg(format!("sdc experiment {id}: run {}", p.name))
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
        // run-trial.sh removes and recreates a new run's directory before it writes events/; the
        // record goes in after that.
        let mut waited = 0;
        while !dir.join("events").is_dir() && waited < 120 {
            sleep_s(1);
            waited += 1;
        }
        if !dir.join("events").is_dir() {
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
        let doc = if dir.is_dir() {
            run_metrics::load(&dir).ok()
        } else {
            load_summary(&summaries_dir(&exp, &id).join(format!("{run}.json"))).ok().flatten().map(|x| x.doc)
        };
        let state = if unit.is_empty() { "-".to_string() } else { Some(unit_state(&unit)).filter(|s| !s.is_empty()).unwrap_or_else(|| "none".into()) };
        let (cost, wall, end) = match &doc {
            Some(d) => (d["metrics"]["cost_usd"].as_f64().map_or("-".into(), |x| format!("{x:.3}")),
                d["metrics"]["wall_s"].as_i64().map_or("-".into(), |x| x.to_string()), d["metrics"]["end"].as_str().unwrap_or("").to_string()),
            None => ("-".into(), "-".into(), "not started".into()),
        };
        println!("{run:<34} {variant:<10} {state:<12} {cost:>9} {wall:>7}  {end}");
    }
    Ok(ExitCode::SUCCESS)
}

fn stop_cmd(args: &[String]) -> Result<ExitCode, Usage> {
    let (mut rec, mut runs_dir, mut builds, mut reason, mut only) =
        (None::<String>, PathBuf::from("runs"), default_builds_dir(), None::<String>, None::<String>);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().cloned().ok_or_else(|| Usage(format!("{a} needs a value")));
        match a.as_str() {
            "--runs-dir" => runs_dir = PathBuf::from(value()?),
            "--builds-dir" => builds = PathBuf::from(value()?),
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
    let script = ystr(&r, "tree_commit").filter(|c| is_hex(c, 40, 40)).map(|c| absolute(&builds).join(c).join("scripts").join("run-trial.sh"))
        .filter(|s| s.is_file()).unwrap_or_else(|| absolute(Path::new("scripts/run-trial.sh")));
    let mut code = ExitCode::SUCCESS;
    let units: BTreeMap<String, String> = planned_runs(&r).into_iter().map(|p| (p.name, p.unit)).collect();
    for v in variants(&r) {
        for rr in v.runs.iter().filter(|x| only.as_ref().is_none_or(|o| o == &x.run)) {
            let dir = runs_dir.join(&rr.run);
            let open = dir.is_dir() && run_metrics::load(&dir).is_ok_and(|d| d["metrics"]["finished"] != true);
            if open {
                let st = Command::new(&script).arg(&rr.run).arg("--stop").arg(&reason)
                    .env("SDC_RUNS_DIR", &runs_dir).env("SDC_PRINCIPAL", &principal).status();
                match st {
                    Ok(s) if s.success() => println!("{}: stopped ({reason})", rr.run),
                    Ok(s) => {
                        eprintln!("sdc-experiment: run-trial.sh {} --stop: {s}", rr.run);
                        code = ExitCode::from(1);
                    }
                    Err(e) => {
                        eprintln!("sdc-experiment: {}: {e}", script.display());
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
        let input = yseq(&y, "inputs")[0].clone();
        let argv = launch_argv(Path::new("/b"), &plan[1].name, &input, &plan[1].variant.settings, Path::new("/r"));
        assert_eq!(argv, ["/b/scripts/run-trial.sh", "--autopilot", "x.high.1", IDEA]);
        let j = experiment_json("x", &plan[1], "8db4b9338f1d8d32c85b9cb2e014dac56980fac0", "abc", "2026-11-21T12:00:00Z");
        let keys: Vec<&String> = j.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["experiment", "format", "input_sha256", "ordinal", "retrospective", "run", "settings_sha256", "started_at", "tree_commit", "variant"]);
        assert_eq!(j["settings_sha256"].as_str().unwrap().len(), 64);
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
    fn rfc3339_of_known_instants() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(1_795_259_185), "2026-11-21T11:06:25Z");
        assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
    }
}
