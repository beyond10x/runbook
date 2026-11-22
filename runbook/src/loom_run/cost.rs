//! The cost of a step: the cost records the bound programs wrote since the last step.
//!
//! A model step (`sdc-node`) writes one directory `NNN-<action>/` per call under
//! `<workspace>/.sdc/llm/` when it starts, and one line per finished call into
//! `<workspace>/.sdc/llm/calls.jsonl` with `cost_usd` and `record` (the call's directory), the
//! `action`, the `adapter` and the adapter's `exit`. A call directory without a line with a number
//! `cost_usd` is a call of unknown cost.
//!
//! A call of unknown cost is booked at its reservation (the spend rule stays fail-closed). What the
//! run does next is the driver's rule (`driver/src/main.rs`, `program_error`): a call that ended
//! with an exit other than 0 does not end the run, the step's retry rule decides, unless the exit
//! is 126, 127 or 75 (the tool could not run). Every other call without a cost ends the run, with a
//! reason that names the action, the adapter, its exit and the first line of its stderr.
//!
//! One call directory is no call: the step ended before it started any adapter (no `request.json`,
//! `status.json` with its exit). It costs nothing, books nothing and does not end the run; the
//! run goes on with what Loom says of the step's exit.

use serde_json::Value;
use std::collections::HashSet;
use std::io::Read;
use std::path::Path;

/// Exits of a model step after which the driver ends the run: the tool could not run.
const PROGRAM_ERROR_EXITS: [i64; 3] = [75, 126, 127];

/// Longest first line of an adapter's stderr in a reason.
const STDERR_LINE_MAX: usize = 300;

/// What one reading of the records found.
#[derive(Debug, Default, PartialEq)]
pub struct StepCost {
    /// Cost of the new calls that have one, micro USD.
    pub micro_usd: u64,
    /// The new cost lines as read.
    pub records: Vec<Value>,
    /// Lines of calls that ended with an exit other than 0 and have no usable cost: each is booked
    /// at the reservation by the caller, and the run goes on.
    pub unknown: Vec<Value>,
    /// Call directories of which no adapter was started (see [`no_call_note`]): they cost nothing
    /// and book nothing. Each is `{"call": <directory name>, "exit": <exit of status.json>}`.
    pub no_call: Vec<Value>,
}

/// Remembers how far the records were read.
#[derive(Debug, Default)]
pub struct CostReader {
    lines_seen: usize,
    recorded_dirs: HashSet<String>,
    checked_dirs: HashSet<String>,
    home: Option<String>,
}

/// USD as a JSON number to micro USD: finite, not negative.
pub fn usd_to_micro(v: &Value) -> Result<u64, String> {
    let x = v.as_f64().ok_or_else(|| format!("cost_usd is not a number: {v}"))?;
    if !x.is_finite() || x < 0.0 {
        return Err(format!("cost_usd is not a finite non-negative number: {v}"));
    }
    let micro = (x * 1_000_000.0).round();
    if micro >= u64::MAX as f64 {
        return Err(format!("cost_usd is too large: {v}"));
    }
    Ok(micro as u64)
}

/// The first non-empty line of what the call's adapter wrote on stderr (`adapter-stderr.txt`, else
/// `stderr.txt` of the call directory), `home` replaced by `~`, cut at 300 characters.
fn stderr_line(call_dir: &Path, home: Option<&str>) -> String {
    for name in ["adapter-stderr.txt", "stderr.txt"] {
        let mut bytes = Vec::new();
        let Ok(file) = std::fs::File::open(call_dir.join(name)) else { continue };
        if file.take(65_536).read_to_end(&mut bytes).is_err() {
            continue;
        }
        let text = String::from_utf8_lossy(&bytes);
        let Some(line) = text.lines().map(str::trim).find(|l| !l.is_empty()) else { continue };
        let line = match home {
            Some(h) if h.len() > 1 => line.replace(h, "~"),
            _ => line.to_owned(),
        };
        return line.chars().take(STDERR_LINE_MAX).collect();
    }
    "(no stderr)".to_owned()
}

/// The note of a call directory in which no adapter was started, or `None` when that is not
/// proved. `sdc-node` writes `request.json` of a call before every start of an
/// adapter (the session, facts and request modes, each retry and the dry stub all pass the one
/// `jq ... > "$rec/request.json"`, and every start names that file), so a directory without it
/// belongs to a step that ended before any adapter was started. Everything else is `None`
/// (fail closed): `request.json` present or not provably absent, and a `status.json` that is
/// missing, unreadable, not JSON or without a numeric `exit` (a call still running).
fn no_call_note(call_dir: &Path, name: &str) -> Option<Value> {
    if !matches!(call_dir.join("request.json").try_exists(), Ok(false)) {
        return None;
    }
    let status: Value = serde_json::from_slice(&std::fs::read(call_dir.join("status.json")).ok()?).ok()?;
    let exit = status.get("exit").and_then(Value::as_i64)?;
    Some(serde_json::json!({"call": name, "exit": exit}))
}

impl CostReader {
    /// A reader that writes `home` as `~` in the stderr lines of its reasons.
    pub fn with_home(home: Option<String>) -> CostReader {
        CostReader { home, ..CostReader::default() }
    }

    /// The call directory of a cost line: its name below `<workspace>/.sdc/llm/`.
    fn call_dir(dir: &Path, line: &Value) -> Option<std::path::PathBuf> {
        let name = line.get("record").and_then(Value::as_str).and_then(|p| p.rsplit('/').next())?;
        Some(dir.join(name))
    }

    /// The reason a call without a usable cost ends the run: action, adapter, exit and stderr.
    fn reason(&self, dir: &Path, line: &Value, detail: &str) -> String {
        let action = line.get("action").and_then(Value::as_str).unwrap_or("?");
        let adapter = line.get("adapter").and_then(Value::as_str).unwrap_or("?");
        let exit = line.get("exit").and_then(Value::as_i64);
        match exit {
            Some(code) if code != 0 => {
                let said = Self::call_dir(dir, line).map_or_else(|| "(no stderr)".to_owned(), |d| stderr_line(&d, self.home.as_deref()));
                format!("model step {action}: adapter {adapter} exited {code}: {said}")
            }
            Some(code) => format!("model step {action}: adapter {adapter} exited {code} and recorded no cost ({detail})"),
            None => format!("model step {action}: adapter {adapter} recorded no cost ({detail})"),
        }
    }

    /// The new cost records of `workspace` since the last call. Errors (fail closed): a call
    /// directory with no cost line, a line that is not JSON, a `cost_usd` that is not a finite
    /// non-negative number on a call that did not end with an exit other than 0 and 126, 127, 75;
    /// the reason names the call.
    pub fn read_new(&mut self, workspace: &Path) -> Result<StepCost, String> {
        let dir = workspace.join(".sdc").join("llm");
        let mut cost = StepCost::default();
        if !dir.is_dir() {
            return Ok(cost);
        }
        let lines_file = dir.join("calls.jsonl");
        if lines_file.exists() {
            let text = std::fs::read_to_string(&lines_file).map_err(|e| format!("cost records unreadable: {e}"))?;
            let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
            for line in lines.iter().skip(self.lines_seen) {
                let v: Value = serde_json::from_str(line).map_err(|e| format!("cost record is not JSON: {e}"))?;
                if let Some(name) = v.get("record").and_then(Value::as_str).and_then(|p| p.rsplit('/').next()) {
                    self.recorded_dirs.insert(name.to_owned());
                }
                // A dry call (`--dry`, adapter `dry`) calls no model: it has no cost and costs 0.
                let dry = v.get("dry") == Some(&Value::Bool(true)) && v.get("cost_usd").is_none_or(Value::is_null);
                let micro = match v.get("cost_usd") {
                    _ if dry => Ok(0),
                    Some(usd) => usd_to_micro(usd),
                    None => Err("a cost record has no cost_usd".to_owned()),
                };
                match micro {
                    Ok(micro) => {
                        cost.micro_usd = cost.micro_usd.checked_add(micro).ok_or("cost overflow")?;
                        cost.records.push(v);
                    }
                    Err(detail) => match v.get("exit").and_then(Value::as_i64) {
                        Some(code) if code != 0 && !PROGRAM_ERROR_EXITS.contains(&code) => cost.unknown.push(v),
                        _ => return Err(self.reason(&dir, &v, &detail)),
                    },
                }
            }
            self.lines_seen = lines.len();
        }
        let entries = std::fs::read_dir(&dir).map_err(|e| format!("cost records unreadable: {e}"))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("cost records unreadable: {e}"))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_call = entry.path().is_dir() && name.split('-').next().is_some_and(|n| n.len() >= 3 && n.bytes().all(|b| b.is_ascii_digit()));
            if is_call && !self.checked_dirs.contains(&name) {
                if !self.recorded_dirs.contains(&name) {
                    if let Some(note) = no_call_note(&entry.path(), &name) {
                        cost.no_call.push(note);
                        self.checked_dirs.insert(name);
                        continue;
                    }
                    let action = name.split_once('-').map_or("?", |(_, a)| a);
                    let response: Value = std::fs::read(entry.path().join("response.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null);
                    let adapter = response.get("adapter").and_then(Value::as_str).unwrap_or("?");
                    let exit = response.get("exit").and_then(Value::as_i64).map_or_else(|| "?".to_owned(), |c| c.to_string());
                    let said = stderr_line(&entry.path(), self.home.as_deref());
                    return Err(format!("model step {action}: model call {name} has no cost record (adapter {adapter}, exit {exit}): {said}"));
                }
                self.checked_dirs.insert(name);
            }
        }
        Ok(cost)
    }
}
