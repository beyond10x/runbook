//! The files of this unit in the run directory: `settings.json`, `events.jsonl` (one JSON object
//! per event), `outcome.json`, `tree-status.json` and `tree-verify.json`.

use crate::loom_run::events::{Cap, CostSource, Event, EventSink, StartSource};
use crate::loom_run::record_write::write_atomic;
use serde_json::{json, Value};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Writes R1's files as events arrive.
pub struct BasicFiles {
    run_dir: PathBuf,
    log: File,
}

impl BasicFiles {
    /// Opens the writer for `run_dir`.
    pub fn open(run_dir: &Path) -> Result<BasicFiles, String> {
        let path = run_dir.join("events.jsonl");
        let log = OpenOptions::new().create_new(true).append(true).open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(BasicFiles { run_dir: run_dir.to_path_buf(), log })
    }
}

/// `text` with the run directory made relative: `<run dir>/x` is `x`, the run directory itself `.`.
/// Loom prints absolute paths of its store and workspace; none of them may reach a file of this
/// unit in the run directory.
pub fn relative_text(run_dir: &Path, text: &str) -> String {
    let root = run_dir.display().to_string();
    let root = root.trim_end_matches('/');
    if root.is_empty() {
        return text.to_owned();
    }
    text.replace(&format!("{root}/"), "").replace(root, ".")
}

/// The JSON object of one event: `{"at": secs, "event": "<name>", ...}`.
pub fn event_json(at_secs: u64, event: &Event) -> Value {
    let (name, fields) = match event {
        Event::RunStarted { run, settings } => ("run_started", json!({"run": run, "settings": settings})),
        Event::ActionStarted { source, role, case, action, try_number } => {
            let source = match source {
                StartSource::ProgramStart => "program_start",
                StartSource::StepCall => "step_call",
            };
            ("action_started", json!({"source": source, "role": role, "case": case, "action": action, "try": try_number}))
        }
        Event::StepMade { index, exit, stop, steps } => ("step_made", json!({"index": index, "exit": exit, "stop": stop, "steps": steps})),
        Event::DecisionAsked { decision } => ("decision_asked", json!({"decision": decision})),
        Event::DecisionAnswered { decision, approve, principal, cost_micro_usd, model_calls } => (
            "decision_answered",
            json!({"decision": decision, "approve": approve, "principal": principal, "cost_micro_usd": cost_micro_usd, "model_calls": model_calls}),
        ),
        Event::AnswerRecorded { line } => ("answer_recorded", json!({"line": line})),
        Event::DeclaredTaken { decision, principal } => ("declared_taken", json!({"decision": decision, "principal": principal})),
        Event::CostedCall { source, cost_micro_usd, record } => {
            let source = match source {
                CostSource::Step => "step",
                CostSource::Decider => "decider",
            };
            ("costed_call", json!({"source": source, "cost_micro_usd": cost_micro_usd, "record": record}))
        }
        Event::NoModelCall { call, exit } => ("no_model_call", json!({"note": "step, no model call made", "call": call, "exit": exit})),
        Event::CapReached { cap, detail } => {
            let cap = match cap {
                Cap::Spend => "spend",
                Cap::LlmCalls => "llm_calls",
                Cap::WallTime => "wall_time",
            };
            ("cap_reached", json!({"cap": cap, "detail": detail}))
        }
        Event::TreeStored { status_exit, verify_exit } => ("tree_stored", json!({"status_exit": status_exit, "verify_exit": verify_exit})),
        Event::Ended { outcome, reason, cause, loom_exit, stderr_tail, exit_code, steps, spent_micro_usd, llm_calls, recorded_micro_usd, recorded_llm_calls } => {
            let mut fields = json!({"outcome": outcome, "reason": reason, "loom_exit_code": loom_exit, "stderr_tail": stderr_tail, "exit_code": exit_code, "steps": steps, "spent_micro_usd": spent_micro_usd, "llm_calls": llm_calls, "recorded_micro_usd": recorded_micro_usd, "recorded_llm_calls": recorded_llm_calls});
            if let (Some(cause), Some(map)) = (cause, fields.as_object_mut()) {
                map.insert("cause".to_owned(), Value::from(cause.as_str()));
            }
            ("ended", fields)
        }
    };
    let mut o = serde_json::Map::new();
    o.insert("at".to_owned(), Value::from(at_secs));
    o.insert("event".to_owned(), Value::from(name));
    if let Value::Object(f) = fields {
        o.extend(f);
    }
    Value::Object(o)
}

impl EventSink for BasicFiles {
    fn event(&mut self, at_secs: u64, event: &Event) -> Result<(), String> {
        if let Event::RunStarted { settings, .. } = event {
            let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())? + "\n";
            std::fs::write(self.run_dir.join("settings.json"), text).map_err(|e| format!("settings.json: {e}"))?;
        }
        let line = serde_json::to_string(&event_json(at_secs, event)).map_err(|e| e.to_string())?;
        let line = relative_text(&self.run_dir, &line);
        writeln!(self.log, "{line}").map_err(|e| format!("events.jsonl: {e}"))?;
        if let Event::Ended { outcome, reason, cause, loom_exit, stderr_tail, exit_code, steps, spent_micro_usd, llm_calls, recorded_micro_usd, recorded_llm_calls } = event {
            let mut v = json!({
                "format": "runbook.loom-run-outcome/1",
                "outcome": outcome,
                "reason": reason,
                "loom_exit_code": loom_exit,
                "stderr_tail": stderr_tail,
                "exit_code": exit_code,
                "steps": steps,
                "spent_micro_usd": spent_micro_usd,
                "llm_calls": llm_calls,
                "recorded_micro_usd": recorded_micro_usd,
                "recorded_llm_calls": recorded_llm_calls,
            });
            if let (Some(cause), Some(map)) = (cause, v.as_object_mut()) {
                map.insert("cause".to_owned(), Value::from(cause.as_str()));
            }
            let text = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())? + "\n";
            write_atomic(&self.run_dir.join("outcome.json"), relative_text(&self.run_dir, &text).as_bytes())?;
        }
        Ok(())
    }
}

/// Writes `tree-status.json`: what `tree status` printed with the run directory made relative
/// (`store`, `work`, `store/case-....json`), through a temporary file and a rename.
pub fn write_status(run_dir: &Path, status_stdout: &[u8]) -> Result<(), String> {
    let text = relative_text(run_dir, &String::from_utf8_lossy(status_stdout));
    write_atomic(&run_dir.join("tree-status.json"), text.as_bytes())
}

/// Writes `tree-status.json` (see [`write_status`]; kept as it is when `None`) and `tree-verify.json` (`{"exit", "stdout",
/// "stderr"}` of `tree verify`, the run directory made relative).
pub fn write_tree_files(run_dir: &Path, status_stdout: Option<&[u8]>, verify_exit: i32, verify_stdout: &[u8], verify_stderr: &[u8]) -> Result<(), String> {
    if let Some(status) = status_stdout {
        write_status(run_dir, status)?;
    }
    let v = json!({
        "exit": verify_exit,
        "stdout": relative_text(run_dir, &String::from_utf8_lossy(verify_stdout)),
        "stderr": relative_text(run_dir, &String::from_utf8_lossy(verify_stderr)),
    });
    let text = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())? + "\n";
    write_atomic(&run_dir.join("tree-verify.json"), text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::current_exe().unwrap().parent().unwrap().join(format!("run-files-unit-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn mkfifo(path: &Path) {
        let c = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o644) }, 0, "mkfifo {}", path.display());
    }

    #[test]
    fn write_atomic_writes_past_a_pipe_and_a_link_at_the_old_temporary_name_and_replaces_what_lies_at_the_name() {
        let d = dir("atomic");
        let outside = d.join("outside.txt");
        std::fs::write(&outside, "keep\n").unwrap();
        mkfifo(&d.join("a.tmp"));
        write_atomic(&d.join("a.json"), b"one\n").unwrap();
        assert_eq!(std::fs::read_to_string(d.join("a.json")).unwrap(), "one\n");
        std::fs::remove_file(d.join("a.tmp")).unwrap();
        std::os::unix::fs::symlink(&outside, d.join("a.tmp")).unwrap();
        write_atomic(&d.join("a.json"), b"two\n").unwrap();
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep\n");
        // a link and a pipe at the name itself are replaced, not written through
        std::fs::remove_file(d.join("a.json")).unwrap();
        std::os::unix::fs::symlink(&outside, d.join("a.json")).unwrap();
        write_atomic(&d.join("a.json"), b"three\n").unwrap();
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep\n");
        assert!(std::fs::symlink_metadata(d.join("a.json")).unwrap().is_file());
        std::fs::remove_file(d.join("a.json")).unwrap();
        mkfifo(&d.join("a.json"));
        write_atomic(&d.join("a.json"), b"four\n").unwrap();
        assert_eq!(std::fs::read_to_string(d.join("a.json")).unwrap(), "four\n");
    }

    #[test]
    fn write_atomic_leaves_no_temporary_file_after_a_write_that_fails() {
        let d = dir("failed");
        std::fs::create_dir(d.join("a.json")).unwrap();
        assert!(write_atomic(&d.join("a.json"), b"x\n").is_err());
        let names: Vec<String> = std::fs::read_dir(&d).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names, vec!["a.json".to_owned()]);
    }
}
