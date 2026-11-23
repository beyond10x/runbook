//! Records: where the facts about a case come from (the driver's case directory or the records of a
//! Loom workspace), the digest of a tree, and the rejections and logs kept there.
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::{read_json, sorted, text};

/// The rejected records of `action` (the evidence kind its binding produces, result `rejected`)
/// in the case, oldest first: `{record, revision, at, principal, reasons, citations}`. Derived from
/// the evidence and the event log, so no counter is kept anywhere.
pub fn rejections(case_dir: &Path, action: &str) -> Value {
    let bindings = read_json(&case_dir.join("bindings.json"));
    let Some(kind) = bindings[action]["produces"].as_str() else { return json!([]) };
    let events: Vec<Value> = fs::read_to_string(case_dir.join("events.jsonl")).unwrap_or_default().lines()
        .filter_map(|l| serde_json::from_str(l).ok()).collect();
    let mut out = Vec::new();
    for e in events.iter().filter(|e| e["event"] == "action_finished" && e["action"] == action) {
        for id in e["evidence"].as_array().into_iter().flatten().filter_map(|i| i.as_str()) {
            let r = read_json(&case_dir.join("evidence").join(format!("{id}.json")));
            if r["kind"] != kind || r["result"] != "rejected" {
                continue;
            }
            let why = e["log"].as_str().map(|l| read_json(&case_dir.join("blobs").join(l))).unwrap_or(Value::Null);
            out.push(json!({"record": id, "revision": r["subject_revision"], "at": r["observed_at"],
                "principal": r["producer"], "reasons": why["reasons"], "citations": why["citations"]}));
        }
    }
    sorted(Value::Array(out))
}

/// Where the facts about a case come from. Under the old driver (`Driver`) they are the driver's
/// case directory: `case.json` (the revisions), the event log, the evidence and the blobs. Under
/// Loom (`LOOM_CASE` set; `Workspace`) a bound program gets its working directory, the workspace,
/// and nothing else, so the same facts are small record files below `<work>/.sdc/records/`, each
/// written by the tool that owns the fact: `<kind>/<digest>.log` (`sdc-reply record`, called by the
/// tools of tests.run and spec.gate: the log of the step, about the tree whose content has that
/// digest) and `rejections/<action>/<n>.json` (the review's own rejections, written by
/// `check-review`). The revision of an artifact is then the digest of its tree.
pub struct Facts {
    dir: PathBuf,
    work: PathBuf,
    workspace: bool,
}

/// The work directory a case directory names: its `work/` when it has one (the driver's layout, and
/// a Loom case directory that keeps the workspace in `work/`), else the directory itself, which then
/// is the workspace (Loom's suite layout keeps workspaces in `work/<n>`, apart from `store/<n>`).
pub fn case_work(case_dir: &Path) -> PathBuf {
    let work = case_dir.join("work");
    if work.is_dir() { work } else { case_dir.to_path_buf() }
}

/// The case directory of a workspace `work`: its parent when `work` is that parent's `work/` (the
/// layouts `case_work` reads back to `work`), else `work` itself.
pub fn case_dir_of(work: &Path) -> PathBuf {
    let parent = work.join("..");
    match (fs::canonicalize(parent.join("work")), fs::canonicalize(work)) {
        (Ok(a), Ok(b)) if a == b => parent,
        _ => work.to_path_buf(),
    }
}

/// The records directory below a workspace.
pub const RECORDS: &str = ".sdc/records";

impl Facts {
    /// The source the environment names: the workspace's records when `LOOM_CASE` is set.
    pub fn open(case_dir: &Path) -> Facts {
        let loom = std::env::var_os("LOOM_CASE").is_some_and(|v| !v.is_empty());
        let work = if loom { case_work(case_dir) } else { case_dir.join("work") };
        Facts { dir: case_dir.to_path_buf(), work, workspace: loom }
    }

    /// The driver mode of a case directory, whatever `LOOM_CASE` says (a caller that builds the
    /// facts of a case directory itself, and the tests).
    pub fn driver(case_dir: &Path) -> Facts {
        Facts { dir: case_dir.to_path_buf(), work: case_dir.join("work"), workspace: false }
    }

    /// The records of the workspace `work` (no case directory).
    pub fn workspace(work: &Path) -> Facts {
        Facts { dir: work.join(".."), work: work.to_path_buf(), workspace: true }
    }

    /// The case's work directory.
    pub fn work(&self) -> PathBuf {
        self.work.clone()
    }

    /// The records directory below the work directory.
    pub fn records(&self) -> PathBuf {
        self.work.join(RECORDS)
    }

    /// The case's id: `case.json`, else the id Loom names in `LOOM_CASE`.
    pub fn case_id(&self) -> Value {
        if self.workspace {
            return std::env::var("LOOM_CASE").map(Value::String).unwrap_or(Value::Null);
        }
        read_json(&self.dir.join("case.json"))["id"].clone()
    }

    /// The tree of a case artifact, relative to the work directory (the artifacts of sdc.story).
    pub fn tree_of(artifact: &str) -> Option<&'static str> {
        match artifact {
            "change" => Some("impl"),
            "spec" => Some("spec"),
            _ => None,
        }
    }

    /// The current revision of an artifact: `case.json`'s, else the digest of its tree.
    pub fn revision(&self, artifact: &str) -> String {
        self.revision_value(artifact).as_str().unwrap_or("none").to_string()
    }

    /// The same as JSON: null where the driver's `case.json` has none.
    pub fn revision_value(&self, artifact: &str) -> Value {
        if !self.workspace {
            return read_json(&self.dir.join("case.json"))["artifacts"][artifact]["revision"].clone();
        }
        Self::tree_of(artifact).and_then(|t| tree_digest(&self.work().join(t))).map_or(Value::Null, Value::String)
    }

    /// The log of the last record of `kind` about the artifact's current revision.
    pub fn record_log(&self, kind: &str, artifact: &str) -> Option<String> {
        let revision = self.revision(artifact);
        if !self.workspace {
            return record_log(&self.dir, kind, &revision);
        }
        fs::read_to_string(self.records().join(kind).join(format!("{revision}.log"))).ok()
    }

    /// The rejected records of `action`, oldest first (`rejections`'s shape).
    pub fn rejections(&self, action: &str) -> Value {
        if !self.workspace {
            return rejections(&self.dir, action);
        }
        let dir = self.records().join("rejections").join(action);
        let mut files: Vec<PathBuf> = fs::read_dir(dir).into_iter().flatten().filter_map(|e| e.ok().map(|e| e.path())).collect();
        files.sort();
        Value::Array(files.iter().map(|p| read_json(p)).filter(|v| !v.is_null()).collect())
    }

    /// Keep a rejection `doc` (sdc.rejection/1) of `action` where `rejections` finds it. The driver
    /// keeps its own record of an action's result, so under it nothing is written.
    pub fn note_rejection(&self, action: &str, doc: &Value) -> Result<(), String> {
        if !self.workspace {
            return Ok(());
        }
        let dir = self.records().join("rejections").join(action);
        fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let n = fs::read_dir(&dir).map_or(0, |d| d.flatten().count()) + 1;
        let entry = json!({"record": format!("{n:03}"), "revision": doc["revision"], "at": now_utc(),
            "principal": doc["principal"], "reasons": doc["reasons"], "citations": doc["citations"]});
        let p = dir.join(format!("{n:03}.json"));
        fs::write(&p, text(&entry)).map_err(|e| format!("{}: {e}", p.display()))
    }

    /// Store the log of a step as the record of `kind` about the tree `tree` (relative to the
    /// work directory) in its present content. Returns where it went.
    pub fn write_record(&self, kind: &str, tree: &str, log: &str) -> Result<PathBuf, String> {
        if kind.contains('/') || kind.contains("..") || kind.is_empty() {
            return Err(format!("{kind:?} is no record kind"));
        }
        let digest = tree_digest(&self.work().join(tree)).ok_or(format!("{tree}: no such tree in the workspace"))?;
        let dir = self.records().join(kind);
        fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let p = dir.join(format!("{digest}.log"));
        fs::write(&p, log).map_err(|e| format!("{}: {e}", p.display()))?;
        Ok(p)
    }

    /// The artifacts of the case as (name, file in the work directory, revision): the driver's
    /// bindings file names them; Loom's workspace has no such file, so none resolve there.
    pub fn artifacts(&self) -> Vec<(String, String, String)> {
        if self.workspace {
            return Vec::new();
        }
        let bindings = read_json(&self.dir.join("bindings.json"));
        let case = read_json(&self.dir.join("case.json"));
        bindings["$artifacts"].as_object().into_iter().flatten()
            .filter_map(|(a, f)| Some((a.clone(), f.as_str()?.to_string(), case["artifacts"][a]["revision"].as_str()?.to_string())))
            .collect()
    }
}

/// The digest of a tree's content: its files by relative path, without build output (`target`),
/// `.sdc` and `.derived-from`, as the review's diff leaves them out. None when `dir` is no directory.
pub fn tree_digest(dir: &Path) -> Option<String> {
    use sha2::{Digest, Sha256};
    if !dir.is_dir() {
        return None;
    }
    let mut files = Vec::new();
    let mut stack = vec![PathBuf::new()];
    while let Some(rel) = stack.pop() {
        let Ok(rd) = fs::read_dir(dir.join(&rel)) else { continue };
        for e in rd.flatten() {
            let name = e.file_name();
            if matches!(name.to_str(), Some("target" | ".sdc" | ".derived-from")) {
                continue;
            }
            let r = rel.join(&name);
            if e.path().is_dir() { stack.push(r) } else { files.push(r) }
        }
    }
    files.sort();
    let mut h = Sha256::new();
    for f in &files {
        let body = fs::read(dir.join(f)).unwrap_or_default();
        h.update(f.to_string_lossy().as_bytes());
        h.update([0]);
        h.update(Sha256::digest(&body));
    }
    Some(h.finalize().iter().take(8).map(|b| format!("{b:02x}")).collect())
}

/// The present time as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn now_utc() -> String {
    let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) as i64;
    let (days, rest) = (s.div_euclid(86400), s.rem_euclid(86400));
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rest / 3600, rest % 3600 / 60, rest % 60)
}

/// The log of the last record of `kind` about `revision` in the case (its step's log blob).
pub fn record_log(case_dir: &Path, kind: &str, revision: &str) -> Option<String> {
    let events: Vec<Value> = fs::read_to_string(case_dir.join("events.jsonl")).unwrap_or_default().lines()
        .filter_map(|l| serde_json::from_str(l).ok()).collect();
    events.iter().rev().filter(|e| e["event"] == "action_finished").find_map(|e| {
        let hit = e["evidence"].as_array().into_iter().flatten().filter_map(|i| i.as_str()).any(|id| {
            let r = read_json(&case_dir.join("evidence").join(format!("{id}.json")));
            r["kind"] == kind && r["subject_revision"] == revision
        });
        if !hit { return None }
        e["log"].as_str().and_then(|l| fs::read_to_string(case_dir.join("blobs").join(l)).ok())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_time_of_a_record_is_a_utc_stamp() {
        let t = now_utc();
        assert!(regex::Regex::new(r"^20\d\d-\d\d-\d\dT\d\d:\d\d:\d\dZ$").unwrap().is_match(&t), "{t}");
    }
}
