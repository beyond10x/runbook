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
//! reason that names the action, the adapter, its exit and the client's error from its stderr (in words
//! where this module knows it); a value the call's record does not hold is "not recorded".
//!
//! One call directory is no call: the step ended before it started any adapter (no `request.json`,
//! `status.json` with its exit). It costs nothing, books nothing and does not end the run; the
//! run goes on with what Loom says of the step's exit.
//!
//! A run on a route ([`CostReader::routed`]) books every model call where it is relayed: the relay's
//! line is the call's cost (`crate::relay`). Its cost lines are read for what they say and carry no
//! cost: a `cost_usd` is not booked, and a line without one is no reason to end the run.

use crate::record_write;
use serde_json::Value;
use std::collections::HashSet;
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
    /// The run is on a route: the relay books its calls, so no line carries a cost here.
    routed: bool,
    /// The role names of the run's route file, when the run has one.
    route_roles: Option<Vec<String>>,
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

/// What a reason says for a value the call's record does not hold.
const NOT_RECORDED: &str = "not recorded";

/// Claude Code's error line for a model it does not know: `[claude-code:unrecognized_model] {"model": ...}`.
const UNRECOGNIZED_MODEL: &str = "[claude-code:unrecognized_model]";

/// What the call's adapter wrote on stderr (`adapter-stderr.txt`, else `stderr.txt` of the call directory), `home`
/// replaced by `~`: the client's own error in words where [`client_words`] knows it, else the first non-empty line cut
/// at 300 characters. `roles` are the role names of the run's route file, `None` when the run has none.
fn stderr_line(call_dir: &Path, home: Option<&str>, roles: Option<&[String]>) -> String {
    for name in ["adapter-stderr.txt", "stderr.txt"] {
        let Ok(bytes) = record_write::read_regular_prefix(&call_dir.join(name), 65_536) else { continue };
        let text = String::from_utf8_lossy(&bytes);
        let text = match home {
            Some(h) if h.len() > 1 => text.replace(h, "~"),
            _ => text.into_owned(),
        };
        let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        let Some(first) = lines.first() else { continue };
        return lines.iter().find_map(|l| client_words(l, roles)).unwrap_or_else(|| first.chars().take(STDERR_LINE_MAX).collect());
    }
    "(no stderr)".to_owned()
}

/// The client's own error of `line` in words, for the errors known here: Claude Code's `unrecognized_model` names the
/// model the client asked for and the roles of the run's route file with the model the file gives each
/// (`runbook-<role>`), or says the run has no route file. `None` for every other line.
fn client_words(line: &str, roles: Option<&[String]>) -> Option<String> {
    let detail: Value = serde_json::from_str(line.strip_prefix(UNRECOGNIZED_MODEL)?.trim()).unwrap_or(Value::Null);
    let model = match detail.get("model").and_then(Value::as_str) {
        Some(m) => format!(", {}", Value::from(m.chars().take(STDERR_LINE_MAX).collect::<String>())),
        None => format!(" (model {NOT_RECORDED})"),
    };
    let route = match roles {
        None => "the run has no route file".to_owned(),
        Some([]) => "the run's route file names no role".to_owned(),
        Some(roles) => {
            let named: Vec<String> = roles.iter().map(|r| format!("{r} (model {})", crate::relay::role_model(r))).collect();
            format!("the roles of the run's route file: {}", named.join(", "))
        }
    };
    Some(format!("Claude Code does not know the model it asked for{model}; {route} {UNRECOGNIZED_MODEL}"))
}

/// The JSON document `file` of a call directory, `Null` when it is missing, no regular file or no JSON.
fn record_json(call_dir: &Path, file: &str) -> Value {
    record_write::read_regular(&call_dir.join(file)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null)
}

/// The adapter of a call: the one its `response.json` names, else its `request.json`.
fn adapter_of(call_dir: &Path, response: &Value) -> Option<String> {
    let named = |v: &Value| v.get("adapter").and_then(Value::as_str).map(str::to_owned);
    named(response).or_else(|| named(&record_json(call_dir, "request.json")))
}

/// The adapter's exit in a call's `response.json`: `adapter_exit` (the client's own exit, where a program keeps it beside
/// the exit it reports), else `exit`.
fn exit_of(response: &Value) -> Option<i64> {
    response.get("adapter_exit").and_then(Value::as_i64).or_else(|| response.get("exit").and_then(Value::as_i64))
}

/// The action of a call directory's name `NNN-<action>`.
fn action_of(name: &str) -> Option<&str> {
    name.split_once('-').map(|(_, a)| a).filter(|a| !a.is_empty())
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
    let status: Value = serde_json::from_slice(&record_write::read_regular(&call_dir.join("status.json")).ok()?).ok()?;
    let exit = status.get("exit").and_then(Value::as_i64)?;
    Some(serde_json::json!({"call": name, "exit": exit}))
}

impl CostReader {
    /// A reader that writes `home` as `~` in the stderr lines of its reasons.
    pub fn with_home(home: Option<String>) -> CostReader {
        CostReader { home, ..CostReader::default() }
    }

    /// This reader for a run on a route: a cost line is booked at 0 and needs no `cost_usd`, because the relay's
    /// line of the call is its cost.
    pub fn routed(mut self) -> CostReader {
        self.routed = true;
        self
    }

    /// This reader for a run whose route file names `roles`.
    pub fn with_route_roles(mut self, roles: Vec<String>) -> CostReader {
        self.route_roles = Some(roles);
        self
    }

    /// The call directory of a cost line: its name below `<workspace>/.sdc/llm/`.
    fn call_dir(dir: &Path, line: &Value) -> Option<std::path::PathBuf> {
        let name = line.get("record").and_then(Value::as_str).and_then(|p| p.rsplit('/').next())?;
        Some(dir.join(name))
    }

    /// What the call's adapter wrote on stderr, with the role names of the run's route file (see [`stderr_line`]).
    fn said(&self, call_dir: &Path) -> String {
        stderr_line(call_dir, self.home.as_deref(), self.route_roles.as_deref())
    }

    /// The reason a call without a usable cost ends the run: action, adapter, exit and stderr. An action or an adapter
    /// the line does not name is taken from its call directory (its name, its `request.json`), else "not recorded".
    fn reason(&self, dir: &Path, line: &Value, detail: &str) -> String {
        let call_dir = Self::call_dir(dir, line);
        let named = |key: &str| line.get(key).and_then(Value::as_str).map(str::to_owned);
        let call_name = call_dir.as_deref().and_then(Path::file_name).and_then(|n| n.to_str());
        let action = named("action").or_else(|| call_name.and_then(action_of).map(str::to_owned)).unwrap_or_else(|| NOT_RECORDED.to_owned());
        let adapter = named("adapter").or_else(|| call_dir.as_deref().and_then(|d| adapter_of(d, &Value::Null))).unwrap_or_else(|| NOT_RECORDED.to_owned());
        let exit = line.get("exit").and_then(Value::as_i64);
        match exit {
            Some(code) if code != 0 => {
                let said = call_dir.map_or_else(|| "(no stderr)".to_owned(), |d| self.said(&d));
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
            let text = record_write::read_regular_text(&lines_file).map_err(|e| format!("cost records unreadable: {e}"))?;
            let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
            for line in lines.iter().skip(self.lines_seen) {
                let v: Value = serde_json::from_str(line).map_err(|e| format!("cost record is not JSON: {e}"))?;
                if let Some(name) = v.get("record").and_then(Value::as_str).and_then(|p| p.rsplit('/').next()) {
                    self.recorded_dirs.insert(name.to_owned());
                }
                // A dry call (`--dry`, adapter `dry`) calls no model: it has no cost and costs 0.
                let dry = v.get("dry") == Some(&Value::Bool(true)) && v.get("cost_usd").is_none_or(Value::is_null);
                let micro = match v.get("cost_usd") {
                    _ if dry || self.routed => Ok(0),
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
                    let action = action_of(&name).unwrap_or(NOT_RECORDED);
                    let response = record_json(&entry.path(), "response.json");
                    let adapter = adapter_of(&entry.path(), &response).unwrap_or_else(|| NOT_RECORDED.to_owned());
                    let exit = exit_of(&response).map_or_else(|| format!("exit {NOT_RECORDED}"), |c| format!("exit {c}"));
                    let said = self.said(&entry.path());
                    let reason = format!("model step {action}: model call {name} has no cost record (adapter {adapter}, {exit}): {said}");
                    // said once, by the step it belongs to: on a route the relay's line of the call stands for the record,
                    // and the next step's read does not fail on it again
                    self.checked_dirs.insert(name);
                    return Err(reason);
                }
                self.checked_dirs.insert(name);
            }
        }
        Ok(cost)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A scratch directory beside the test binary, made empty.
    struct Dir(PathBuf);

    impl Dir {
        fn new(tag: &str) -> Dir {
            let d = std::env::current_exe().unwrap().parent().unwrap().join(format!("cost-unit-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            Dir(d)
        }

        fn dir(&self, rel: &str) -> PathBuf {
            let p = self.0.join(rel);
            std::fs::create_dir_all(&p).unwrap();
            p
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_stderr_line_of_a_call_is_the_first_line_of_its_file_in_full() {
        let dir = Dir::new("stderr-line");
        let call = dir.dir("call");
        std::fs::write(call.join("adapter-stderr.txt"), "\n  the model said no, at length  \nsecond\n").unwrap();
        assert_eq!(stderr_line(&call, None, None), "the model said no, at length");
        let home = format!("{}/home", call.display());
        std::fs::write(call.join("adapter-stderr.txt"), format!("failed in {home}/x\n")).unwrap();
        assert_eq!(stderr_line(&call, Some(&home), None), "failed in ~/x");
    }

    /// A workspace whose `.sdc/llm/<call>/` holds `files` (name, text).
    fn call(dir: &Dir, call: &str, files: &[(&str, &str)]) -> PathBuf {
        let work = dir.dir("work");
        let rec = dir.dir(&format!("work/.sdc/llm/{call}"));
        for (name, text) in files {
            std::fs::write(rec.join(name), text).unwrap();
        }
        work
    }

    #[test]
    fn the_failure_of_a_call_without_a_cost_record_names_the_adapter_of_its_request() {
        let dir = Dir::new("names-request");
        let work = call(&dir, "001-proposals.write", &[("request.json", r#"{"action":"proposals.write","adapter":"claude-cli"}"#), ("response.json", ""), ("stderr.txt", "it broke\n")]);
        assert_eq!(
            CostReader::default().read_new(&work),
            Err("model step proposals.write: model call 001-proposals.write has no cost record (adapter claude-cli, exit not recorded): it broke".to_owned())
        );
        let dir = Dir::new("names-nothing");
        let work = call(&dir, "001-proposals.write", &[("request.json", "{}"), ("stderr.txt", "it broke\n")]);
        assert_eq!(
            CostReader::default().read_new(&work),
            Err("model step proposals.write: model call 001-proposals.write has no cost record (adapter not recorded, exit not recorded): it broke".to_owned())
        );
    }

    #[test]
    fn the_failure_of_a_call_without_a_cost_record_names_the_exit_of_its_response() {
        for (response, said) in [
            (r#"{"adapter":"claude-cli","exit":1,"adapter_exit":143}"#, "adapter claude-cli, exit 143"),
            (r#"{"adapter":"metaharness","exit":2}"#, "adapter metaharness, exit 2"),
        ] {
            let dir = Dir::new("names-response");
            let work = call(&dir, "002-notes.write", &[("request.json", r#"{"adapter":"from-the-request"}"#), ("response.json", response)]);
            assert_eq!(
                CostReader::default().read_new(&work),
                Err(format!("model step notes.write: model call 002-notes.write has no cost record ({said}): (no stderr)")),
                "{response}"
            );
        }
    }

    #[test]
    fn a_cost_line_without_action_or_adapter_says_not_recorded_or_takes_them_from_the_call() {
        let dir = Dir::new("line-from-call");
        let work = call(&dir, "001-notes.write", &[("request.json", r#"{"adapter":"claude-cli"}"#), ("stderr.txt", "no login\n")]);
        let rec = work.join(".sdc/llm/001-notes.write");
        std::fs::write(work.join(".sdc/llm/calls.jsonl"), format!("{}\n", serde_json::json!({"record": rec.display().to_string(), "exit": 127}))).unwrap();
        assert_eq!(CostReader::default().read_new(&work), Err("model step notes.write: adapter claude-cli exited 127: no login".to_owned()));
        let dir = Dir::new("line-bare");
        let work = dir.dir("work");
        dir.dir("work/.sdc/llm");
        std::fs::write(work.join(".sdc/llm/calls.jsonl"), "{\"exit\":0}\n").unwrap();
        assert_eq!(
            CostReader::default().read_new(&work),
            Err("model step not recorded: adapter not recorded exited 0 and recorded no cost (a cost record has no cost_usd)".to_owned())
        );
    }

    #[test]
    fn claude_codes_unrecognized_model_is_said_in_words_with_the_roles_of_the_route_file() {
        let error = "[claude-code:unrecognized_model] {\"model\":\"default[1m]\",\"query_source\":\"sdk\"}\n";
        let files = [("request.json", r#"{"adapter":"claude-cli"}"#), ("stderr.txt", error)];
        let dir = Dir::new("words-roles");
        let work = call(&dir, "001-proposals.write", &files);
        let reader = CostReader::default().routed().with_route_roles(vec!["default".to_owned(), "review".to_owned()]);
        assert_eq!(
            { reader }.read_new(&work),
            Err("model step proposals.write: model call 001-proposals.write has no cost record (adapter claude-cli, exit not recorded): Claude Code does not know the model it asked for, \"default[1m]\"; the roles of the run's route file: default (model runbook-default), review (model runbook-review) [claude-code:unrecognized_model]".to_owned())
        );
        let dir = Dir::new("words-no-route");
        let work = call(&dir, "001-proposals.write", &[("request.json", r#"{"adapter":"claude-cli"}"#), ("stderr.txt", &format!("a warning first\n{}", error.replace("\"default[1m]\"", "null")))]);
        assert_eq!(
            CostReader::default().read_new(&work),
            Err("model step proposals.write: model call 001-proposals.write has no cost record (adapter claude-cli, exit not recorded): Claude Code does not know the model it asked for (model not recorded); the run has no route file [claude-code:unrecognized_model]".to_owned())
        );
    }

    #[test]
    fn the_stderr_line_of_a_call_is_not_read_through_a_link_and_the_next_file_is_tried() {
        let dir = Dir::new("stderr-link");
        let call = dir.dir("call");
        let outside = dir.dir("outside");
        std::fs::write(outside.join("said.txt"), "text of a file outside the run\n").unwrap();
        std::os::unix::fs::symlink(outside.join("said.txt"), call.join("adapter-stderr.txt")).unwrap();
        assert_eq!(stderr_line(&call, None, None), "(no stderr)");
        std::fs::write(call.join("stderr.txt"), "the plain file\n").unwrap();
        assert_eq!(stderr_line(&call, None, None), "the plain file");
    }
}
