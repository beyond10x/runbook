//! Attribution: which action and which tool call was in flight when a change happened.
//!
//! - Action window, exact by order: every case log (`store/**/events.jsonl`) is read as it grows,
//!   and each `action_started`/`action_finished` gets the ordinal of the inotify event that showed
//!   it; a change gets the ordinal of its first inotify event. Lines already there at start get 0.
//! - Tool window, inferred by time: the agent call's metaharness stream (`<record>/events.jsonl`)
//!   gives `tool.requested`/`tool.result` with vendor timestamps; a change belongs to the tool call
//!   whose [requested, result] window holds the change's wall time.
//! - Step window, for a Loom run (`runbook run`), exact by order: Loom's run log has one line per
//!   action, written at its finish, so no action start is known while a step runs. The runner
//!   appends one line to `rounds.jsonl` when a `tree run` step ends; a change belongs to the step
//!   after the lines already there (`step` = lines + 1). The call record open at that moment
//!   (`<record>/status.json` not yet finished or failed) gives the action (its directory name),
//!   the case (its `request.json`) and the tool window; with none open, case and action are null.

use crate::layout;
use crate::timefmt::parse_ms;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// A tool call's write lands after its request is recorded; allow this much clock skew.
pub const REQ_SLACK_MS: i64 = 50;
/// The watcher sees a write a little after it happened; the window stays open this long after
/// the result, unless a later call has started.
pub const RES_SLACK_MS: i64 = 250;

/// Reads the complete lines appended to a file since the last read.
pub struct Tail {
    path: PathBuf,
    offset: u64,
    partial: Vec<u8>,
}

impl Tail {
    pub fn new(path: PathBuf) -> Tail {
        Tail { path, offset: 0, partial: Vec::new() }
    }
    pub fn lines(&mut self) -> Vec<String> {
        let Ok(mut f) = fs::File::open(&self.path) else { return Vec::new() };
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        if len < self.offset {
            self.offset = 0;
            self.partial.clear();
        }
        if len == self.offset || f.seek(SeekFrom::Start(self.offset)).is_err() {
            return Vec::new();
        }
        let mut buf = Vec::new();
        let n = f.read_to_end(&mut buf).unwrap_or(0);
        self.offset += n as u64;
        self.partial.extend_from_slice(&buf[..n]);
        let Some(last_nl) = self.partial.iter().rposition(|&b| b == b'\n') else { return Vec::new() };
        let rest = self.partial.split_off(last_nl + 1);
        let done = std::mem::replace(&mut self.partial, rest);
        String::from_utf8_lossy(&done).lines().filter(|l| !l.trim().is_empty()).map(String::from).collect()
    }
}

struct Mark {
    ord: u64,
    /// `Some(action)`: started; `None`: finished.
    started: Option<String>,
}

pub struct CaseLog {
    pub id: String,
    pub dir: String,
    tail: Tail,
    marks: Vec<Mark>,
}

impl CaseLog {
    fn update(&mut self, ord: u64) {
        for l in self.tail.lines() {
            let Ok(v) = serde_json::from_str::<Value>(&l) else { continue };
            match v["event"].as_str() {
                Some("action_started") => {
                    self.marks.push(Mark { ord, started: Some(v["action"].as_str().unwrap_or("").to_string()) })
                }
                Some("action_finished") => self.marks.push(Mark { ord, started: None }),
                _ => {}
            }
        }
    }
    /// The action open at `ord`, and the ordinal it started at.
    pub fn open_at(&self, ord: u64) -> Option<(&str, u64)> {
        let m = self.marks.iter().rev().find(|m| m.ord <= ord)?;
        m.started.as_deref().map(|a| (a, m.ord))
    }
}

pub struct Tool {
    pub seq: u64,
    pub call_id: String,
    pub name: String,
    pub input: String,
    pub req_at: i64,
    pub res_at: Option<i64>,
}

#[derive(Default)]
pub struct Stream {
    tail: Option<Tail>,
    pub tools: Vec<Tool>,
    pub latest_at: i64,
    pub closed: bool,
}

fn tool_input(name: &str, input: &Value) -> String {
    let s = match (name, input) {
        ("Bash", v) if v["command"].is_string() => v["command"].as_str().unwrap_or("").to_string(),
        (_, v) if v["file_path"].is_string() => v["file_path"].as_str().unwrap_or("").to_string(),
        (_, v) if v["notebook_path"].is_string() => v["notebook_path"].as_str().unwrap_or("").to_string(),
        (_, v) => v.to_string(),
    };
    s.chars().take(300).collect()
}

impl Stream {
    fn update(&mut self, path: PathBuf) -> bool {
        let tail = self.tail.get_or_insert_with(|| Tail::new(path));
        let lines = tail.lines();
        for l in &lines {
            let Ok(v) = serde_json::from_str::<Value>(l) else { continue };
            let at = v["at"].as_str().and_then(parse_ms);
            if let Some(a) = at {
                self.latest_at = self.latest_at.max(a);
            }
            match v["event"].as_str() {
                Some("tool.requested") => {
                    let name = v["name"].as_str().unwrap_or("").to_string();
                    self.tools.push(Tool {
                        seq: v["seq"].as_u64().unwrap_or(0),
                        call_id: v["call_id"].as_str().unwrap_or("").to_string(),
                        input: tool_input(&name, &v["input"]),
                        name,
                        req_at: at.unwrap_or(self.latest_at),
                        res_at: None,
                    });
                }
                Some("tool.result") => {
                    let id = v["call_id"].as_str().unwrap_or("");
                    let res = at.unwrap_or(self.latest_at);
                    if let Some(t) = self.tools.iter_mut().rev().find(|t| t.call_id == id) {
                        t.res_at = Some(res);
                    }
                }
                Some("session.ended") | Some("stream.closed") => self.closed = true,
                _ => {}
            }
        }
        !lines.is_empty()
    }

    /// The tool call in flight at wall time `t`: of the calls whose window holds `t`, one whose
    /// input names the file, else the one requested last.
    pub fn tool_at(&self, t: i64, file_name: &str) -> Option<&Tool> {
        let c: Vec<&Tool> = self.tools.iter()
            .filter(|x| x.req_at <= t + REQ_SLACK_MS && x.res_at.is_none_or(|r| t <= r + RES_SLACK_MS))
            .collect();
        let naming: Vec<&Tool> = c.iter().copied().filter(|x| !file_name.is_empty() && x.input.contains(file_name)).collect();
        let pool = if naming.is_empty() { c } else { naming };
        pool.into_iter().max_by_key(|x| (x.req_at, x.seq))
    }

    /// Whether the stream has caught up with wall time `t` (or there is no session at all).
    pub fn caught_up(&self, t: i64) -> bool {
        self.tail.is_none() || self.closed || self.latest_at >= t
    }
}

pub struct Record {
    pub rel: String,
    pub action: String,
    pub decider: bool,
    pub case: Option<String>,
    pub associated: bool,
    /// The case came from the open-action guess; a later `request.json` or call log replaces it.
    pub guessed: bool,
    pub created_ord: u64,
    /// Loom run: the ordinal at which `status.json` was first seen finished or failed.
    pub closed_ord: Option<u64>,
    pub stream: Stream,
}

/// The finished steps of a Loom run: the ordinal at which each line of `rounds.jsonl` was seen.
pub struct Steps {
    tail: Tail,
    marks: Vec<u64>,
}

impl Steps {
    fn update(&mut self, ord: u64) {
        for l in self.tail.lines() {
            if serde_json::from_str::<Value>(&l).is_ok() {
                self.marks.push(ord);
            }
        }
    }
    /// The step in flight at `ord`: one after the steps finished by then.
    pub fn at(&self, ord: u64) -> u64 {
        self.marks.iter().filter(|m| **m <= ord).count() as u64 + 1
    }
}

pub struct Attributor {
    pub run: PathBuf,
    pub cases: BTreeMap<String, CaseLog>,
    pub records: BTreeMap<String, Record>,
    /// `Some` for a Loom run.
    pub steps: Option<Steps>,
}

pub struct Found {
    pub attribution: Value,
    /// Nothing more can be learnt by waiting for the stream.
    pub resolved: bool,
    pub record: Option<String>,
    pub has_tool: bool,
}

fn last(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

impl Attributor {
    pub fn new(run: PathBuf) -> Attributor {
        let steps = (layout::kind() == layout::Kind::Loom).then(|| Steps { tail: Tail::new(run.join("rounds.jsonl")), marks: Vec::new() });
        Attributor { run, cases: BTreeMap::new(), records: BTreeMap::new(), steps }
    }

    /// `rounds.jsonl` grew: its new lines are steps that ended at `ord`.
    pub fn steps_update(&mut self, ord: u64) {
        if let Some(s) = &mut self.steps {
            s.update(ord);
        }
    }

    /// A call record's `status.json` was written: a call that is no longer `running` is closed.
    pub fn status_update(&mut self, rel: &str, ord: u64) {
        self.record(rel, ord, false);
        let state = fs::read_to_string(self.run.join(rel).join("status.json")).ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v["state"].as_str().map(String::from));
        if let (Some(s), Some(r)) = (state, self.records.get_mut(rel)) {
            if s != "running" && r.closed_ord.is_none() {
                r.closed_ord = Some(ord);
            }
        }
    }

    fn open_at(r: &Record, ord: u64) -> bool {
        r.created_ord <= ord && r.closed_ord.is_none_or(|c| c > ord)
    }

    /// The agent call record open at `ord` (the one opened last): a Loom run's `work/` change
    /// belongs to it. A record whose `status.json` shows no end is open.
    fn open_call(&self, ord: u64) -> Option<String> {
        self.records.values().filter(|r| !r.decider && Self::open_at(r, ord))
            .max_by(|x, y| (x.created_ord, &x.rel).cmp(&(y.created_ord, &y.rel))).map(|r| r.rel.clone())
    }

    pub fn case_update(&mut self, dir: &str, ord: u64) {
        let run = self.run.clone();
        let c = self.cases.entry(dir.to_string()).or_insert_with(|| CaseLog {
            id: last(dir).to_string(), dir: dir.to_string(),
            tail: Tail::new(run.join(dir).join("events.jsonl")), marks: Vec::new() });
        c.update(ord);
    }

    /// Registers an agent call record; `associate_now` ties it to a case from the actions open now.
    pub fn record(&mut self, rel: &str, ord: u64, associate_now: bool) {
        if !self.records.contains_key(rel) {
            let c: Vec<&str> = rel.split('/').collect();
            self.records.insert(rel.to_string(), Record { rel: rel.to_string(), action: layout::record_action(last(rel)),
                decider: layout::record_kind_is_decider(&c), case: None, associated: false, guessed: false, created_ord: ord,
                closed_ord: None, stream: Stream::default() });
        }
        if associate_now {
            self.associate(rel, ord);
        }
    }

    /// Returns whether the stream got new lines.
    pub fn stream_update(&mut self, rel: &str, ord: u64) -> bool {
        self.record(rel, ord, true);
        let p = self.run.join(rel).join("events.jsonl");
        self.records.get_mut(rel).is_some_and(|r| r.stream.update(p))
    }

    fn open_actions(&self, ord: u64, tree: Option<&str>) -> Vec<(&CaseLog, &str, u64)> {
        self.cases.values()
            .filter(|c| tree.is_none_or(|t| c.dir == t || c.dir.starts_with(&format!("{t}/"))))
            .filter_map(|c| c.open_at(ord).map(|(a, o)| (c, a, o))).collect()
    }

    fn calls_case(&self, rel: &str) -> Option<String> {
        let parent = rel.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
        for f in [self.run.join("llm-calls.jsonl"), self.run.join(parent).join("calls.jsonl")] {
            let Ok(text) = fs::read_to_string(&f) else { continue };
            for l in text.lines().rev() {
                let Ok(v) = serde_json::from_str::<Value>(l) else { continue };
                if v["record"].as_str().is_some_and(|r| last(r) == last(rel)) {
                    return v["case"].as_str().filter(|c| !c.is_empty()).map(String::from);
                }
            }
        }
        None
    }

    /// The case an agent call's `request.json` names (`case`, written by `sdc-node`).
    fn request_case(&self, rel: &str) -> Option<String> {
        let t = fs::read_to_string(self.run.join(rel).join("request.json")).ok()?;
        let v: Value = serde_json::from_str(&t).ok()?;
        v["case"].as_str().filter(|c| !c.is_empty()).map(String::from)
    }

    /// Ties a record to its case: the decider's request, the call's `request.json` (`case`), the
    /// call log, else the one case whose open action (at `ord`) is the record's action (the latest
    /// started if several). A guessed case is replaced once `request.json` or the log names one.
    pub fn associate(&mut self, rel: &str, ord: u64) {
        let Some(r) = self.records.get(rel) else { return };
        if r.associated && !r.guessed {
            return;
        }
        let (decider, action, was_guessed) = (r.decider, r.action.clone(), r.guessed);
        let mut case = None;
        let mut guessed = false;
        let mut action_override = None;
        if decider {
            if let Ok(v) = fs::read_to_string(self.run.join(rel).join("decide-request.json"))
                .map_err(|_| ()).and_then(|t| serde_json::from_str::<Value>(&t).map_err(|_| ())) {
                case = v["case_dir"].as_str().map(|d| last(d.trim_end_matches('/')).to_string());
                action_override = v["action"].as_str().map(String::from);
            }
        } else {
            case = self.request_case(rel).or_else(|| self.calls_case(rel));
            if case.is_none() && !was_guessed {
                case = self.open_actions(ord, None).into_iter().filter(|(_, a, _)| *a == action)
                    .max_by_key(|(_, _, o)| *o).map(|(c, _, _)| c.id.clone());
                guessed = case.is_some();
            }
        }
        let r = self.records.get_mut(rel).unwrap();
        if let Some(a) = action_override {
            r.action = a;
        }
        if case.is_some() {
            r.case = case;
            r.associated = true;
            r.guessed = guessed;
        }
    }

    pub fn associate_all(&mut self, ord: u64) {
        let rels: Vec<String> = self.records.keys().cloned().collect();
        for r in rels {
            self.associate(&r, ord);
        }
    }

    pub fn attribute(&mut self, root: &layout::Root, ord: u64, t: i64, file_name: &str) -> Found {
        let none = |case: Option<&str>, record: Option<&str>| Found {
            attribution: json!({"case": case, "action": null, "record": record, "tool": null, "basis": "none"}),
            resolved: true, record: None, has_tool: false };
        let (case, action, rec_rel, window): (String, String, Option<String>, bool) = match &root.kind {
            layout::RootKind::Work { tree } => {
                let Some((c, a, o)) = self.open_actions(ord, Some(tree)).into_iter().max_by_key(|x| x.2) else {
                    return none(None, None);
                };
                let rec = self.records.values()
                    .filter(|r| r.case.as_deref() == Some(c.id.as_str()) && r.action == a && r.created_ord >= o)
                    .max_by(|x, y| (x.created_ord, &x.rel).cmp(&(y.created_ord, &y.rel))).map(|r| r.rel.clone());
                (c.id.clone(), a.to_string(), rec, true)
            }
            layout::RootKind::Loom => match self.open_call(ord) {
                Some(rel) => {
                    self.associate(&rel, ord);
                    let r = &self.records[&rel];
                    (r.case.clone().unwrap_or_default(), r.action.clone(), Some(rel), true)
                }
                None => (String::new(), String::new(), None, true),
            },
            layout::RootKind::Record { record } => {
                self.record(record, ord, false);
                self.associate(record, ord);
                let r = &self.records[record];
                let (case, action) = (r.case.clone(), r.action.clone());
                let window = if self.steps.is_some() {
                    !r.decider && Self::open_at(r, ord)
                } else {
                    case.as_deref().is_some_and(|cid| {
                        self.cases.values().any(|c| c.id == cid && c.open_at(ord).is_some_and(|(a, _)| a == action))
                    })
                };
                (case.unwrap_or_default(), action, Some(record.clone()), window)
            }
        };
        let rec = rec_rel.as_ref().and_then(|r| self.records.get(r));
        let tool = rec.and_then(|r| r.stream.tool_at(t, file_name));
        let resolved = tool.is_some() || rec.is_none_or(|r| r.stream.caught_up(t));
        let loom = self.steps.is_some();
        let basis = if tool.is_some() {
            "tool-window"
        } else if loom && (window || rec.is_none()) {
            "step-window"
        } else if window {
            "action-window"
        } else if rec.is_some() {
            "record"
        } else {
            "none"
        };
        let case_v = if case.is_empty() { Value::Null } else { json!(case) };
        let action_v = if basis == "none" || action.is_empty() { Value::Null } else { json!(action) };
        let mut attribution = json!({"case": case_v, "action": action_v, "record": rec_rel,
            "tool": tool.map(|x| json!({"seq": x.seq, "name": x.name, "input": x.input})), "basis": basis});
        if let Some(s) = &self.steps {
            attribution["step"] = json!(s.at(ord));
        }
        Found { attribution, resolved, has_tool: tool.is_some(), record: rec_rel }
    }
}

pub fn file_name(path: &Path) -> String {
    path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_window_prefers_latest_and_naming() {
        let mut s = Stream { tail: Some(Tail::new(PathBuf::from("/nonexistent"))), ..Default::default() };
        let t = |seq, input: &str, req, res| Tool { seq, call_id: format!("c{seq}"), name: "Write".into(),
            input: input.into(), req_at: req, res_at: res };
        s.tools = vec![t(1, "/x/a.rs", 1000, Some(1100)), t(2, "/x/b.rs", 1120, Some(1300)), t(3, "cargo test", 2000, None)];
        assert_eq!(s.tool_at(1050, "a.rs").unwrap().seq, 1);
        assert_eq!(s.tool_at(1130, "b.rs").unwrap().seq, 2);
        assert_eq!(s.tool_at(1130, "a.rs").unwrap().seq, 1, "both windows hold t; the naming one wins");
        assert!(s.tool_at(1700, "b.rs").is_none());
        assert_eq!(s.tool_at(99_999, "z").unwrap().seq, 3, "an open call holds every later t");
        assert!(!s.caught_up(5000));
        s.closed = true;
        assert!(s.caught_up(5000));
    }
}
