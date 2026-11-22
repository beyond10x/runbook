//! sdc-replay: re-drive a recorded run (docs/driver.md, "Recorded replay").
//!
//!   sdc-replay <request.json>      the adapter: called by `sdc-node`
//!                                  like any adapter program (SDC_LLM_ADAPTER, SDC_DECIDER_ADAPTER)
//!   sdc-replay prepare --source <run or recording> --out <new run> --tree <sdc checkout>
//!                      [--source-tree <commit>] [--on-divergence stop|nearest|live]
//!                      [--live-adapter dry|metaharness|claude-cli|<path>] [--live-cap <n>]
//!                      [--inject <fault>]... [--set <condition>=<value>]... [--take-recorded <actions>]
//!   sdc-replay env --out <run>     the environment of a re-driven run, again (a resume)
//!   sdc-replay settings --out <run>  the `replay` object of run-settings.jsonl
//!   sdc-replay prepare ... --plan-only   the same plan without writing anything (`runbook run` reads the
//!                                  settings of the run from it before the run directory exists)
//!   sdc-replay program --action <action> [--plan <plan.json>] -- <argv>...   wraps a real program
//!                                  (check-fail, recorded results)
//!   sdc-replay seal <run>          at the run's end: counts into status.json and run-stopped.json,
//!                                  virtual seconds into rounds.jsonl, each call's mode into
//!                                  llm-calls.jsonl and decisions.jsonl, replay/summary.json
//!   sdc-replay compare <run>       the actions per case and the end, against the source (exit 1: differ)
//!   sdc-replay pack <run> <dir> [--source-tree <commit>]   a recording that can be committed
//!   sdc-replay index <run or recording>   the recorded calls with their keys
//!
//! Key of a call: case, node or decision, arrival and try (and the review's attempt). A work call's
//! arrival is counted as the driver counts it for `max_runs` (a `revised` event no agent action of
//! the case wrote), its try is the node's runs since that arrival, plus one. A decider call's
//! arrival is the n-th time the decision is asked in the case, its try the decider's attempt.
//!
//! A run of `runbook run` (Loom's run log, `store/runlog-case-<hex of the case id>.jsonl`, has no
//! `revised` event) is counted from the log: the case opens as arrival 1, and a later arrival is a
//! change of `case_revision` between two evaluations with no action of the case finished and no
//! child closed in between (nothing of the case's own explains it); a try counts the finished runs
//! of the node since the last arrival (not exits 126, 127, 75, 69 and 70), plus one. A rejection
//! of an approval does not start an arrival: the rework is the next try. The adapter finds the plan
//! (`replay/plan.json`) above its call's `record_dir` when `SDC_REPLAY` is not set: the runner
//! passes no environment to a program.
//! Inputs: one sha256 per file of the call's `in/` and the request without its paths and
//! ordinals, each normalised (run directories, home paths, instants, revisions, seconds).
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::OnceLock;

use clap::{Parser, Subcommand};
use regex::Regex;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

type R<T> = Result<T, String>;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Prepare {
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        tree: Option<PathBuf>,
        #[arg(long)]
        source_tree: Option<String>,
        #[arg(long, default_value = "stop")]
        on_divergence: String,
        #[arg(long, default_value = "metaharness")]
        live_adapter: String,
        #[arg(long, default_value_t = 5)]
        live_cap: u64,
        #[arg(long)]
        inject: Vec<String>,
        #[arg(long = "set")]
        set: Vec<String>,
        #[arg(long, default_value = "")]
        take_recorded: String,
        #[arg(long)]
        plan_only: bool,
    },
    Env {
        #[arg(long)]
        out: PathBuf,
    },
    Settings {
        #[arg(long)]
        out: PathBuf,
    },
    Program {
        #[arg(long)]
        action: String,
        #[arg(long)]
        plan: Option<PathBuf>,
        #[arg(last = true)]
        argv: Vec<String>,
    },
    Seal {
        run: PathBuf,
    },
    Compare {
        run: PathBuf,
    },
    Pack {
        run: PathBuf,
        dest: PathBuf,
        #[arg(long)]
        source_tree: Option<String>,
    },
    Index {
        source: PathBuf,
    },
}

// --- files ---------------------------------------------------------------------------------------

fn read_json(p: &Path) -> R<Value> {
    let b = fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
    serde_json::from_slice(&b).map_err(|e| format!("{}: {e}", p.display()))
}

fn read_json_or(p: &Path, d: Value) -> Value {
    read_json(p).unwrap_or(d)
}

fn write_json(p: &Path, v: &Value) -> R<()> {
    if let Some(d) = p.parent() {
        fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = p.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(v).unwrap() + "\n").map_err(|e| e.to_string())?;
    fs::rename(&tmp, p).map_err(|e| e.to_string())
}

/// Writes `v` unless the file exists: the first divergence is the one that ended the run, a retry
/// of the same call does not replace it.
fn write_first(p: &Path, v: &Value) -> R<()> {
    if p.exists() { Ok(()) } else { write_json(p, v) }
}

fn jsonl(p: &Path) -> Vec<Value> {
    fs::read_to_string(p)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

fn append(p: &Path, v: &Value) -> R<()> {
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(p)
        .map_err(|e| format!("{}: {e}", p.display()))?;
    f.write_all(format!("{}\n", serde_json::to_string(v).unwrap()).as_bytes())
        .map_err(|e| e.to_string())
}

fn write_jsonl(p: &Path, lines: &[Value]) -> R<()> {
    let text: String = lines.iter().map(|v| serde_json::to_string(v).unwrap() + "\n").collect();
    fs::write(p, text).map_err(|e| e.to_string())
}

fn sha(b: &[u8]) -> String {
    Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(p) = stack.pop() {
        let Ok(rd) = fs::read_dir(&p) else { continue };
        for e in rd.flatten() {
            let path = e.path();
            if path.is_dir() {
                stack.push(path)
            } else {
                out.push(path)
            }
        }
    }
    out.sort();
    out
}

/// Copy a tree; text files go through `map` (a revision mapping), others as they are.
fn copy_tree(src: &Path, dst: &Path, map: &dyn Fn(&str) -> String) -> R<()> {
    for f in files_under(src) {
        let rel = f.strip_prefix(src).unwrap();
        let to = dst.join(rel);
        fs::create_dir_all(to.parent().unwrap()).map_err(|e| e.to_string())?;
        let b = fs::read(&f).map_err(|e| e.to_string())?;
        match String::from_utf8(b) {
            Ok(t) => fs::write(&to, map(&t)),
            Err(e) => fs::write(&to, e.into_bytes()),
        }
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn clear_dir(d: &Path) -> R<()> {
    if d.exists() {
        fs::remove_dir_all(d).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(d).map_err(|e| e.to_string())
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn canon(p: &Path) -> PathBuf {
    fs::canonicalize(p).unwrap_or(p.to_path_buf())
}

// --- normalisation -------------------------------------------------------------------------------

fn re(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pat).unwrap())
}

/// What may differ between two runs of the same inputs: the run directories, home paths (the
/// checkout and the case tree), instants, artifact revisions (they digest files that carry
/// instants, the aep store's transitions) and seconds in program output.
fn normalise(text: &str, roots: &[String]) -> String {
    static HOME: OnceLock<Regex> = OnceLock::new();
    static INSTANT: OnceLock<Regex> = OnceLock::new();
    static REV: OnceLock<Regex> = OnceLock::new();
    static SECS: OnceLock<Regex> = OnceLock::new();
    let mut t = text.to_string();
    for r in roots.iter().filter(|r| r.len() > 1) {
        t = t.replace(r.as_str(), "<run>");
    }
    let t = re(&HOME, r#"/home/[^/\s"'`]+/[^\s"'`):,]*"#).replace_all(&t, "<path>");
    let t = re(&INSTANT, r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(\.\d+)?( ?(Z|[+-]\d{2}:?\d{2}))?").replace_all(&t, "<instant>");
    let t = re(&REV, r"\bt[0-9a-f]{16}\b").replace_all(&t, "<rev>");
    let t = re(&SECS, r"\b\d+(\.\d+)?m?s\b").replace_all(&t, "<s>").into_owned();
    sort_test_lines(&t)
}

/// Test harness lines (`test <name> ... ok`) come in the order the threads finish: sorted in place.
fn sort_test_lines(t: &str) -> String {
    static TEST: OnceLock<Regex> = OnceLock::new();
    let r = re(&TEST, r"^[\s|]*test \S+ \.\.\. \S+\s*$");
    let mut lines: Vec<&str> = t.split('\n').collect();
    let at: Vec<usize> = (0..lines.len()).filter(|&i| r.is_match(lines[i])).collect();
    let mut sorted: Vec<&str> = at.iter().map(|&i| lines[i]).collect();
    sorted.sort();
    for (i, l) in at.into_iter().zip(sorted) {
        lines[i] = l;
    }
    lines.join("\n")
}

fn hash_file(p: &Path, roots: &[String]) -> String {
    let b = fs::read(p).unwrap_or_default();
    match std::str::from_utf8(&b) {
        Ok(t) => sha(normalise(t, roots).as_bytes()),
        Err(_) => sha(&b),
    }
}

/// One digest per file under `in/`, keyed `in/<path>`.
fn hash_inputs(dir: &Path, roots: &[String]) -> BTreeMap<String, String> {
    files_under(dir)
        .into_iter()
        .map(|f| (format!("in/{}", f.strip_prefix(dir).unwrap().display()), hash_file(&f, roots)))
        .collect()
}

/// The request as the model saw it: without its paths (prompt_file, cwd, record_dir, case_dir,
/// json_schema) and ordinals (attempt, review_attempt: the key holds them).
fn normalise_request(req: &Value, roots: &[String]) -> Value {
    let mut m = req.as_object().cloned().unwrap_or_default();
    for k in ["prompt_file", "cwd", "record_dir", "case_dir", "json_schema", "attempt", "review_attempt"] {
        m.remove(k);
    }
    serde_json::from_str(&normalise(&Value::Object(m).to_string(), roots)).unwrap_or(Value::Null)
}

/// The response of a recorded call without what holds paths or transcript excerpts.
fn sanitise_response(r: &Value) -> Value {
    let keep = [
        "adapter", "model", "exit", "duration_ms", "wall_ms", "cost_usd", "num_turns", "usage", "is_error",
        "subtype", "terminal_reason", "api_error_status", "schema_constrained", "schema_note", "observed_models", "dry",
    ];
    let mut m = Map::new();
    for k in keep {
        if let Some(v) = r.get(k) {
            m.insert(k.into(), v.clone());
        }
    }
    Value::Object(m)
}

// --- the recording -------------------------------------------------------------------------------

fn program_error(exit: &Value) -> bool {
    exit == "126" || exit == "127" || exit == "75" || exit == "69" || exit == "70"
}

/// The arrival and the try of the run whose `action_started` is event `i` (see the module doc).
fn arrival_try(evs: &[Value], writes: &HashSet<String>, i: usize, action: &str) -> (u64, u64) {
    let is_arrival = |j: usize| {
        evs[j]["event"] == "revised"
            && !(j > 0
                && evs[j - 1]["event"] == "action_finished"
                && evs[j - 1]["principal"].is_null()
                && writes.contains(evs[j - 1]["action"].as_str().unwrap_or("")))
    };
    let arrivals: Vec<usize> = (0..i.min(evs.len())).filter(|&j| is_arrival(j)).collect();
    let since = arrivals.last().copied().unwrap_or(0);
    let runs = evs[since..i.min(evs.len())]
        .iter()
        .filter(|e| {
            e["event"] == "action_finished"
                && e["action"] == action
                && !program_error(&e["exit"])
                && !e["not_called"].is_string()
        })
        .count();
    (arrivals.len() as u64, runs as u64 + 1)
}

fn write_actions(ir: &Value) -> HashSet<String> {
    ir["actions"]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(_, a)| a["effect"] == "write")
        .map(|(k, _)| k.clone())
        .collect()
}

/// The events of a case of `runbook run` in the old driver's shape, from Loom's run log
/// (`<store>/runlog-case-<hex of the case id>.jsonl`): `opened`, a `revised` per arrival (see the
/// module doc), and an `action_started` and `action_finished` per finished action. `None` when the
/// store has no log of the case. A call that is in flight has no `action_finished` yet.
fn loom_events(store: &Path, case: &str) -> Option<Vec<Value>> {
    let hex: String = case.bytes().map(|b| format!("{b:02x}")).collect();
    let text = fs::read_to_string(store.join(format!("runlog-case-{hex}.jsonl"))).ok()?;
    let mut evs: Vec<Value> = Vec::new();
    let mut last_revision: Option<u64> = None;
    let mut own_since_evaluation = false;
    let mut principal = Value::Null;
    for line in text.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()) {
        match line["kind"].as_str() {
            Some("CaseOpened") => evs.push(json!({"event": "opened"})),
            Some("Evaluated") => {
                let revision = line["case_revision"].as_u64();
                if last_revision.is_none() || (last_revision != revision && !own_since_evaluation) {
                    evs.push(json!({"event": "revised", "revision": revision}));
                }
                last_revision = revision;
                own_since_evaluation = false;
            }
            Some("AuthorityRecorded") => principal = line["principal"].clone(),
            Some("ActionFinished") => {
                let action = line["action"].clone();
                evs.push(json!({"event": "action_started", "action": action}));
                let exit = line["exit"].as_str().map_or_else(|| line["exit"].to_string(), String::from);
                evs.push(json!({"event": "action_finished", "action": action, "exit": exit, "principal": principal}));
                principal = Value::Null;
                own_since_evaluation = true;
            }
            Some("ChildClosed") => own_since_evaluation = true,
            _ => {}
        }
    }
    Some(evs)
}

/// Whether a run directory is one of `runbook run`: Loom's run logs are in its store.
fn is_loom_run(run: &Path) -> bool {
    fs::read_dir(run.join("store")).into_iter().flatten().flatten().any(|e| e.file_name().to_string_lossy().starts_with("runlog-case-"))
}

/// The directory of the work calls' records: `llm/` of the old driver, `work/.sdc/llm/` of `runbook run`.
fn llm_dir(run: &Path) -> PathBuf {
    let loom = run.join("work/.sdc/llm");
    if loom.is_dir() { loom } else { run.join("llm") }
}

/// Every case of a run: id → (directory in the store, if there is one; events).
fn run_cases(run: &Path) -> BTreeMap<String, (Option<PathBuf>, Vec<Value>)> {
    let mut out = BTreeMap::new();
    let loom = is_loom_run(run);
    let mut stack = vec![run.join("store")];
    while let Some(p) = stack.pop() {
        let Ok(rd) = fs::read_dir(&p) else { continue };
        for e in rd.flatten() {
            let path = e.path();
            if path.is_dir() && !path.is_symlink() && path.file_name().is_some_and(|n| n != "work") {
                stack.push(path.clone());
            }
            if path.file_name().is_some_and(|n| n == "case.json") {
                let dir = path.parent().unwrap().to_path_buf();
                if let Some(id) = read_json_or(&path, Value::Null)["id"].as_str() {
                    out.insert(id.to_string(), (Some(dir.clone()), jsonl(&dir.join("events.jsonl"))));
                }
            } else if loom && path.file_name().is_some_and(|n| n == "events.jsonl") && !path.with_file_name("case.json").exists() {
                // A case of `runbook run` has no case.json: its directory is named by its id.
                let dir = path.parent().unwrap().to_path_buf();
                if let Some(id) = dir.file_name().map(|n| n.to_string_lossy().to_string()) {
                    out.insert(id, (Some(dir.clone()), jsonl(&path)));
                }
            }
        }
    }
    for f in files_under(&run.join("events")) {
        let id = f.file_stem().unwrap().to_string_lossy().to_string();
        out.entry(id).or_insert_with(|| (None, jsonl(&f)));
    }
    out
}

fn rel(run: &Path, p: &Path) -> String {
    p.strip_prefix(run).map(|r| r.display().to_string()).unwrap_or(p.display().to_string())
}

fn dirs_sorted(p: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(p)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    v.sort();
    v
}

fn decision_name(dr: &Value) -> String {
    dr["question"]["id"]
        .as_str()
        .or(dr["finding"]["id"].as_str())
        .or(dr["action"].as_str())
        .unwrap_or("")
        .to_string()
}

fn case_of_dir(p: &str) -> String {
    p.trim_end_matches('/').rsplit('/').next().unwrap_or("").to_string()
}

/// The revisions a decider call was shown, artifact → revision (in/decision.json
/// `citable_artifacts`, the requester's in/request.json subject).
fn shown_revisions(cwd_in: &Path) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    let d = read_json_or(&cwd_in.join("decision.json"), Value::Null);
    for a in d["citable_artifacts"].as_array().into_iter().flatten() {
        if let (Some(k), Some(v)) = (a["artifact"].as_str(), a["revision"].as_str()) {
            m.insert(k.to_string(), v.to_string());
        }
    }
    let r = read_json_or(&cwd_in.join("request.json"), Value::Null);
    if let (Some(k), Some(v)) = (r["subject_artifact"].as_str(), r["subject_revision"].as_str()) {
        m.insert(k.to_string(), v.to_string());
    }
    m
}

/// The recorded calls of a run directory, in the order they were made, with their keys.
fn index_run(run: &Path) -> R<Vec<Value>> {
    let run = canon(run);
    let roots = vec![run.display().to_string()];
    let cases = run_cases(&run);
    let loom = is_loom_run(&run);
    let mut calls = Vec::new();
    // Work calls: the k-th record of (case, node) is the k-th run of the node in the case that
    // reached the adapter (not `not_called`, no missing program).
    let mut seen: BTreeMap<(String, String), usize> = BTreeMap::new();
    for rec in dirs_sorted(&llm_dir(&run)) {
        let Ok(req) = read_json(&rec.join("request.json")) else { continue };
        let node = req["node"].as_str().unwrap_or("").to_string();
        let case = req["case"]
            .as_str()
            .filter(|c| !c.is_empty())
            .map(String::from)
            .unwrap_or_else(|| case_of_dir(req["case_dir"].as_str().unwrap_or("")));
        let k = seen.entry((case.clone(), node.clone())).or_insert(0);
        *k += 1;
        let (mut arrival, mut try_) = (0, *k as u64);
        let mut matched = false;
        let from_log = if loom { loom_events(&run.join("store"), &case) } else { None };
        if let Some((dir, evs)) = cases.get(&case).map(|(d, e)| (d, from_log.as_ref().unwrap_or(e))) {
            let writes = dir.as_ref().map(|d| write_actions(&read_json_or(&d.join("ir.json"), Value::Null))).unwrap_or_default();
            let mut started = None;
            let mut n = 0;
            for (i, e) in evs.iter().enumerate() {
                if e["action"] != node.as_str() {
                    continue;
                }
                if e["event"] == "action_started" {
                    started = Some(i);
                } else if e["event"] == "action_finished"
                    && !e["not_called"].is_string()
                    && e["exit"] != "126"
                    && e["exit"] != "127"
                {
                    n += 1;
                    if n == *k {
                        if let Some(s) = started {
                            (arrival, try_) = arrival_try(evs, &writes, s, &node);
                            matched = true;
                        }
                        break;
                    }
                }
            }
        }
        let resp = read_json_or(&rec.join("response.json"), json!({}));
        let base = json!({
            "kind": "work", "case": case, "name": node, "arrival": arrival, "try": try_,
            "matched_event": matched,
            "inputs": hash_inputs(&rec.join("cwd/in"), &roots),
            "request": normalise_request(&req, &roots),
            "prompt": fs::read_to_string(rec.join("prompt.md")).ok().map(|p| sha(normalise(&p, &roots).as_bytes())),
        });
        // change.review asks again inside one call: response.try<n>.json and reply.try<n>.json
        // keep each earlier attempt, response.json and the structured output the last.
        let last = req["review_attempt"].as_u64().unwrap_or(1);
        for a in 1..=last {
            let mut c = base.clone();
            let (response, structured, out) = if a < last {
                (read_json_or(&rec.join(format!("response.try{a}.json")), json!({})),
                 Some(rec.join(format!("reply.try{a}.json"))).filter(|p| p.exists()), None)
            } else {
                (resp.clone(), Some(rec.join("structured-output.json")).filter(|p| p.exists()),
                 Some(rec.join("cwd/out")).filter(|p| p.exists()))
            };
            c["id"] = json!(if last > 1 { format!("{}#{a}", rel(&run, &rec)) } else { rel(&run, &rec) });
            c["attempt"] = json!(a);
            c["virtual_ms"] = json!(response["duration_ms"].as_u64().or(response["wall_ms"].as_u64()).unwrap_or(0));
            c["response"] = sanitise_response(&response);
            c["out"] = json!(out.map(|p| p.display().to_string()));
            c["structured"] = json!(structured.map(|p| p.display().to_string()));
            calls.push(c);
        }
    }
    // Decider calls (decider/NNN-<id>/, not decider/rule): the arrival is the n-th asking of the
    // decision in the case, the try the attempt.
    let mut asked: BTreeMap<(String, String), u64> = BTreeMap::new();
    for rec in dirs_sorted(&run.join("decider")) {
        let (Ok(dr), Ok(req)) = (read_json(&rec.join("decide-request.json")), read_json(&rec.join("request.json"))) else {
            continue;
        };
        let case = case_of_dir(dr["case_dir"].as_str().unwrap_or(""));
        let name = decision_name(&dr);
        let attempt = dr["retry"]["attempt"].as_u64().unwrap_or(1);
        let a = asked.entry((case.clone(), name.clone())).or_insert(0);
        if attempt == 1 {
            *a += 1;
        }
        let resp = read_json_or(&rec.join("response.json"), json!({}));
        let out = rec.join("cwd/out");
        calls.push(json!({
            "id": rel(&run, &rec), "kind": "decider", "case": case, "name": name, "node": req["node"],
            "arrival": *a, "try": attempt, "attempt": 1, "matched_event": true,
            "inputs": hash_inputs(&rec.join("cwd/in"), &roots),
            "request": normalise_request(&req, &roots),
            "prompt": fs::read_to_string(rec.join("prompt.md")).ok().map(|p| sha(normalise(&p, &roots).as_bytes())),
            "virtual_ms": resp["duration_ms"].as_u64().or(resp["wall_ms"].as_u64()).unwrap_or(0),
            "response": sanitise_response(&resp),
            "out": out.exists().then(|| out.display().to_string()),
            "structured": Some(rec.join("structured-output.json")).filter(|p| p.exists()).map(|p| p.display().to_string()),
            "revisions": shown_revisions(&rec.join("cwd/in")),
        }));
    }
    // Ordinals: the n-th call of a name in its case and in the run (what an injection counts).
    let mut in_case: BTreeMap<(String, String), u64> = BTreeMap::new();
    let mut in_run: BTreeMap<String, u64> = BTreeMap::new();
    for c in calls.iter_mut() {
        let (case, name) = (c["case"].as_str().unwrap().to_string(), c["name"].as_str().unwrap().to_string());
        let n = in_case.entry((case, name.clone())).or_insert(0);
        *n += 1;
        c["ordinal"] = json!(*n);
        let m = in_run.entry(name).or_insert(0);
        *m += 1;
        c["ordinal_run"] = json!(*m);
    }
    Ok(calls)
}

/// A source: a run directory, or a recording written by `pack`.
struct Source {
    path: PathBuf,
    packed: bool,
    calls: Vec<Value>,
    settings: Vec<Value>,
    status: Value,
    events: BTreeMap<String, Vec<Value>>,
    case_dirs: BTreeMap<String, PathBuf>,
    idea: Option<PathBuf>,
    goals: Option<PathBuf>,
    tree: Value,
    /// The run's SDC_PRINCIPAL, as the aep store recorded it (`actor` of a move), when it was set.
    principal: Option<String>,
}

fn tree_hint(run: &Path) -> Value {
    // Runs recorded before run-settings named the tree: the checkout path in run.log, normalised.
    let log = fs::read_to_string(run.join("run.log")).unwrap_or_default();
    static TREE: OnceLock<Regex> = OnceLock::new();
    let hint = re(&TREE, r"(/[^\s]*?)/protocol/sdc\.protocol\.yaml")
        .captures(&log)
        .map(|c| c[1].rsplit('/').next().unwrap_or("").to_string());
    json!({"commit": null, "dirty": null, "checkout": hint})
}

fn load_source(p: &Path) -> R<Source> {
    let path = canon(p);
    if path.join("recording.json").exists() {
        let r = read_json(&path.join("recording.json"))?;
        let mut calls: Vec<Value> = r["calls"].as_array().cloned().unwrap_or_default();
        for c in calls.iter_mut() {
            for k in ["out", "structured"] {
                if let Some(s) = c[k].as_str() {
                    c[k] = json!(path.join(s).display().to_string());
                }
            }
        }
        let events = files_under(&path.join("events"))
            .into_iter()
            .map(|f| (f.file_stem().unwrap().to_string_lossy().to_string(), jsonl(&f)))
            .collect();
        return Ok(Source {
            calls,
            settings: r["settings"].as_array().cloned().unwrap_or_default(),
            status: r["status"].clone(),
            events,
            case_dirs: BTreeMap::new(),
            idea: Some(path.join("idea.yaml")).filter(|p| p.exists()),
            goals: Some(path.join("goals.yaml")).filter(|p| p.exists()),
            tree: r["tree"].clone(),
            principal: r["principal"].as_str().map(String::from),
            packed: true,
            path,
        });
    }
    if !llm_dir(&path).is_dir() && !path.join("decider").is_dir() {
        return Err(format!("{}: not a recorded run (no llm/ or decider/) and not a recording (no recording.json)", path.display()));
    }
    let settings = jsonl(&path.join("run-settings.jsonl"));
    let tree = settings
        .iter()
        .rev()
        .find(|s| s["tree"]["commit"].is_string())
        .map(|s| s["tree"].clone())
        .unwrap_or_else(|| tree_hint(&path));
    let cases = run_cases(&path);
    let status = read_json(&path.join("status.json"))
        .or_else(|_| read_json(&path.join("run-stopped.json")))
        .unwrap_or(Value::Null);
    Ok(Source {
        calls: index_run(&path)?,
        settings,
        status,
        events: cases.iter().map(|(k, (_, e))| (k.clone(), e.clone())).collect(),
        case_dirs: cases.iter().filter_map(|(k, (d, _))| d.clone().map(|d| (k.clone(), d))).collect(),
        idea: [path.join("store/IDEA-1/work/idea.yaml"), path.join("work/idea.yaml")].into_iter().find(|p| p.exists()),
        goals: [path.join("store/IDEA-1/work/goals.yaml"), path.join("work/goals.yaml")].into_iter().find(|p| p.exists()),
        tree,
        principal: aep_actor(&path.join("store/IDEA-1/work/aep")).or_else(|| aep_actor(&path.join("work/aep"))),
        packed: false,
        path,
    })
}

/// The principal `sdc-node` gives aep (AEP_ACTOR): SDC_PRINCIPAL when it was a `<kind>:<name>`
/// principal, else `agent:<node>` (then None: the variable was not set).
fn aep_actor(store: &Path) -> Option<String> {
    static ACTOR: OnceLock<Regex> = OnceLock::new();
    files_under(store).iter().filter_map(|f| fs::read_to_string(f).ok()).find_map(|t| {
        re(&ACTOR, r#"actor: "([^"]+)""#).captures(&t).map(|c| c[1].to_string())
    }).filter(|a| !a.starts_with("agent:") || !a[6..].contains('.'))
}

fn git_tree(dir: &Path) -> Value {
    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    json!({"commit": git(&["rev-parse", "--short=12", "HEAD"]),
        "dirty": git(&["status", "--porcelain", "--untracked-files=no"]).map(|s| !s.is_empty())})
}

// --- prepare -------------------------------------------------------------------------------------

/// `<kind>@[<case>/]<name>#<n>[=<value>]` (check-fail@<action>#<n>, reject@<decision>#<n>=<reason>).
fn parse_fault(s: &str) -> R<Value> {
    let bad = || format!("--inject {s}: expected <fault>@[<case>/]<name>#<n>[=<value>], fault one of timeout, invalid-reply, exit, reject, check-fail");
    let (kind, rest) = s.split_once('@').ok_or_else(bad)?;
    let (place, value) = match rest.split_once('=') {
        Some((p, v)) => (p, Some(v.to_string())),
        None => (rest, None),
    };
    let (place, n) = place.rsplit_once('#').ok_or_else(bad)?;
    let n: u64 = n.parse().map_err(|_| bad())?;
    // A case id has no slash; a name never has one either.
    let (case, name) = match place.split_once('/') {
        Some((c, nm)) => (Some(c.to_string()), nm.to_string()),
        None => (None, place.to_string()),
    };
    match kind {
        "timeout" | "invalid-reply" | "check-fail" => {}
        "exit" => {
            value.as_deref().and_then(|v| v.parse::<i32>().ok()).ok_or(format!("--inject {s}: exit@… needs =<code>"))?;
        }
        "reject" => {}
        _ => return Err(bad()),
    }
    Ok(json!({"fault": kind, "case": case, "name": name, "n": n, "value": value, "spec": s}))
}

fn classes(tools: &Path) -> Value {
    read_json_or(&tools.join("node-classes.json"), json!({"classes": {}}))["classes"].clone()
}

/// `--set` conditions: driver binding overrides (per action), environment for the scripts.
/// Binding overrides per action, environment, the conditions as given.
type Sets = (Map<String, Value>, Map<String, Value>, Map<String, Value>);

fn parse_sets(sets: &[String], tools: &Path) -> R<Sets> {
    let mut actions = Map::new();
    let mut env = Map::new();
    let mut recorded = Map::new();
    let cls = classes(tools);
    for s in sets {
        let (k, v) = s.split_once('=').ok_or(format!("--set {s}: expected <condition>=<value>"))?;
        if k == "unstick" {
            return Err(unstick_refusal(&format!("--set {s}")));
        }
        let n: u64 = v.parse().map_err(|_| format!("--set {s}: the value is a whole number"))?;
        recorded.insert(k.to_string(), json!(n));
        let per_action = |field: &str, target: &str, actions: &mut Map<String, Value>| {
            let names: Vec<String> = match cls[target].as_array() {
                Some(a) => a.iter().filter_map(|x| x.as_str().map(String::from)).collect(),
                None => vec![target.to_string()],
            };
            for a in names {
                actions.entry(a).or_insert(json!({}))[field] = json!(n);
            }
        };
        match k.split_once('.') {
            Some(("max_runs", t)) => per_action("max_runs", t, &mut actions),
            Some(("timeout", t)) => per_action("timeout_s", t, &mut actions),
            _ => {
                let var = match k {
                    "max_llm_calls" => "SDC_MAX_LLM_CALLS",
                    "max_decider_calls" => "SDC_MAX_DECIDER_CALLS",
                    "max_rejections" => "SDC_MAX_REJECTIONS",
                    "decider_retries" => "SDC_DECIDER_RETRIES",
                    "review_retries" => "SDC_REVIEW_RETRIES",
                    "max_wall_seconds" => "SDC_MAX_WALL_SECONDS",
                    _ => return Err(format!("--set {s}: unknown condition (max_runs.<node|class>, timeout.<node|class|action>, max_llm_calls, max_decider_calls, max_rejections, decider_retries, review_retries, max_wall_seconds)")),
                };
                env.insert(var.into(), json!(v));
            }
        }
    }
    Ok((actions, env, recorded))
}

#[allow(clippy::too_many_arguments)]
fn prepare(
    source: &Path,
    out: &Path,
    tree_dir: Option<&Path>,
    source_tree: Option<String>,
    policy: &str,
    live_adapter: &str,
    live_cap: u64,
    inject: &[String],
    sets: &[String],
    take_recorded: &str,
    write: bool,
) -> R<Value> {
    if !["stop", "nearest", "live"].contains(&policy) {
        return Err(format!("--on-divergence: stop, nearest or live, not {policy}"));
    }
    let src = load_source(source)?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let tree_dir = tree_dir.map(canon).unwrap_or_else(|| {
        exe.ancestors().nth(4).map(Path::to_path_buf).unwrap_or_default()
    });
    let tools = tree_dir.join("bindings/tools");
    if write {
        fs::create_dir_all(out).map_err(|e| e.to_string())?;
    }
    let out = canon(out);
    let rd = out.join("replay");
    if write {
        fs::create_dir_all(&rd).map_err(|e| e.to_string())?;
    }
    let faults: Vec<Value> = inject.iter().map(|s| parse_fault(s)).collect::<R<_>>()?;
    let (mut actions, mut env, conditions) = parse_sets(sets, &tools)?;
    let take: Vec<String> = take_recorded.split(',').filter(|s| !s.is_empty()).map(String::from).collect();
    if !take.is_empty() && src.packed {
        return Err("--take-recorded needs a run directory: a recording holds no program logs".into());
    }
    // The programs to wrap: an injected check failure, a recorded result.
    let mut wrapped: BTreeSet<String> = take.iter().filter(|a| *a != "all").cloned().collect();
    wrapped.extend(faults.iter().filter(|f| f["fault"] == "check-fail").map(|f| f["name"].as_str().unwrap().to_string()));
    if take.iter().any(|a| a == "all") {
        for evs in src.events.values() {
            for e in evs.iter().filter(|e| e["event"] == "action_finished" && e["llm"] != true) {
                if let Some(a) = e["action"].as_str() {
                    wrapped.insert(a.to_string());
                }
            }
        }
    }
    for a in &wrapped {
        actions.entry(a.clone()).or_insert(json!({}))["wrap"] =
            json!([exe.display().to_string(), "program", "--action", a, "--plan", rd.join("plan.json").display().to_string(), "--"]);
    }
    // The source's own run settings: decider, models, caps, the nodes that went to the adapter.
    let last = src.settings.last().cloned().unwrap_or(Value::Null);
    let first = src.settings.first().cloned().unwrap_or(Value::Null);
    // A variable the caller set wins: a changed model or principal is an experiment of its own.
    let mut set_env = |k: &str, v: Value| {
        if std::env::var(k).is_ok() {
            return;
        }
        if let Some(s) = v.as_str().map(String::from).or_else(|| v.as_u64().map(|n| n.to_string())) {
            env.entry(k.to_string()).or_insert(json!(s));
        }
    };
    set_env("SDC_DECIDER", last["decider"]["kind"].clone());
    set_env("SDC_REQUESTER_DECIDER", last["requester_decider"]["kind"].clone());
    set_env("SDC_MAX_LLM_CALLS", last["max_llm_calls"].clone());
    set_env("SDC_MAX_DECIDER_CALLS", last["decider"]["max_decider_calls"].clone());
    set_env("SDC_AUTONOMY", last["autonomy"].clone());
    set_env("SDC_LLM_NODES", last["llm_nodes"].clone());
    set_env("SDC_PRINCIPAL", src.principal.clone().map_or(Value::Null, Value::String));
    for (class, m) in last["models"].as_object().into_iter().flatten() {
        if m["source"] != "binding" && m["source"] != "default" {
            set_env(&format!("SDC_LLM_MODEL_{}", class.to_uppercase()), m["model"].clone());
        }
    }
    let live_bin = match live_adapter {
        "metaharness" | "claude-cli" | "dry" => tools.join(format!("llm-adapter-{live_adapter}.sh")).display().to_string(),
        p => p.to_string(),
    };
    if live_adapter == "dry" {
        env.insert("SDC_LLM_DRY_LOG".into(), json!(out.join("dry-commands.txt").display().to_string()));
    }
    if !write {
        return Ok(json!({"env": env, "settings": loom_settings(&last, &exe, &actions, &conditions, src.principal.as_deref(), &src.calls)?}));
    }
    // The index, with the source's paths resolved.
    write_json(&rd.join("index.json"), &json!({"format": "sdc.replay-index/1", "calls": src.calls}))?;
    if let Some(i) = &src.idea {
        fs::copy(i, rd.join("idea.yaml")).map_err(|e| e.to_string())?;
        env.insert("SDC_REPLAY_IDEA".into(), json!(rd.join("idea.yaml").display().to_string()));
    }
    if let Some(g) = &src.goals {
        fs::copy(g, rd.join("goals.yaml")).map_err(|e| e.to_string())?;
        env.insert("SDC_REPLAY_GOALS".into(), json!(rd.join("goals.yaml").display().to_string()));
    }
    if !actions.is_empty() {
        write_json(&rd.join("conditions.json"), &json!({"format": "sdc.conditions/1", "actions": actions}))?;
        env.insert("SDC_CONDITIONS".into(), json!(rd.join("conditions.json").display().to_string()));
    }
    let mut src_tree = src.tree.clone();
    if let Some(c) = source_tree {
        src_tree = json!({"commit": c, "dirty": null, "checkout": src_tree["checkout"], "stated": true});
    }
    let tree = git_tree(&tree_dir);
    let same = match (src_tree["commit"].as_str(), tree["commit"].as_str()) {
        (Some(a), Some(b)) => Some((a.starts_with(b) || b.starts_with(a)) && tree["dirty"] != true),
        _ => None,
    };
    let plan = json!({
        "format": "sdc.replay-plan/1",
        "source": {"path": src.path.display().to_string(), "name": src.path.file_name().map(|n| n.to_string_lossy().to_string()),
            "packed": src.packed, "tree": src_tree, "status": src.status, "calls": src.calls.len(),
            "started": first["at"], "adapter": last["adapter"]},
        "tree": tree, "same_tree": same, "out": out.display().to_string(),
        "policy": policy, "live": {"adapter": live_adapter, "bin": live_bin, "cap": live_cap},
        "injections": faults, "conditions": conditions, "take_recorded": take, "tools": tools.display().to_string(),
    });
    let mut plan = plan;
    plan["first_line"] = json!(first_line(&plan));
    write_json(&rd.join("plan.json"), &plan)?;
    env.insert("SDC_REPLAY".into(), json!(rd.join("plan.json").display().to_string()));
    env.insert("SDC_LLM_ADAPTER".into(), json!(exe.display().to_string()));
    env.insert("SDC_DECIDER_ADAPTER".into(), json!(exe.display().to_string()));
    write_json(&rd.join("env.json"), &Value::Object(env.clone()))?;
    Ok(Value::Object(env))
}

/// The settings `runbook run` takes for a re-drive (`docs/loom-run-settings.md`), from the source's
/// own run settings: its autonomy, its decider on this program as the adapter (the Loom decider
/// holds no claim values, so `without_claims`), the models the caller set, and under `replay` what
/// the bindings copy needs: this program as the adapter of the nodes the source sent to a model
/// (`nodes`, `null`: all) and the per-action conditions (`max_runs`, `timeout_s`, `wrap`). A
/// condition the engine has no setting for is refused by name.
fn loom_settings(last: &Value, exe: &Path, actions: &Map<String, Value>, conditions: &Map<String, Value>, principal: Option<&str>, calls: &[Value]) -> R<Value> {
    let mut decider_overrides = Map::new();
    for (k, v) in conditions {
        match k.as_str() {
            "max_decider_calls" => {
                decider_overrides.insert("max_decider_calls".into(), v.clone());
            }
            "decider_retries" => {
                decider_overrides.insert("retries".into(), v.clone());
            }
            "max_llm_calls" | "max_wall_seconds" => {
                return Err(format!("--set {k}: under `runbook run` it is the option --{}, which also bounds the re-drive: give the option", k.replace('_', "-")));
            }
            "max_rejections" | "review_retries" => {
                return Err(format!("--set {k}: the engine has no setting for it (the driver's switch is an environment variable of its loop, and `runbook run` passes no environment to a bound program)"));
            }
            _ if k.starts_with("max_runs.") || k.starts_with("timeout.") => {}
            other => return Err(format!("--set {other}: not a condition of a re-drive under `runbook run`")),
        }
    }
    let exe_text = exe.display().to_string();
    let mut s = Map::new();
    if let Some(a) = last["autonomy"].as_str() {
        s.insert("autonomy".into(), json!(a));
    }
    let source_decider = &last["decider"];
    let decider = match source_decider["kind"].as_str() {
        Some("model") => {
            let mut d = json!({"kind": "model", "adapter": exe_text, "without_claims": true});
            for k in ["model", "escalate", "max_decider_calls"] {
                if !source_decider[k].is_null() {
                    d[k] = source_decider[k].clone();
                }
            }
            for (k, v) in &decider_overrides {
                d[k] = v.clone();
            }
            Some(d)
        }
        Some(v @ ("approve" | "reject")) => Some(json!({"kind": "rule", "verdict": v})),
        _ => None,
    };
    if let Some(d) = decider {
        s.insert("decider".into(), d);
    }
    let mut models = Map::new();
    for (class, m) in last["models"].as_object().into_iter().flatten() {
        if m["source"] != "binding" && m["source"] != "default" {
            if let Some(model) = m["model"].as_str() {
                models.insert(class.clone(), json!({"model": model}));
            }
        }
    }
    if !models.is_empty() {
        s.insert("models".into(), Value::Object(models));
    }
    // A source run by `runbook run` names no `llm_nodes`: the nodes it called are the nodes it sent to a model.
    let called: BTreeSet<&str> = calls.iter().filter(|c| c["kind"] == "work").filter_map(|c| c["name"].as_str()).collect();
    let nodes: Option<Vec<&str>> = last["llm_nodes"]
        .as_str()
        .map(|t| t.split_whitespace().collect())
        .or_else(|| (!called.is_empty()).then(|| called.iter().copied().collect()))
        .filter(|n: &Vec<&str>| !n.is_empty());
    s.insert("replay".into(), json!({"adapter": exe_text, "nodes": nodes, "actions": actions, "principal": principal}));
    Ok(Value::Object(s))
}

fn short(c: &Value) -> String {
    c["commit"].as_str().map(|s| s.chars().take(12).collect()).unwrap_or_else(|| {
        format!("an unrecorded commit{}", c["checkout"].as_str().map(|h| format!(" (checkout {h})")).unwrap_or_default())
    })
}

/// The re-drive's first line: the source, both trees, and whether they differ.
fn first_line(plan: &Value) -> String {
    let s = &plan["source"];
    let trees = match plan["same_tree"].as_bool() {
        Some(true) => "the same tree".to_string(),
        Some(false) if short(&plan["tree"]).starts_with(&short(&s["tree"])) || short(&s["tree"]).starts_with(&short(&plan["tree"])) =>
            "the same commit plus uncommitted changes: calls can diverge for those alone".to_string(),
        Some(false) => "DIFFERENT TREES: code changed since the recording, calls can diverge for that alone".to_string(),
        None => "trees not comparable (the source names no commit): calls can diverge for code changes".to_string(),
    };
    // Each condition by its kind and value: `(max_runs 2)`, `(timeout 9)`.
    let conditions: Vec<String> = plan["conditions"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(k, v)| format!("{} {}", k.split('.').next().unwrap_or(k), v.as_str().map_or_else(|| v.to_string(), str::to_string)))
        .collect();
    format!(
        "re-drive of {} (recorded on tree {}) on tree {}{}: {}; policy {}, {} injection(s), {} condition(s){}",
        s["name"].as_str().unwrap_or("?"), short(&s["tree"]), short(&plan["tree"]),
        if plan["tree"]["dirty"] == true { " with uncommitted changes" } else { "" }, trees,
        plan["policy"].as_str().unwrap_or("stop"), plan["injections"].as_array().map_or(0, |a| a.len()),
        conditions.len() + plan["take_recorded"].as_array().map_or(0, |a| a.len()),
        if conditions.is_empty() { String::new() } else { format!(" ({})", conditions.join(", ")) }
    )
}

// --- the adapter ---------------------------------------------------------------------------------

struct Ctx {
    plan: Value,
    out: PathBuf,
    calls: Vec<Value>,
}

/// The plan nearest above `from` (a call's record directory): the run directory of `runbook run`
/// holds `replay/plan.json`, and the runner passes a program no environment to name it.
fn find_plan(from: &Path) -> Option<PathBuf> {
    from.ancestors().map(|d| d.join("replay/plan.json")).find(|p| p.is_file())
}

fn ctx() -> R<Ctx> {
    let p = std::env::var("SDC_REPLAY").map_err(|_| "sdc-replay: SDC_REPLAY (the replay plan) is not set".to_string())?;
    ctx_at(Path::new(&p))
}

/// The refusal of the condition `unstick`, which names `from` (a `--set` or a plan file): the step
/// it switched is gone, so nothing reads it.
fn unstick_refusal(from: &str) -> String {
    format!("{from}: unstick is no longer a condition of a re-drive (the step it switched was removed with the old driver)")
}

/// The refusal of a recorded plan that names `unstick` (as a condition or as its own member).
fn plan_refusal(plan: &Value, plan_path: &Path) -> Option<String> {
    (plan["conditions"].get("unstick").is_some() || plan.get("unstick").is_some()).then(|| unstick_refusal(&format!("the plan {}", plan_path.display())))
}

fn ctx_at(plan_path: &Path) -> R<Ctx> {
    let plan = read_json(plan_path)?;
    if let Some(refusal) = plan_refusal(&plan, plan_path) {
        return Err(refusal);
    }
    let out = PathBuf::from(plan["out"].as_str().unwrap_or(""));
    let calls = read_json(&out.join("replay/index.json"))?["calls"].as_array().cloned().unwrap_or_default();
    Ok(Ctx { plan, out, calls })
}

fn key_str(k: &Value) -> String {
    let a = k["attempt"].as_u64().unwrap_or(1);
    format!(
        "{}/{}@{}.{}{}",
        k["case"].as_str().unwrap_or(""), k["name"].as_str().unwrap_or(""), k["arrival"], k["try"],
        if a > 1 { format!(".{a}") } else { String::new() }
    )
}

fn same_key(c: &Value, k: &Value) -> bool {
    ["kind", "case", "name", "arrival", "try"].iter().all(|f| c[*f] == k[*f])
        && c["attempt"].as_u64().unwrap_or(1) == k["attempt"].as_u64().unwrap_or(1)
}

fn differences(c: &Value, inputs: &BTreeMap<String, String>, request: &Value, skip: &[&str]) -> Vec<String> {
    let mut d = Vec::new();
    let mut src: BTreeMap<String, String> = serde_json::from_value(c["inputs"].clone()).unwrap_or_default();
    src.retain(|f, _| !skip.contains(&f.as_str()));
    let inputs: BTreeMap<String, String> = inputs.iter().filter(|(f, _)| !skip.contains(&f.as_str())).map(|(f, h)| (f.clone(), h.clone())).collect();
    let inputs = &inputs;
    for (f, h) in inputs {
        match src.get(f) {
            None => d.push(format!("{f} is new")),
            Some(s) if s != h => d.push(format!("{f} changed")),
            _ => {}
        }
    }
    for f in src.keys().filter(|f| !inputs.contains_key(*f)) {
        d.push(format!("{f} is gone"));
    }
    let empty = Map::new();
    let (a, b) = (c["request"].as_object().unwrap_or(&empty), request.as_object().unwrap_or(&empty));
    for k in a.keys().chain(b.keys()).collect::<BTreeSet<_>>() {
        if a.get(k) != b.get(k) {
            d.push(format!("request.{k} {} -> {}", a.get(k).unwrap_or(&Value::Null), b.get(k).unwrap_or(&Value::Null)));
        }
    }
    d
}

/// The policy `nearest`: the same node (or decision) in the same case, the same arrival at the
/// nearest try (an earlier one first), else the latest earlier arrival.
fn nearest<'a>(calls: &'a [Value], k: &Value) -> Option<&'a Value> {
    let (arr, tr) = (k["arrival"].as_u64().unwrap_or(0), k["try"].as_u64().unwrap_or(1));
    let same: Vec<&Value> = calls.iter().filter(|c| c["kind"] == k["kind"] && c["case"] == k["case"] && c["name"] == k["name"]).collect();
    let at = |a: u64| same.iter().copied().filter(move |c| c["arrival"].as_u64() == Some(a));
    let pick = at(arr)
        .min_by_key(|c| {
            let t = c["try"].as_u64().unwrap_or(1);
            (if t <= tr { (tr - t) * 2 } else { (t - tr) * 2 + 1 }, c["attempt"].as_u64().unwrap_or(1))
        });
    pick.or_else(|| {
        same.iter()
            .copied()
            .filter(|c| c["arrival"].as_u64().unwrap_or(0) < arr)
            .max_by_key(|c| (c["arrival"].as_u64().unwrap_or(0), c["try"].as_u64().unwrap_or(1)))
    })
}

/// Replace each revision the source was shown by the current revision of the same artifact.
fn revision_map(src: &Value, cwd_in: &Path) -> Vec<(String, String)> {
    let now = shown_revisions(cwd_in);
    src["revisions"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(a, r)| {
            let r = r.as_str()?;
            now.get(a).filter(|n| n.as_str() != r).map(|n| (r.to_string(), n.clone()))
        })
        .collect()
}

fn fault_for<'a>(plan: &'a Value, kind: &[&str], case: &str, name: &str, n_case: u64, n_run: u64) -> Option<&'a Value> {
    plan["injections"].as_array()?.iter().find(|f| {
        kind.contains(&f["fault"].as_str().unwrap_or(""))
            && f["name"] == name
            && match f["case"].as_str() {
                Some(c) => c == case && f["n"].as_u64() == Some(n_case),
                None => f["n"].as_u64() == Some(n_run),
            }
    })
}

fn adapter(req_path: &Path) -> R<i32> {
    let started = std::time::Instant::now();
    let req = read_json(req_path)?;
    let node = req["node"].as_str().unwrap_or("").to_string();
    let rec = PathBuf::from(req["record_dir"].as_str().unwrap_or(""));
    let c = match find_plan(&rec) {
        Some(p) => ctx_at(&p)?,
        None => ctx()?,
    };
    let cwd = PathBuf::from(req["cwd"].as_str().unwrap_or(""));
    let case_dir = PathBuf::from(req["case_dir"].as_str().unwrap_or(""));
    let decider = node.starts_with("decide.") || node.starts_with("requester.");
    let roots = vec![c.out.display().to_string(), canon(&c.out).display().to_string()];
    // The key of this call.
    let key = if decider {
        let dr = read_json_or(&rec.join("decide-request.json"), Value::Null);
        let case = case_of_dir(dr["case_dir"].as_str().unwrap_or(case_dir.to_str().unwrap_or("")));
        let name = decision_name(&dr);
        let attempt = dr["retry"]["attempt"].as_u64().unwrap_or(1);
        let earlier = dirs_sorted(&c.out.join("decider"))
            .into_iter()
            .filter(|d| *d < rec)
            .filter(|d| {
                let x = read_json_or(&d.join("decide-request.json"), Value::Null);
                case_of_dir(x["case_dir"].as_str().unwrap_or("")) == case
                    && decision_name(&x) == name
                    && x["retry"]["attempt"].as_u64().unwrap_or(1) == 1
            })
            .count() as u64;
        json!({"kind": "decider", "case": case, "name": name, "arrival": earlier + u64::from(attempt == 1), "try": attempt, "attempt": 1})
    } else {
        let case = req["case"].as_str().filter(|s| !s.is_empty()).map(String::from).unwrap_or_else(|| case_of_dir(case_dir.to_str().unwrap_or("")));
        // A call of `runbook run` is in flight: its action is not finished in Loom's log yet.
        let (evs, writes, i) = match loom_events(&c.out.join("store"), &case) {
            Some(evs) => {
                let end = evs.len();
                (evs, HashSet::new(), end)
            }
            None => {
                let evs = jsonl(&case_dir.join("events.jsonl"));
                let writes = write_actions(&read_json_or(&case_dir.join("ir.json"), Value::Null));
                let i = evs.iter().rposition(|e| e["event"] == "action_started" && e["action"] == node.as_str()).unwrap_or(evs.len());
                (evs, writes, i)
            }
        };
        let (arrival, try_) = arrival_try(&evs, &writes, i, &node);
        json!({"kind": "work", "case": case, "name": node, "arrival": arrival, "try": try_,
            "attempt": req["review_attempt"].as_u64().unwrap_or(1)})
    };
    let (case, name) = (key["case"].as_str().unwrap().to_string(), key["name"].as_str().unwrap().to_string());
    let log = jsonl(&c.out.join("replay/calls.jsonl"));
    let n_case = 1 + log.iter().filter(|l| l["case"] == case.as_str() && l["name"] == name.as_str()).count() as u64;
    let n_run = 1 + log.iter().filter(|l| l["name"] == name.as_str()).count() as u64;
    let inputs = hash_inputs(&cwd.join("in"), &roots);
    let request = normalise_request(&req, &roots);
    let prompt = req["prompt_file"].as_str().and_then(|p| fs::read_to_string(p).ok()).map(|p| sha(normalise(&p, &roots).as_bytes()));
    // Resolve: recorded, else the policy.
    let exact = c.calls.iter().find(|x| same_key(x, &key));
    let mut mode = "recorded";
    let mut difference: Vec<String> = Vec::new();
    let mut chosen: Option<&Value> = None;
    match exact {
        Some(x) => {
            // Under `runbook run` a decider is shown Loom's decision and checks (`decision.json`, `checks.json`): another
            // document than the old driver's, so they say nothing about the evidence the decider read, which the other files hold.
            let envelope: &[&str] = if decider && is_loom_run(&c.out) { &["in/decision.json", "in/checks.json"] } else { &[] };
            difference = differences(x, &inputs, &request, envelope);
            if difference.is_empty() {
                chosen = Some(x);
            }
        }
        None => difference.push(format!("the source made no call at {}", key_str(&key))),
    }
    let policy = c.plan["policy"].as_str().unwrap_or("stop");
    let live_n = log.iter().filter(|l| l["mode"] == "live").count() as u64;
    // Fails closed: a cap that cannot be read is no cap to spend, so no live call starts.
    let live_cap = c.plan["live"]["cap"].as_u64();
    let mut diverged: Option<String> = None;
    if chosen.is_none() {
        match policy {
            "nearest" => match nearest(&c.calls, &key) {
                Some(x) => {
                    mode = "reused";
                    chosen = Some(x);
                }
                None => diverged = Some("nothing to reuse: the source has no call of this node or decision in this case".into()),
            },
            "live" if live_cap.is_some_and(|cap| live_n < cap) => mode = "live",
            "live" => {
                diverged = Some(match live_cap {
                    Some(cap) => format!("the live call cap is spent ({cap} live calls, --live-cap)"),
                    None => "the live call cap is not readable in the plan (live.cap): no live call starts".to_owned(),
                })
            }
            _ => diverged = Some(String::new()),
        }
    }
    let fault = fault_for(&c.plan, &["timeout", "invalid-reply", "exit", "reject"], &case, &name, n_case, n_run).cloned();
    let mut line = json!({
        "at_ms": 0, "record": rec.strip_prefix(&c.out).map(|r| r.display().to_string()).unwrap_or_default(),
        "kind": key["kind"], "case": case, "name": name, "node": node, "key": key_str(&key),
        "arrival": key["arrival"], "try": key["try"], "attempt": key["attempt"], "ordinal": n_case, "ordinal_run": n_run,
        "mode": mode, "source": chosen.map(|x| x["id"].clone()), "difference": difference,
        "prompt_changed": chosen.is_some_and(|x| x["prompt"].is_string() && prompt.is_some() && x["prompt"].as_str() != prompt.as_deref()),
        "fault": fault.as_ref().map(|f| f["spec"].clone()), "virtual_ms": 0, "clock": "virtual",
    });
    let finish = |line: &mut Value, response: &mut Value, code: i32| -> R<i32> {
        line["at_ms"] = json!(now_ms() as u64);
        let mut l = line.clone();
        l["format"] = json!("sdc.replay-call/1");
        let _ = write_json(&rec.join("replay.json"), &l);
        append(&c.out.join("replay/calls.jsonl"), line)?;
        response["replay"] = line.clone();
        response["wall_ms"] = json!(started.elapsed().as_millis() as u64);
        println!("{}", serde_json::to_string(response).unwrap());
        Ok(code)
    };
    if let Some(why) = diverged {
        let what = if why.is_empty() { difference.join("; ") } else { format!("{why}; {}", difference.join("; ")) };
        line["mode"] = json!("diverged");
        line["difference"] = json!(what);
        let summary = format!("{} at {}: {}", if decider { "decider call" } else { "model call" }, key_str(&key), what);
        write_first(&c.out.join("replay/diverged.json"), &json!({"format": "sdc.replay-divergence/1",
            "place": {"kind": key["kind"], "case": case, "case_dir": case_dir.display().to_string(), "name": name, "arrival": key["arrival"], "try": key["try"],
                "attempt": key["attempt"], "record": line["record"]},
            "policy": policy, "difference": what, "summary": summary}))?;
        let mut resp = json!({"adapter": "recorded", "model": req["model"], "exit": 75, "is_error": true,
            "terminal_reason": "recording_diverged", "duration_ms": 0, "cost_usd": 0, "num_turns": 0, "usage": null,
            "tool_calls": [], "offered_tools": []});
        if !decider {
            let _ = clear_dir(&cwd.join("out"));
        }
        return finish(&mut line, &mut resp, 75);
    }
    // The reply.
    let mut resp;
    let mut code;
    if mode == "live" {
        let bin = c.plan["live"]["bin"].as_str().unwrap_or("");
        let mut live = Command::new(bin);
        live.arg(req_path);
        if c.plan["live"]["adapter"] == "dry" {
            live.env("SDC_LLM_DRY_LOG", c.out.join("dry-commands.txt"));
        }
        let o = live.output().map_err(|e| format!("live adapter {bin}: {e}"))?;
        let _ = fs::write(rec.join("live-stderr.txt"), &o.stderr);
        resp = serde_json::from_slice(&o.stdout).unwrap_or(json!({}));
        code = o.status.code().unwrap_or(1);
        line["live_adapter"] = json!(c.plan["live"]["adapter"]);
        line["virtual_ms"] = resp["duration_ms"].clone();
        line["clock"] = json!("real");
        line["paid"] = json!(resp["dry"] != true && resp["adapter"] != "dry");
        resp["live_adapter"] = resp["adapter"].clone();
        resp["adapter"] = json!("live");
    } else {
        let x = chosen.unwrap();
        let map = if decider { revision_map(x, &cwd.join("in")) } else { vec![] };
        if !map.is_empty() {
            line["revisions_mapped"] = json!(map.iter().map(|(a, b)| format!("{a} -> {b}")).collect::<Vec<_>>());
        }
        let mapf = |t: &str| map.iter().fold(t.to_string(), |acc, (a, b)| acc.replace(a.as_str(), b));
        if !decider {
            clear_dir(&cwd.join("out"))?;
        }
        if let Some(o) = x["out"].as_str() {
            copy_tree(Path::new(o), &cwd.join("out"), &mapf)?;
        }
        if let Some(s) = x["structured"].as_str() {
            let t = fs::read_to_string(s).map_err(|e| e.to_string())?;
            fs::write(rec.join("structured-output.json"), mapf(&t)).map_err(|e| e.to_string())?;
        }
        resp = x["response"].clone();
        resp["source_adapter"] = resp["adapter"].clone();
        resp["adapter"] = json!(mode);
        resp["tool_calls"] = json!([]);
        resp["offered_tools"] = json!([]);
        resp["duration_ms"] = x["virtual_ms"].clone();
        line["virtual_ms"] = x["virtual_ms"].clone();
        code = resp["exit"].as_i64().unwrap_or(0) as i32;
        // The virtual clock: a recorded call that took longer than this run's timeout times out.
        let timeout_s = timeout_s_of(&c.out, &node);
        if let Some(t) = timeout_s.filter(|t| x["virtual_ms"].as_u64().unwrap_or(0) >= t * 1000) {
            if fault.is_none() {
                line["fault"] = json!(format!("virtual-timeout ({} ms recorded, timeout_s {t})", x["virtual_ms"]));
                code = timeout(&cwd, &mut resp, &mut line, t, x["virtual_ms"].as_u64().unwrap_or(0))?;
            }
        }
    }
    if let Some(f) = &fault {
        let v = f["value"].as_str().unwrap_or("");
        match f["fault"].as_str().unwrap() {
            "timeout" => {
                let t = timeout_s_of(&c.out, &node).unwrap_or(0);
                let ms = line["virtual_ms"].as_u64().unwrap_or(0);
                code = timeout(&cwd, &mut resp, &mut line, t, ms)?;
            }
            "invalid-reply" => {
                let bad = json!({"invalid": "an invalid reply injected by sdc-replay"});
                if decider || node == "change.review" {
                    let out_file = if node.starts_with("requester.") { "reply.json" } else if decider { "decision.json" } else { "review.json" };
                    clear_dir(&cwd.join("out"))?;
                    fs::write(cwd.join("out").join(out_file), bad.to_string()).map_err(|e| e.to_string())?;
                    fs::write(rec.join("structured-output.json"), bad.to_string()).map_err(|e| e.to_string())?;
                } else {
                    clear_dir(&cwd.join("out"))?;
                }
                code = 0;
                resp["exit"] = json!(0);
            }
            "exit" => {
                code = v.parse().unwrap_or(1);
                clear_dir(&cwd.join("out"))?;
                let _ = fs::remove_file(rec.join("structured-output.json"));
                resp["exit"] = json!(code);
                resp["is_error"] = json!(true);
                if code == 75 {
                    resp["terminal_reason"] = json!("api_error");
                    resp["api_error_status"] = json!("injected");
                }
            }
            "reject" => {
                if !node.starts_with("decide.") {
                    return Err(format!("reject@{name}: only a decider call can be replaced by a reject"));
                }
                let reason = if v.is_empty() { "a reject injected by sdc-replay in place of the recorded reply" } else { v };
                let r = json!({"decision": "reject", "answer": null, "best": null, "reasons": [reason], "cites": []});
                clear_dir(&cwd.join("out"))?;
                fs::write(cwd.join("out/decision.json"), r.to_string()).map_err(|e| e.to_string())?;
                fs::write(rec.join("structured-output.json"), r.to_string()).map_err(|e| e.to_string())?;
                code = 0;
                resp["exit"] = json!(0);
            }
            _ => {}
        }
    }
    finish(&mut line, &mut resp, code)
}

/// A call that ran into its timeout: nothing written, the driver told through
/// SDC_VIRTUAL_TIMEOUT, the timeout's seconds on the virtual clock.
fn timeout(cwd: &Path, resp: &mut Value, line: &mut Value, timeout_s: u64, recorded_ms: u64) -> R<i32> {
    clear_dir(&cwd.join("out"))?;
    let why = format!("the recorded call took {} s, timeout_s {timeout_s}", recorded_ms / 1000);
    match std::env::var("SDC_VIRTUAL_TIMEOUT") {
        Ok(f) => {
            let _ = fs::write(f, &why);
        }
        // No driver reads a file under `runbook run`: the program's output says it, as the driver did.
        Err(_) => eprintln!("sdc-replay: killed after {timeout_s} s (virtual clock: {why})"),
    }
    let ms = if timeout_s > 0 { timeout_s * 1000 } else { recorded_ms };
    line["virtual_ms"] = json!(ms);
    resp["duration_ms"] = json!(ms);
    resp["exit"] = json!(124);
    resp["is_error"] = json!(true);
    resp["terminal_reason"] = json!("timeout");
    Ok(124)
}

/// The timeout of an action on the virtual clock: `SDC_TIMEOUT_S` (the driver's), else the
/// `timeout_seconds` of the action in the run's bindings copy (`<run>/bindings/*.yaml`, where the
/// conditions of the plan were applied).
fn timeout_s_of(out: &Path, action: &str) -> Option<u64> {
    if let Some(t) = std::env::var("SDC_TIMEOUT_S").ok().and_then(|t| t.parse::<u64>().ok()) {
        return Some(t);
    }
    for file in files_under(&out.join("bindings")).into_iter().filter(|f| f.extension().is_some_and(|x| x == "yaml")) {
        let text = fs::read_to_string(&file).unwrap_or_default();
        let mut current: Option<&str> = None;
        for line in text.lines() {
            let t = line.trim_start().trim_start_matches("- ");
            if let Some(name) = t.strip_prefix("action:") {
                current = Some(name.trim().trim_matches(|c| c == '"' || c == '\''));
            } else if let (Some(v), true) = (t.strip_prefix("timeout_seconds:"), current == Some(action)) {
                if let Ok(n) = v.trim().parse() {
                    return Some(n);
                }
            }
        }
    }
    None
}

// --- programs ------------------------------------------------------------------------------------

fn program(action: &str, plan: Option<&Path>, argv: &[String]) -> R<i32> {
    let c = match plan {
        Some(p) => ctx_at(p)?,
        None => ctx()?,
    };
    // A program of `runbook run` names its case in LOOM_CASE; the driver's in SDC_CASE.
    let case = std::env::var("LOOM_CASE").or_else(|_| std::env::var("SDC_CASE")).unwrap_or_default();
    let log_p = c.out.join("replay/programs.jsonl");
    let log = jsonl(&log_p);
    let n_case = 1 + log.iter().filter(|l| l["case"] == case.as_str() && l["action"] == action).count() as u64;
    let n_run = 1 + log.iter().filter(|l| l["action"] == action).count() as u64;
    let take = c.plan["take_recorded"].as_array().is_some_and(|t| t.iter().any(|a| a == action || a == "all"));
    let fault = fault_for(&c.plan, &["check-fail"], &case, action, n_case, n_run).cloned();
    let mut line = json!({"at_ms": now_ms() as u64, "case": case, "action": action, "ordinal": n_case, "ordinal_run": n_run,
        "mode": if take { "recorded" } else { "real" }, "fault": fault.as_ref().map(|f| f["spec"].clone())});
    let code: i32 = if take {
        let src = load_source(Path::new(c.plan["source"]["path"].as_str().unwrap_or("")))?;
        let evs = src.events.get(&case).cloned().unwrap_or_default();
        let runs: Vec<&Value> = evs.iter().filter(|e| e["event"] == "action_finished" && e["action"] == action && !program_error(&e["exit"])).collect();
        let pick = runs.get(n_case as usize - 1).copied().or_else(|| {
            (c.plan["policy"] == "nearest").then(|| runs.last().copied()).flatten()
        });
        let Some(e) = pick else {
            let what = format!("the source ran {action} {} time(s) in {case}, this is run {n_case}", runs.len());
            write_first(&c.out.join("replay/diverged.json"), &json!({"format": "sdc.replay-divergence/1",
                "place": {"kind": "program", "case": case, "case_dir": std::env::var("SDC_CASE_DIR").ok(), "name": action, "ordinal": n_case}, "policy": c.plan["policy"],
                "difference": what, "summary": format!("program {action} in {case}: {what}")}))?;
            line["mode"] = json!("diverged");
            append(&log_p, &line)?;
            eprintln!("sdc-replay: recording diverged: {what}");
            return Ok(127);
        };
        if let (Some(d), Some(l)) = (src.case_dirs.get(&case), e["log"].as_str()) {
            let b = fs::read(d.join("blobs").join(l)).unwrap_or_default();
            std::io::stdout().write_all(&b).map_err(|e| e.to_string())?;
        }
        println!("\nsdc-replay: the recorded result of {action} run {n_case} in {case} (the program was not run)");
        match e["exit"].as_str() {
            Some("timeout") => {
                if let Ok(f) = std::env::var("SDC_VIRTUAL_TIMEOUT") {
                    let _ = fs::write(f, "the recorded run timed out");
                }
                124
            }
            Some(x) => x.parse().unwrap_or(1),
            None => 1,
        }
    } else {
        let st = Command::new(&argv[0]).args(&argv[1..]).status().map_err(|e| format!("{}: {e}", argv[0]))?;
        st.code().unwrap_or(1)
    };
    line["exit"] = json!(code);
    let code = match &fault {
        Some(f) => {
            println!("\nsdc-replay: check-fail injected ({}): the program exited {code}, recorded as 1", f["spec"].as_str().unwrap_or(""));
            1
        }
        None => code,
    };
    line["recorded_exit"] = json!(code);
    append(&log_p, &line)?;
    Ok(code)
}

// --- seal, compare, settings ---------------------------------------------------------------------

fn summary(out: &Path) -> R<Value> {
    let plan = read_json(&out.join("replay/plan.json"))?;
    let calls = jsonl(&out.join("replay/calls.jsonl"));
    let programs = jsonl(&out.join("replay/programs.jsonl"));
    let count = |m: &str| calls.iter().filter(|l| l["mode"] == m).count();
    let (recorded, reused, live, diverged) = (count("recorded"), count("reused"), count("live"), count("diverged"));
    let virtual_ms: u64 = calls.iter().filter(|l| l["clock"] == "virtual").filter_map(|l| l["virtual_ms"].as_u64()).sum();
    let label = if live > 0 {
        "re-driven: with live model calls"
    } else if reused > 0 {
        "re-driven: with reused replies"
    } else {
        "re-driven: recorded replies only"
    };
    Ok(json!({
        "format": "sdc.replay-status/1", "label": label,
        "source": plan["source"]["name"], "source_tree": plan["source"]["tree"], "tree": plan["tree"], "same_tree": plan["same_tree"],
        "policy": plan["policy"], "injections": plan["injections"].as_array().map(|a| a.iter().map(|f| f["spec"].clone()).collect::<Vec<_>>()),
        "conditions": plan["conditions"], "take_recorded": plan["take_recorded"],
        "calls": {"recorded": recorded, "reused": reused, "live": live, "diverged": diverged,
            "paid": calls.iter().filter(|l| l["paid"] == true).count(),
            "faults": calls.iter().filter(|l| l["fault"].is_string()).count(),
            "prompt_changed": calls.iter().filter(|l| l["prompt_changed"] == true).count()},
        "programs": {"real": programs.iter().filter(|l| l["mode"] == "real").count(),
            "recorded": programs.iter().filter(|l| l["mode"] == "recorded").count(),
            "check_fail": programs.iter().filter(|l| l["fault"].is_string()).count()},
        "virtual_seconds": (virtual_ms as f64 / 1000.0).round(),
        "diverged": read_json(&out.join("replay/diverged.json")).ok(),
    }))
}

fn seal(out: &Path) -> R<Value> {
    let out = canon(out);
    let s = summary(&out)?;
    write_json(&out.join("replay/summary.json"), &s)?;
    for f in ["status.json", "run-stopped.json"] {
        if let Ok(mut v) = read_json(&out.join(f)) {
            v["replay"] = s.clone();
            // `runbook run` ends a diverged re-drive by its own cause (`error`): the recording's.
            if let (Some(d), true) = (s["diverged"].as_object(), f == "run-stopped.json" && v["cause"] != "recording_diverged") {
                v["cause"] = json!("recording_diverged");
                v["reason"] = d.get("summary").cloned().unwrap_or(Value::Null);
            }
            write_json(&out.join(f), &v)?;
        }
    }
    // rounds.jsonl: the work calls' share of each round on the virtual clock, in `seconds` (decider
    // calls run between rounds, as in a real run).
    let calls = jsonl(&out.join("replay/calls.jsonl"));
    let rounds: Vec<Value> = jsonl(&out.join("rounds.jsonl"))
        .into_iter()
        .map(|mut r| {
            let real = r["real_seconds"].as_i64().or(r["seconds"].as_i64()).unwrap_or(0);
            let (a, b) = (r["started_epoch"].as_u64().unwrap_or(0) * 1000, (r["ended_epoch"].as_u64().unwrap_or(0) + 1) * 1000);
            let v: u64 = calls
                .iter()
                .filter(|l| l["kind"] == "work" && l["clock"] == "virtual" && (a..b).contains(&l["at_ms"].as_u64().unwrap_or(0)))
                .filter_map(|l| l["virtual_ms"].as_u64())
                .sum();
            let v = (v as f64 / 1000.0).round() as i64;
            r["real_seconds"] = json!(real);
            r["virtual_seconds"] = json!(v);
            r["seconds"] = json!(real + v);
            r["clock"] = json!("virtual");
            r
        })
        .collect();
    if out.join("rounds.jsonl").exists() {
        write_jsonl(&out.join("rounds.jsonl"), &rounds)?;
    }
    // Each call line names its mode (the call record's replay.json).
    for f in ["llm-calls.jsonl", "decisions.jsonl"] {
        let p = out.join(f);
        if !p.exists() {
            continue;
        }
        let lines: Vec<Value> = jsonl(&p)
            .into_iter()
            .map(|mut l| {
                if let Some(r) = l["record"].as_str() {
                    if let Ok(x) = read_json(&Path::new(r).join("replay.json")) {
                        l["replay"] = json!({"mode": x["mode"], "key": x["key"], "source": x["source"], "fault": x["fault"],
                            "clock": x["clock"], "virtual_ms": x["virtual_ms"], "prompt_changed": x["prompt_changed"]});
                    }
                }
                l
            })
            .collect();
        write_jsonl(&p, &lines)?;
    }
    Ok(s)
}

fn sequence(evs: &[Value]) -> Vec<String> {
    evs.iter()
        .filter(|e| e["event"] == "action_finished")
        .map(|e| {
            let mut s = format!("{} {}", e["action"].as_str().unwrap_or(""), e["exit"].as_str().unwrap_or("?"));
            if let Some(r) = e["result"].as_str() {
                s += &format!(" {r}");
            }
            if e["not_called"].is_string() {
                s += " not_called";
            }
            s
        })
        .collect()
}

fn compare(out: &Path) -> R<(Value, bool)> {
    let out = canon(out);
    let plan = read_json(&out.join("replay/plan.json"))?;
    let src = load_source(Path::new(plan["source"]["path"].as_str().unwrap_or("")))?;
    let now = run_cases(&out);
    let mut cases = Vec::new();
    let mut equal = true;
    let ids: BTreeSet<&String> = src.events.keys().chain(now.keys()).collect();
    for id in ids {
        let a = src.events.get(id).map(|e| sequence(e)).unwrap_or_default();
        let b = now.get(id).map(|(_, e)| sequence(e)).unwrap_or_default();
        let first = a.iter().zip(&b).position(|(x, y)| x != y).or((a.len() != b.len()).then(|| a.len().min(b.len())));
        equal &= first.is_none();
        cases.push(json!({"case": id, "equal": first.is_none(), "actions": b.len(), "source_actions": a.len(),
            "first_difference": first.map(|i| json!({"index": i, "source": a.get(i), "replay": b.get(i)}))}));
    }
    let end = |v: &Value| json!({"state": v["state"], "outcome": v["outcome"], "closed_by": v["closed_by"], "cause": v["cause"]});
    let now_status = read_json(&out.join("status.json")).or_else(|_| read_json(&out.join("run-stopped.json"))).unwrap_or(Value::Null);
    let same_end = end(&src.status) == end(&now_status);
    let r = json!({"equal": equal && same_end, "same_actions": equal, "same_end": same_end,
        "source_end": end(&src.status), "end": end(&now_status), "cases": cases});
    Ok((r, equal && same_end))
}

fn settings(out: &Path) -> R<Value> {
    let plan = read_json(&out.join("replay/plan.json"))?;
    Ok(json!({"source": plan["source"]["name"], "source_tree": plan["source"]["tree"], "tree": plan["tree"],
        "same_tree": plan["same_tree"], "policy": plan["policy"], "live": plan["live"]["adapter"], "live_cap": plan["live"]["cap"],
        "injections": plan["injections"].as_array().map(|a| a.iter().map(|f| f["spec"].clone()).collect::<Vec<_>>()),
        "conditions": plan["conditions"], "take_recorded": plan["take_recorded"], "clock": "virtual"}))
}

// --- pack ----------------------------------------------------------------------------------------

/// A recording that can be committed: the calls' keys and digests, their replies (out/ trees and
/// structured outputs) and the cases' event logs, with no prompt, transcript or absolute path.
fn pack(run: &Path, dest: &Path, source_tree: Option<String>) -> R<Value> {
    let src = load_source(run)?;
    if src.packed {
        return Err("already a recording".into());
    }
    let roots = vec![src.path.display().to_string()];
    clear_dir(dest)?;
    let mut calls = Vec::new();
    for (i, c) in src.calls.iter().enumerate() {
        let mut c = c.clone();
        let d = format!("calls/{:03}", i + 1);
        if let Some(o) = c["out"].as_str() {
            copy_tree(Path::new(o), &dest.join(&d).join("out"), &|t| t.to_string())?;
            c["out"] = json!(format!("{d}/out"));
        }
        if let Some(s) = c["structured"].as_str() {
            fs::create_dir_all(dest.join(&d)).map_err(|e| e.to_string())?;
            fs::copy(s, dest.join(&d).join("structured-output.json")).map_err(|e| e.to_string())?;
            c["structured"] = json!(format!("{d}/structured-output.json"));
        }
        calls.push(c);
    }
    for (id, evs) in &src.events {
        let lines: Vec<Value> = evs.iter().map(|e| serde_json::from_str(&normalise_paths(&e.to_string(), &roots)).unwrap()).collect();
        fs::create_dir_all(dest.join("events")).map_err(|e| e.to_string())?;
        write_jsonl(&dest.join("events").join(format!("{id}.jsonl")), &lines)?;
    }
    for (from, to) in [(&src.idea, "idea.yaml"), (&src.goals, "goals.yaml")] {
        if let Some(f) = from {
            fs::copy(f, dest.join(to)).map_err(|e| e.to_string())?;
        }
    }
    let settings: Vec<Value> = src
        .settings
        .iter()
        .map(|s| {
            let mut s = s.clone();
            if let Some(a) = s["adapter"].as_str() {
                s["adapter"] = json!(a.rsplit('/').next().unwrap_or(a));
            }
            serde_json::from_str(&normalise_paths(&s.to_string(), &roots)).unwrap()
        })
        .collect();
    let mut tree = src.tree.clone();
    if let Some(c) = source_tree {
        tree = json!({"commit": c, "dirty": null, "stated": true});
    }
    let status = json!({"state": src.status["state"], "outcome": src.status["outcome"], "delivered": src.status["delivered"],
        "closed_by": src.status["closed_by"], "cause": src.status["cause"]});
    let r = json!({"format": "sdc.recording/1", "name": src.path.file_name().map(|n| n.to_string_lossy().to_string()),
        "tree": tree, "status": status, "principal": src.principal, "settings": settings, "calls": calls});
    write_json(&dest.join("recording.json"), &r)?;
    // Nothing absolute may remain.
    for f in files_under(dest) {
        if let Ok(t) = fs::read_to_string(&f) {
            if t.contains("/home/") || t.contains(&roots[0]) {
                return Err(format!("{}: an absolute path remains", f.display()));
            }
        }
    }
    Ok(json!({"packed": dest.display().to_string(), "calls": r["calls"].as_array().map_or(0, |a| a.len()), "cases": src.events.len()}))
}

/// Paths only (the run root and home paths), for event logs kept as they are otherwise.
fn normalise_paths(text: &str, roots: &[String]) -> String {
    static HOME: OnceLock<Regex> = OnceLock::new();
    let mut t = text.to_string();
    for r in roots.iter().filter(|r| r.len() > 1) {
        t = t.replace(r.as_str(), "<run>");
    }
    re(&HOME, r#"/home/[^/\s"'`]+/[^\s"'`):,]*"#).replace_all(&t, "<path>").into_owned()
}

// --- main ----------------------------------------------------------------------------------------

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    // As an adapter it takes a response schema: the recorded reply is the schema document, written
    // to structured-output.json (sdc-reply asks every adapter program `--capabilities`).
    if args.len() == 2 && args[1] == "--capabilities" {
        println!("{}", json!({"json_schema": true}));
        return ExitCode::SUCCESS;
    }
    // The adapter contract: one argument, the request.
    if args.len() == 2 && args[1].ends_with("request.json") && Path::new(&args[1]).is_file() {
        return match adapter(Path::new(&args[1])) {
            Ok(c) => ExitCode::from(c.clamp(0, 255) as u8),
            Err(e) => {
                eprintln!("sdc-replay: {e}");
                ExitCode::from(1)
            }
        };
    }
    let cli = Cli::parse();
    let r: R<(Value, u8)> = match &cli.cmd {
        Cmd::Prepare { source, out, tree, source_tree, on_divergence, live_adapter, live_cap, inject, set, take_recorded, plan_only } => prepare(
            source, out, tree.as_deref(), source_tree.clone(), on_divergence, live_adapter, *live_cap, inject, set, take_recorded, !plan_only,
        )
        .map(|v| (v, 0)),
        Cmd::Env { out } => read_json(&out.join("replay/env.json")).map(|v| (v, 0)),
        Cmd::Settings { out } => settings(out).map(|v| (v, 0)),
        Cmd::Program { action, plan, argv } => {
            if argv.is_empty() {
                Err("program: no argv after --".into())
            } else {
                return match program(action, plan.as_deref(), argv) {
                    Ok(c) => ExitCode::from(c.clamp(0, 255) as u8),
                    Err(e) => {
                        eprintln!("sdc-replay: {e}");
                        ExitCode::from(127)
                    }
                };
            }
        }
        Cmd::Seal { run } => seal(run).map(|v| (v, 0)),
        Cmd::Compare { run } => compare(run).map(|(v, eq)| (v, u8::from(!eq))),
        Cmd::Pack { run, dest, source_tree } => pack(run, dest, source_tree.clone()).map(|v| (v, 0)),
        Cmd::Index { source } => load_source(source).map(|s| (json!(s.calls), 0)),
    };
    match r {
        Ok((v, c)) => {
            println!("{}", serde_json::to_string(&v).unwrap());
            ExitCode::from(c)
        }
        Err(e) => {
            eprintln!("sdc-replay: {e}");
            ExitCode::from(64)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalise_hides_what_differs_between_runs() {
        let roots = vec!["/r/run-a".to_string()];
        let a = normalise("at: 2026-11-21T10:30:34Z rev t3212d0e4d6079583 /r/run-a/x 0.12s", &roots);
        let b = normalise("at: 2026-11-22T01:02:03.5Z rev t0000000000000000 <run>/x 3s", &roots);
        assert_eq!(a, b);
        assert_eq!(normalise(concat!("/ho", "me/someone/x/y z"), &[]), "<path> z");
        let x = normalise("+++ a\t2026-11-21 12:06:20.144 +0100\n  | test b ... ok\n  | test a ... ok\nend", &[]);
        assert_eq!(x, "+++ a\t<instant>\n  | test a ... ok\n  | test b ... ok\nend");
    }

    #[test]
    fn the_condition_unstick_is_refused_by_name_in_a_set_and_in_a_plan() {
        let tools = Path::new("/nonexistent");
        for v in ["on", "off", "1"] {
            let e = parse_sets(&[format!("unstick={v}")], tools).unwrap_err();
            assert!(e.contains(&format!("--set unstick={v}")) && e.contains("no longer a condition"), "{e}");
        }
        let p = Path::new("replay/plan.json");
        for plan in [json!({"out": "/x", "conditions": {"unstick": "on"}}), json!({"out": "/x", "unstick": {"on": false}})] {
            let e = plan_refusal(&plan, p).expect("a plan that names unstick is refused");
            assert!(e.contains("no longer a condition") && e.contains("replay/plan.json"), "{e}");
        }
        assert!(plan_refusal(&json!({"out": "/x", "conditions": {"max_runs.implement": 2}}), p).is_none());
    }

    #[test]
    fn request_drops_paths_and_ordinals() {
        let r = normalise_request(&json!({"model": "m", "cwd": "/a", "attempt": 3, "review_attempt": 2, "node": "n"}), &[]);
        assert_eq!(r, json!({"model": "m", "node": "n"}));
    }

    #[test]
    fn arrival_and_try_follow_the_driver() {
        let writes: HashSet<String> = ["w".to_string()].into();
        let evs = vec![
            json!({"event": "revised"}),
            json!({"event": "action_started", "action": "w"}),
            json!({"event": "action_finished", "action": "w", "exit": "0", "principal": null}),
            json!({"event": "revised"}),
            json!({"event": "action_started", "action": "w"}),
            json!({"event": "action_finished", "action": "w", "exit": "1", "principal": null}),
            json!({"event": "action_started", "action": "w"}),
        ];
        assert_eq!(arrival_try(&evs, &writes, 1, "w"), (1, 1));
        // The revision w wrote is not an arrival; the second run is try 2, the third try 3.
        assert_eq!(arrival_try(&evs, &writes, 4, "w"), (1, 2));
        assert_eq!(arrival_try(&evs, &writes, 6, "w"), (1, 3));
    }

    #[test]
    fn faults_parse() {
        let f = parse_fault("timeout@IDEA-1.list.offline/change.implement#1").unwrap();
        assert_eq!((f["case"].as_str(), f["name"].as_str(), f["n"].as_u64()), (Some("IDEA-1.list.offline"), Some("change.implement"), Some(1)));
        let f = parse_fault("reject@merge.approve#1=tests miss a refusal").unwrap();
        assert_eq!((f["case"].clone(), f["value"].as_str()), (Value::Null, Some("tests miss a refusal")));
        assert!(parse_fault("exit@x#1").is_err());
        assert!(parse_fault("melt@x#1").is_err());
    }

    #[test]
    fn nearest_prefers_same_arrival_then_earlier() {
        let c = |a: u64, t: u64| json!({"kind": "work", "case": "c", "name": "n", "arrival": a, "try": t, "attempt": 1, "id": format!("{a}.{t}")});
        let calls = vec![c(1, 1), c(1, 2), c(2, 1)];
        let k = |a: u64, t: u64| json!({"kind": "work", "case": "c", "name": "n", "arrival": a, "try": t, "attempt": 1});
        assert_eq!(nearest(&calls, &k(1, 3)).unwrap()["id"], "1.2");
        assert_eq!(nearest(&calls, &k(2, 2)).unwrap()["id"], "2.1");
        assert_eq!(nearest(&calls, &k(3, 1)).unwrap()["id"], "2.1");
        assert!(nearest(&calls, &json!({"kind": "work", "case": "d", "name": "n", "arrival": 1, "try": 1})).is_none());
    }
}
