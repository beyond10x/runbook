//! The writer of the run directory files `sdc-experiment report` and `runbook parity summarize`
//! read, from the events of a Loom run. `docs/loom-run-directory.md` is the table of every file,
//! where each member comes from and what a Loom run does not record.
//!
//! Two kinds of file:
//!
//! - Append-only records, one line per event, written as the event arrives (`run-settings.jsonl`,
//!   `llm-calls.jsonl`, `decisions.jsonl`, `rounds.jsonl`). A run that is killed leaves every line
//!   written so far.
//! - A projection of Loom's own records, rewritten whole through a temporary file and a rename
//!   after every step, so a reader never sees half a file: the driver-shaped case store
//!   `store/<root case>/events.jsonl` (with `children/<case>/` below it, `observed.jsonl`,
//!   `evidence/`, `ir.json`, `bindings.json`) made from Loom's run logs
//!   (`store/runlog-case-<hex of the case id>.jsonl`) and `tree-status.json`, and
//!   `loom/status.json`, the status without absolute paths.
//!
//! Nothing here writes the content of a login file, a path under a home directory or an
//! environment: every value copied from an event is cleaned first (`Cleaner`).
//!
//! This file uses `super::events` and nothing else of the crate, so that the tests of the
//! integration-test binary (there is no library target) can include it as it is.

use super::events::{Cap, CostSource, Event, EventSink};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The placeholder `sdc-experiment` puts in a summary for a string holding an absolute path.
const REMOVED: &str = "<absolute path removed>";
/// Text of a string that holds one of these is replaced (the list of `sdc-experiment`'s summaries).
const PATH_MARKERS: [&str; 7] = ["/home/", "/Users/", "/root/", "/tmp/", "/var/", "/run/user/", "/dev/shm/"];
/// The format of a `tree status` document.
const STATUS_FORMAT: &str = "loom.tree-status/1";
/// The longest reason text written into `run-stopped.json`.
const REASON_MAX: usize = 600;

// ---------------------------------------------------------------------------------------- clock

/// `YYYY-MM-DDTHH:MM:SS[.fff]Z` of `ms` milliseconds since the Unix epoch.
fn rfc3339_ms(ms: u64, with_ms: bool) -> String {
    let secs = ms / 1000;
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    let base = format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}", rem / 3600, rem % 3600 / 60, rem % 60);
    if with_ms {
        format!("{base}.{:03}Z", ms % 1000)
    } else {
        format!("{base}Z")
    }
}

fn rfc3339(secs: u64) -> String {
    rfc3339_ms(secs * 1000, false)
}

// -------------------------------------------------------------------------------------- cleaner

/// Cleans what is copied from an event or from Loom into a file: a string that starts with the run
/// directory's path becomes relative to it, a string that still holds an absolute path of another
/// place is replaced, and the value of a member that names a login, a credential or a secret is
/// replaced by `named` whatever it holds.
struct Cleaner {
    prefixes: Vec<String>,
}

impl Cleaner {
    fn new(run_dir: &Path) -> Cleaner {
        let mut prefixes = vec![];
        for p in [Some(run_dir.to_path_buf()), fs::canonicalize(run_dir).ok()].into_iter().flatten() {
            let s = p.to_string_lossy().trim_end_matches('/').to_string();
            if s.starts_with('/') && !prefixes.contains(&s) {
                prefixes.push(s);
            }
        }
        prefixes.sort_by_key(|p| std::cmp::Reverse(p.len()));
        Cleaner { prefixes }
    }

    fn secret_key(key: &str) -> bool {
        let k = key.to_ascii_lowercase();
        ["login", "credential", "secret", "password", "passwd", "api_key", "apikey", "authorization", "bearer"]
            .iter()
            .any(|w| k.contains(w))
            || k == "token"
            || k.ends_with("_token")
            || k.ends_with("-token")
    }

    fn text(&self, s: &str) -> String {
        let mut t = s.to_string();
        for p in &self.prefixes {
            if t == *p {
                t = ".".into();
            } else {
                t = t.replace(&format!("{p}/"), "");
            }
        }
        if PATH_MARKERS.iter().any(|m| t.contains(m)) {
            REMOVED.to_string()
        } else {
            t
        }
    }

    fn value(&self, v: &Value) -> Value {
        match v {
            Value::String(s) => Value::String(self.text(s)),
            Value::Array(a) => Value::Array(a.iter().map(|x| self.value(x)).collect()),
            Value::Object(m) => Value::Object(
                m.iter()
                    .map(|(k, x)| (k.clone(), if Self::secret_key(k) { Value::String("named".into()) } else { self.value(x) }))
                    .collect(),
            ),
            other => other.clone(),
        }
    }
}

// ------------------------------------------------------------------------------ small file work

fn io(what: &Path, e: std::io::Error) -> String {
    format!("{}: {e}", what.display())
}

/// One line appended to `path` with one write: a killed process leaves whole lines.
fn append_line(path: &Path, v: &Value) -> Result<(), String> {
    let mut f = fs::OpenOptions::new().create(true).append(true).open(path).map_err(|e| io(path, e))?;
    let mut line = serde_json::to_string(v).map_err(|e| e.to_string())?;
    line.push('\n');
    f.write_all(line.as_bytes()).map_err(|e| io(path, e))?;
    f.flush().map_err(|e| io(path, e))
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

fn hex(s: &str) -> String {
    s.bytes().map(|b| format!("{b:02x}")).collect()
}

/// A case id is a directory name below `store/`: no separator, never `.` or `..`.
fn component(id: &str) -> Result<&str, String> {
    if id.is_empty() || id == "." || id == ".." || id.contains(['/', '\\', '\0']) {
        Err(format!("the case id {id:?} is not a directory name"))
    } else {
        Ok(id)
    }
}

/// `CamelCase` of Loom's stop names as the summary's `snake_case`; text that has no capital stays.
fn snake(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

// ---------------------------------------------------------------------- what the run files hold

/// What is read of a protocol file and of a bindings file: the facts the report names a node by.
#[derive(Default, Clone)]
struct ProtocolFacts {
    /// `protocol.id` of the protocol file, as the driver names it (`sdc.intake`).
    id: Option<String>,
    /// action -> `effect` of the protocol file (`write`, `read`).
    effects: BTreeMap<String, String>,
    /// Actions bound to `sdc-node`.
    llm: HashSet<String>,
    /// Actions bound to a person.
    manual: HashSet<String>,
    /// action -> source protocol of a copy binding.
    copies: BTreeMap<String, String>,
}

/// The registration stem of a protocol id: `.` and `_` written as `-`.
fn stem(id: &str) -> String {
    id.replace(['.', '_'], "-")
}

/// Facts by registration stem, from `protocols/*.protocol.yaml` and `bindings/*.yaml` of the run
/// directory.
fn load_facts(run_dir: &Path) -> HashMap<String, ProtocolFacts> {
    let mut by_stem: HashMap<String, ProtocolFacts> = HashMap::new();
    let yaml_files = |dir: &str| -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = fs::read_dir(run_dir.join(dir))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "yaml"))
            .collect();
        v.sort();
        v
    };
    for p in yaml_files("protocols") {
        let Some(doc) = fs::read_to_string(&p).ok().and_then(|t| serde_yaml::from_str::<serde_yaml::Value>(&t).ok()) else { continue };
        let Some(id) = doc["protocol"]["id"].as_str() else { continue };
        let f = by_stem.entry(stem(id)).or_default();
        f.id = Some(id.to_string());
        if let Some(actions) = doc["actions"].as_mapping() {
            for (name, a) in actions {
                if let (Some(name), Some(effect)) = (name.as_str(), a["effect"].as_str()) {
                    f.effects.insert(name.to_string(), effect.to_string());
                }
            }
        }
    }
    for p in yaml_files("bindings") {
        let Some(doc) = fs::read_to_string(&p).ok().and_then(|t| serde_yaml::from_str::<serde_yaml::Value>(&t).ok()) else { continue };
        let Some(protocol) = doc["protocol"].as_str() else { continue };
        let name = protocol.split('@').next().unwrap_or(protocol);
        let f = by_stem.entry(stem(name)).or_default();
        for a in doc["actions"].as_sequence().into_iter().flatten() {
            let Some(action) = a["action"].as_str() else { continue };
            match a["executor"]["kind"].as_str() {
                Some("person") => {
                    f.manual.insert(action.to_string());
                }
                Some("copy") => {
                    f.copies.insert(action.to_string(), a["executor"]["from"].as_str().unwrap_or("").to_string());
                }
                Some("program") => {
                    let first = a["executor"]["argv"][0].as_str().unwrap_or("");
                    if first.rsplit('/').next() == Some("sdc-node") {
                        f.llm.insert(action.to_string());
                    }
                }
                _ => {}
            }
        }
    }
    by_stem
}

// ------------------------------------------------------------------------------------ run logs

/// One line of a case's run log the projection uses.
#[derive(Clone, Debug)]
enum LineEvent {
    Opened,
    Finished { action: String, exit: i64, evidence: Vec<String>, principal: Option<String> },
    Terminated { outcome: String },
}

#[derive(Clone, Debug)]
struct Line {
    seq: u64,
    at: String,
    event: LineEvent,
}

/// A case as its run log tells it.
#[derive(Debug, Default)]
struct CaseLog {
    protocol: String,
    lines: Vec<Line>,
    children: Vec<String>,
}

/// Reads `store/runlog-case-<hex>.jsonl`; a line that does not parse (the last one of a killed
/// run) is left out. `None` when the file does not exist.
fn read_runlog(store: &Path, case: &str) -> Option<CaseLog> {
    let text = fs::read_to_string(store.join(format!("runlog-case-{}.jsonl", hex(case)))).ok()?;
    let mut log = CaseLog::default();
    let mut pending: Option<String> = None;
    for (i, raw) in text.lines().enumerate() {
        let Ok(v) = serde_json::from_str::<Value>(raw) else { continue };
        let seq = v["seq"].as_u64().unwrap_or(i as u64);
        let at = v["at"].as_str().unwrap_or("").to_string();
        match v["kind"].as_str().unwrap_or("") {
            "CaseOpened" => {
                log.protocol = v["protocol"].as_str().unwrap_or("").to_string();
                log.lines.push(Line { seq, at, event: LineEvent::Opened });
            }
            "AuthorityRecorded" => {
                if v["decision"] == "Allow" {
                    pending = v["principal"].as_str().map(String::from);
                }
            }
            "Evaluated" | "Stopped" => pending = None,
            "ActionFinished" => {
                let program = v.get("blob_digest").is_some_and(|b| !b.is_null());
                let principal = if program { None } else { pending.take() };
                log.lines.push(Line {
                    seq,
                    at,
                    event: LineEvent::Finished {
                        action: v["action"].as_str().unwrap_or("").to_string(),
                        exit: v["exit"].as_i64().unwrap_or(-1),
                        evidence: v["evidence"].as_array().into_iter().flatten().filter_map(|e| e.as_str().map(String::from)).collect(),
                        principal,
                    },
                });
            }
            "Terminated" => log.lines.push(Line {
                seq,
                at,
                event: LineEvent::Terminated { outcome: v["reason"].as_str().unwrap_or("").to_string() },
            }),
            "ChildOpened" => {
                if let Some(c) = v["child"].as_str() {
                    log.children.push(c.to_string());
                }
            }
            _ => {}
        }
    }
    Some(log)
}

// ---------------------------------------------------------------------------------- the writer

/// What `Ended` told: the run closed through the tree's outcome, or it did not.
struct EndNote {
    tree_outcome: Option<String>,
}

/// Writes the run directory's report files as events arrive.
pub struct RunDirWriter {
    run_dir: PathBuf,
    clean: Cleaner,
    /// The settings as given, cleaned.
    settings: Value,
    /// The root case id: the settings' `case`, else Loom's `store/tree.json`, else the status.
    root: Option<String>,
    /// Model name of the decider from the settings, for lines that name no model.
    decider_model: Option<String>,
    /// Model name of the last costed decider call.
    last_decider_model: Option<String>,
    /// The time of the last event that ended a stretch of work, seconds.
    last_at: u64,
    /// When the open decision was asked.
    asked_at: Option<u64>,
    /// First sight of a run log line: (case, seq) -> (start, end) of the step it appeared in.
    seen: HashMap<(String, u64), (u64, u64)>,
    /// Content of every projected file as last written (a file is rewritten only when it changes).
    written: HashMap<PathBuf, Vec<u8>>,
    /// Facts of the protocols and bindings, loaded when a projection finds them.
    facts: HashMap<String, ProtocolFacts>,
    /// The case of the most recent new run log line.
    last_case: Option<String>,
    end: Option<EndNote>,
    stopped_written: bool,
}

impl RunDirWriter {
    /// Opens the writer for `run_dir` (it exists; R1's files are in it). `settings` has the login
    /// path replaced by `named`.
    pub fn open(run_dir: &Path, settings: &Value) -> Result<RunDirWriter, String> {
        if !run_dir.is_dir() {
            return Err(format!("{}: the run directory does not exist", run_dir.display()));
        }
        if run_dir.join("free-agent.json").exists() {
            return Err(format!("{}: free-agent.json would make the report read a free-agent run", run_dir.display()));
        }
        let clean = Cleaner::new(run_dir);
        let settings = clean.value(settings);
        if !settings.is_object() {
            return Err("the settings are not a JSON object".into());
        }
        // A run records every costed call and every answer: an empty file says "none yet", not
        // "never recorded".
        for f in ["llm-calls.jsonl", "decisions.jsonl"] {
            let p = run_dir.join(f);
            fs::OpenOptions::new().create(true).append(true).open(&p).map_err(|e| io(&p, e))?;
        }
        fs::create_dir_all(run_dir.join("store")).map_err(|e| io(&run_dir.join("store"), e))?;
        let decider_model = settings["decider"]["model"].as_str().filter(|m| !m.is_empty()).map(String::from);
        let root = settings["case"].as_str().map(String::from);
        Ok(RunDirWriter {
            run_dir: run_dir.to_path_buf(),
            clean,
            settings,
            root,
            decider_model,
            last_decider_model: None,
            last_at: 0,
            asked_at: None,
            seen: HashMap::new(),
            written: HashMap::new(),
            facts: HashMap::new(),
            last_case: None,
            end: None,
            stopped_written: false,
        })
    }

    fn path(&self, name: &str) -> PathBuf {
        self.run_dir.join(name)
    }

    /// Replaces `path` by `content` through a temporary file and a rename; nothing is written
    /// when the file already holds it.
    fn put(&mut self, path: &Path, content: Vec<u8>) -> Result<(), String> {
        if self.written.get(path).is_some_and(|c| *c == content) {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| io(parent, e))?;
        }
        let mut tmp = path.as_os_str().to_os_string();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        fs::write(&tmp, &content).map_err(|e| io(&tmp, e))?;
        fs::rename(&tmp, path).map_err(|e| io(path, e))?;
        self.written.insert(path.to_path_buf(), content);
        Ok(())
    }

    fn put_json(&mut self, path: &Path, v: &Value) -> Result<(), String> {
        let mut t = serde_json::to_string_pretty(v).map_err(|e| e.to_string())?;
        t.push('\n');
        self.put(path, t.into_bytes())
    }

    fn put_lines(&mut self, path: &Path, lines: &[Value]) -> Result<(), String> {
        let mut t = String::new();
        for l in lines {
            t.push_str(&serde_json::to_string(l).map_err(|e| e.to_string())?);
            t.push('\n');
        }
        self.put(path, t.into_bytes())
    }

    // ------------------------------------------------------------------ the append-only files

    fn run_started(&mut self, at: u64) -> Result<(), String> {
        self.last_at = at;
        let mut line = Map::new();
        line.insert("format".into(), json!("sdc.run-settings/1"));
        line.insert("at".into(), json!(rfc3339(at)));
        line.insert("leg".into(), json!("loom"));
        for (k, v) in self.settings.as_object().into_iter().flatten() {
            if !matches!(k.as_str(), "format" | "at" | "leg") {
                line.insert(k.clone(), v.clone());
            }
        }
        append_line(&self.path("run-settings.jsonl"), &Value::Object(line))
    }

    fn step_made(&mut self, at: u64, stop: &str) -> Result<(), String> {
        let start = self.last_at.min(at);
        let line = json!({
            "started": rfc3339(start), "ended": rfc3339(at),
            "started_epoch": start, "ended_epoch": at, "seconds": at - start,
            "verdict": snake(stop), "case_dir": "",
        });
        append_line(&self.path("rounds.jsonl"), &line)?;
        self.project((start, at))?;
        self.last_at = at;
        Ok(())
    }

    fn costed_step(&mut self, cost_micro_usd: u64, record: &Value) -> Result<(), String> {
        let Value::Object(map) = self.clean.value(record) else {
            return Err("a cost record of a step is not a JSON object".into());
        };
        let mut line = Map::new();
        for (k, v) in map {
            line.insert(k, v);
        }
        // The ledger's number is the one that was booked.
        line.insert("cost_usd".into(), json!(cost_micro_usd as f64 / 1e6));
        append_line(&self.path("llm-calls.jsonl"), &Value::Object(line))
    }

    fn decision_answered(&mut self, at: u64, decision: &Value, approve: bool, principal: &str, cost_micro_usd: u64, model_calls: u64) -> Result<(), String> {
        let d = self.clean.value(decision);
        let kind = d["kind"].as_str().unwrap_or("");
        let class = if kind.eq_ignore_ascii_case("close") { "close" } else { "approval" };
        let model = if model_calls > 0 {
            Some(self.last_decider_model.take().or_else(|| self.decider_model.clone()).unwrap_or_else(|| "unknown".into()))
        } else {
            None
        };
        let mut line = Map::new();
        line.insert("time".into(), json!(rfc3339(at)));
        line.insert("case".into(), d["case"].clone());
        line.insert("type".into(), json!(class));
        line.insert("class".into(), json!(class));
        line.insert("action".into(), d["action"].clone());
        line.insert("capability".into(), d["capability"].clone());
        line.insert("kind".into(), d["kind"].clone());
        line.insert("revision".into(), d["revision"].clone());
        line.insert("subject".into(), d["subject"].clone());
        line.insert("decision".into(), json!(if approve { "approve" } else { "reject" }));
        line.insert("principal".into(), json!(self.clean.text(principal)));
        line.insert("model".into(), model.map_or(Value::Null, Value::String));
        line.insert("model_calls".into(), json!(model_calls));
        line.insert("cost_usd".into(), json!(cost_micro_usd as f64 / 1e6));
        line.insert("valid".into(), json!(true));
        append_line(&self.path("decisions.jsonl"), &Value::Object(line))
    }

    /// One line of the decider's answer to a request of the answers file, in the driver's format
    /// (the line is built by `answers::decision_line`); `time` is the runner's clock.
    fn answer_recorded(&mut self, at: u64, line: &Value) -> Result<(), String> {
        let Value::Object(map) = self.clean.value(line) else {
            return Err("an answer line is not a JSON object".into());
        };
        let mut out = Map::new();
        out.insert("time".into(), json!(rfc3339(at)));
        for (k, v) in map {
            out.insert(k, v);
        }
        append_line(&self.path("decisions.jsonl"), &Value::Object(out))
    }

    // ------------------------------------------------------------------- stop and end records

    fn write_stop(&mut self, at: u64, cause: &str, reason: &str) -> Result<(), String> {
        if self.stopped_written || self.path("run-stopped.json").exists() {
            return Ok(());
        }
        let case = self.last_case.clone().or_else(|| self.root.clone());
        let mut reason = self.clean.text(reason);
        reason.truncate(REASON_MAX);
        let v = json!({
            "format": "sdc.run-stopped/1", "at": rfc3339(at), "by": "rule:loom-run",
            "reason": reason, "cause": cause, "verdict_at_stop": null, "case": case,
        });
        self.put_json(&self.path("run-stopped.json"), &v)?;
        self.stopped_written = true;
        Ok(())
    }

    fn ended(&mut self, at: u64, outcome: &str, reason: Option<&str>) -> Result<(), String> {
        if outcome == "ended" {
            self.end = Some(EndNote { tree_outcome: reason.map(String::from) });
        }
        self.project((self.last_at.min(at), at))?;
        self.last_at = at;
        if outcome == "ended" {
            let tree = self.end.as_ref().and_then(|e| e.tree_outcome.clone());
            let tree = tree.or_else(|| self.root_outcome());
            match tree {
                Some(o) => {
                    let delivered = matches!(o.as_str(), "operating" | "fixed" | "delivered");
                    let v = json!({
                        "format": "sdc.run-status/1", "at": rfc3339(at), "state": "closed",
                        "outcome": o, "delivered": delivered, "note": null, "closed_by": null,
                    });
                    self.put_json(&self.path("status.json"), &v)?;
                }
                None => self.write_stop(at, "ended_without_outcome", "the tree ended and named no outcome")?,
            }
            return Ok(());
        }
        let cause = match outcome {
            "stuck" => "stuck",
            "stopped: spend cap" => "spend_cap",
            "stopped: wall time" => "wall_time_spent",
            "stopped: llm calls" => "llm_calls_spent",
            "stopped: signal" => "signal",
            "stopped: decider" => "decider_cannot_decide",
            "stopped: rejected" => "rejected_without_rework_path",
            _ => "error",
        };
        let reason = match reason {
            // The reason of a rejection names the step and the case first, as the driver's did.
            Some(r) if cause == "rejected_without_rework_path" => r.to_owned(),
            Some(r) => format!("{outcome}: {r}"),
            None => outcome.to_string(),
        };
        self.write_stop(at, cause, &reason)
    }

    /// The outcome of the root case as its run log tells it.
    fn root_outcome(&self) -> Option<String> {
        let root = self.root.clone()?;
        let log = read_runlog(&self.path("store"), &root)?;
        log.lines.iter().rev().find_map(|l| match &l.event {
            LineEvent::Terminated { outcome } => Some(outcome.clone()),
            _ => None,
        })
    }

    // ---------------------------------------------------------------------------- loom/ files

    /// `loom/status.json`: `tree-status.json` with its absolute paths made relative to the run
    /// directory, and `loom/verify.json`.
    fn store_loom_files(&mut self) -> Result<(), String> {
        if let Some(status) = read_json(&self.path("tree-status.json")).filter(|s| s["format"] == STATUS_FORMAT) {
            let mut roots: Vec<(String, &str)> = vec![];
            if let Some(s) = status["store"].as_str() {
                roots.push((s.trim_end_matches('/').to_string(), "store"));
            }
            if let Some(w) = status["workspace"].as_str() {
                roots.push((w.trim_end_matches('/').to_string(), "work"));
            }
            let rebased = rebase(&status, &roots);
            let cleaned = self.clean.value(&rebased);
            self.put_json(&self.path("loom/status.json"), &cleaned)?;
        }
        if let Some(v) = read_json(&self.path("tree-verify.json")) {
            let cleaned = self.clean.value(&v);
            self.put_json(&self.path("loom/verify.json"), &cleaned)?;
        }
        Ok(())
    }

    // ------------------------------------------------------------------------- the projection

    fn root_id(&mut self, status: Option<&Value>) -> Option<String> {
        if self.root.is_none() {
            self.root = read_json(&self.path("store/tree.json"))
                .and_then(|t| t["root"].as_str().map(String::from))
                .or_else(|| status.and_then(|s| s["root"].as_str().map(String::from)));
        }
        self.root.clone()
    }

    /// Rewrites the driver-shaped case store from Loom's run logs and the status. `window` is the
    /// span of the step whose end brought the new lines.
    fn project(&mut self, window: (u64, u64)) -> Result<(), String> {
        let status = read_json(&self.path("tree-status.json")).filter(|s| s["format"] == STATUS_FORMAT);
        let Some(root) = self.root_id(status.as_ref()) else { return Ok(()) };
        component(&root)?;
        let store = self.path("store");
        if self.facts.is_empty() {
            self.facts = load_facts(&self.run_dir);
        }
        let facts = self.facts.clone();

        // The cases, parents first, from the run logs.
        let mut order: Vec<(String, Option<String>, CaseLog)> = vec![];
        let mut queue = vec![(root.clone(), None::<String>)];
        let mut visited: HashSet<String> = HashSet::new();
        while let Some((id, parent)) = queue.pop() {
            if !visited.insert(id.clone()) {
                continue;
            }
            component(&id)?;
            let Some(log) = read_runlog(&store, &id) else { continue };
            let mut kids: Vec<(String, Option<String>)> = log.children.iter().map(|c| (c.clone(), Some(id.clone()))).collect();
            kids.reverse();
            queue.extend(kids);
            order.push((id, parent, log));
        }
        if order.is_empty() {
            return Ok(());
        }

        let status_cases: HashMap<String, &Value> = status
            .as_ref()
            .and_then(|s| s["cases"].as_array())
            .into_iter()
            .flatten()
            .filter_map(|c| c["case"].as_str().map(|id| (id.to_string(), c)))
            .collect();

        let mut dirs: HashMap<String, PathBuf> = HashMap::new();
        for (id, parent, log) in &order {
            let dir = match parent.as_ref().and_then(|p| dirs.get(p)) {
                Some(pd) => pd.join("children").join(id),
                None => store.join(id),
            };
            dirs.insert(id.clone(), dir.clone());

            // New lines get the window of this step.
            for l in &log.lines {
                if let std::collections::hash_map::Entry::Vacant(e) = self.seen.entry((id.clone(), l.seq)) {
                    e.insert(window);
                    self.last_case = Some(id.clone());
                }
            }

            let (protocol_stem, revision) = match log.protocol.split_once('@') {
                Some((n, r)) => (n.to_string(), r.to_string()),
                None => (log.protocol.clone(), String::new()),
            };
            let pf = facts.get(&protocol_stem).cloned().unwrap_or_default();
            let protocol_id = pf.id.clone().unwrap_or_else(|| driver_id(&protocol_stem));
            let sc = status_cases.get(id).copied();
            let runs: Vec<&Value> = sc.and_then(|c| c["runs"].as_array()).into_iter().flatten().collect();

            let mut events: Vec<Value> = vec![];
            let mut observed: Vec<Value> = vec![];
            let mut program_index = 0usize;
            let mut terminated = false;
            for l in &log.lines {
                let w = self.seen.get(&(id.clone(), l.seq)).copied().unwrap_or(window);
                match &l.event {
                    LineEvent::Opened => {
                        let rev: Value = revision.parse::<i64>().map_or_else(|_| json!(revision), |n| json!(n));
                        emit(&mut events, &mut observed, json!({"event": "opened", "at": l.at, "protocol": {"id": protocol_id, "revision": rev}}), w.0 * 1000);
                    }
                    LineEvent::Finished { action, exit, evidence, principal } => {
                        let is_program = principal.is_none() && !pf.manual.contains(action) && !pf.copies.contains_key(action);
                        let retry_of: Vec<Value> = if is_program {
                            let r = runs.get(program_index).filter(|r| r["action"] == action.as_str());
                            program_index += 1;
                            r.and_then(|r| r["retry_of"].as_array()).cloned().unwrap_or_default()
                        } else {
                            vec![]
                        };
                        let llm = pf.llm.contains(action);
                        emit(
                            &mut events,
                            &mut observed,
                            json!({"event": "action_started", "at": l.at, "action": action, "llm": llm, "retry_of": retry_of}),
                            w.0 * 1000,
                        );
                        let mut fin = json!({"event": "action_finished", "at": l.at, "action": action, "exit": exit.to_string(), "evidence": evidence});
                        if let Some(p) = principal {
                            fin["principal"] = json!(self.clean.text(p));
                        }
                        emit(&mut events, &mut observed, fin, w.1 * 1000);
                    }
                    LineEvent::Terminated { outcome } => {
                        terminated = true;
                        emit(&mut events, &mut observed, json!({"event": "terminated", "at": l.at, "outcome": outcome}), w.1 * 1000);
                    }
                }
            }
            if !terminated && *id == root {
                if let Some(o) = self.end.as_ref().and_then(|e| e.tree_outcome.clone()) {
                    emit(&mut events, &mut observed, json!({"event": "terminated", "at": rfc3339(window.1), "outcome": o}), window.1 * 1000);
                }
            }
            self.put_lines(&dir.join("events.jsonl"), &events)?;
            self.put_lines(&dir.join("observed.jsonl"), &observed)?;

            // Node kinds: the protocol's effects, the bindings' person and copy actions.
            if !pf.effects.is_empty() {
                let actions: Map<String, Value> = pf.effects.iter().map(|(a, e)| (a.clone(), json!({"effect": e}))).collect();
                self.put_json(&dir.join("ir.json"), &json!({"actions": actions}))?;
            }
            if !pf.manual.is_empty() || !pf.copies.is_empty() {
                let mut b: BTreeMap<String, Value> = BTreeMap::new();
                for a in pf.manual.iter() {
                    b.insert(a.clone(), json!({"manual": true}));
                }
                for (a, from) in &pf.copies {
                    b.insert(a.clone(), json!({"copy": {"from": from}}));
                }
                self.put_json(&dir.join("bindings.json"), &json!(b))?;
            }
            // Evidence kind and result, from the status.
            for e in sc.and_then(|c| c["evidence"].as_array()).into_iter().flatten() {
                let Some(eid) = e["evidence_id"].as_str() else { continue };
                component(eid)?;
                let mut rec = Map::new();
                rec.insert("format".into(), json!("canon-evidence/1"));
                rec.insert("id".into(), json!(eid));
                for k in ["kind", "result", "subject", "subject_revision", "producer", "action"] {
                    if !e[k].is_null() {
                        rec.insert(k.into(), self.clean.value(&e[k]));
                    }
                }
                self.put_json(&dir.join("evidence").join(format!("{eid}.json")), &Value::Object(rec))?;
            }
            if *id == root {
                let findings = self.path("work/findings.yaml");
                if let Ok(bytes) = fs::read(&findings) {
                    self.put(&dir.join("work").join("findings.yaml"), bytes)?;
                }
            }
        }
        Ok(())
    }
}

/// Appends one derived event and the time it was observed.
fn emit(events: &mut Vec<Value>, observed: &mut Vec<Value>, mut e: Value, at_ms: u64) {
    let i = events.len();
    e["seq"] = json!(i);
    e["source"] = json!("loom-runlog");
    observed.push(json!({"index": i, "event": e["event"], "observed_at": rfc3339_ms(at_ms, true), "format": "sdc.observed/1"}));
    events.push(e);
}

/// The driver's id for a registration stem when no protocol file names it: `sdc` and
/// `sdc-<stage>` read back as `sdc` and `sdc.<stage>`.
fn driver_id(stem: &str) -> String {
    match stem.strip_prefix("sdc-") {
        Some(rest) => format!("sdc.{rest}"),
        None => stem.to_string(),
    }
}

/// `v` with every string that starts with one of the roots replaced by the name and the rest.
fn rebase(v: &Value, roots: &[(String, &str)]) -> Value {
    match v {
        Value::String(s) => {
            for (root, name) in roots {
                if s == root {
                    return Value::String((*name).to_string());
                }
                if let Some(rest) = s.strip_prefix(&format!("{root}/")) {
                    return Value::String(format!("{name}/{rest}"));
                }
            }
            v.clone()
        }
        Value::Array(a) => Value::Array(a.iter().map(|x| rebase(x, roots)).collect()),
        Value::Object(m) => Value::Object(m.iter().map(|(k, x)| (k.clone(), rebase(x, roots))).collect()),
        other => other.clone(),
    }
}

impl EventSink for RunDirWriter {
    fn event(&mut self, at_secs: u64, event: &Event) -> Result<(), String> {
        match event {
            Event::RunStarted { .. } => self.run_started(at_secs),
            Event::StepMade { stop, .. } => self.step_made(at_secs, stop),
            Event::DecisionAsked { .. } => {
                self.asked_at = Some(at_secs);
                Ok(())
            }
            Event::DecisionAnswered { decision, approve, principal, cost_micro_usd, model_calls } => {
                self.decision_answered(at_secs, decision, *approve, principal, *cost_micro_usd, *model_calls)?;
                let start = self.asked_at.take().unwrap_or(self.last_at).min(at_secs);
                self.project((start, at_secs))?;
                self.last_at = at_secs;
                Ok(())
            }
            Event::AnswerRecorded { line } => self.answer_recorded(at_secs, line),
            Event::DeclaredTaken { .. } => {
                let start = self.asked_at.take().unwrap_or(self.last_at).min(at_secs);
                self.project((start, at_secs))?;
                self.last_at = at_secs;
                Ok(())
            }
            Event::CostedCall { source: CostSource::Step, cost_micro_usd, record } => self.costed_step(*cost_micro_usd, record),
            Event::CostedCall { source: CostSource::Decider, record, .. } => {
                self.last_decider_model = record["model"].as_str().filter(|m| !m.is_empty()).map(String::from);
                Ok(())
            }
            Event::NoModelCall { .. } => Ok(()),
            Event::CapReached { cap, detail } => {
                let cause = match cap {
                    Cap::Spend => "spend_cap",
                    Cap::LlmCalls => "llm_calls_spent",
                    Cap::WallTime => "wall_time_spent",
                };
                self.write_stop(at_secs, cause, detail)
            }
            Event::TreeStored { .. } => {
                self.project((self.last_at.min(at_secs), at_secs))?;
                self.store_loom_files()
            }
            Event::Ended { outcome, reason, .. } => self.ended(at_secs, outcome, reason.as_deref()),
        }
    }
}
