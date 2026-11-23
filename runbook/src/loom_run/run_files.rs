//! The files of this unit in the run directory: `settings.json`, `events.jsonl` (one JSON object
//! per event), `outcome.json`, `tree-status.json` and `tree-verify.json`.

use crate::loom_run::events::{Cap, CostSource, Event, EventSink};
use serde_json::{json, Value};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Writes R1's files as events arrive.
pub struct BasicFiles {
    run_dir: PathBuf,
    log: File,
    /// The decision of the opening, written into `outcome.json` as it was made; absent when the run
    /// ended before the opening decided.
    has_model_node: Option<bool>,
}

impl BasicFiles {
    /// Opens the writer for `run_dir`.
    pub fn open(run_dir: &Path) -> Result<BasicFiles, String> {
        let path = run_dir.join("events.jsonl");
        let log = OpenOptions::new().create_new(true).append(true).open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(BasicFiles { run_dir: run_dir.to_path_buf(), log, has_model_node: None })
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

/// Writes `bytes` to `path` through a temporary file and a rename, so a reader never sees half a file.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// The JSON object of one event: `{"at": secs, "event": "<name>", ...}`.
pub fn event_json(at_secs: u64, event: &Event) -> Value {
    let (name, fields) = match event {
        Event::RunStarted { run, settings } => ("run_started", json!({"run": run, "settings": settings})),
        Event::ModelNodeDecided { has_model_node, because } => ("model_node_decided", json!({"has_model_node": has_model_node, "because": because})),
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
                Cap::ModelNode => "model_node",
            };
            ("cap_reached", json!({"cap": cap, "detail": detail}))
        }
        Event::TreeStored { status_exit, verify_exit } => ("tree_stored", json!({"status_exit": status_exit, "verify_exit": verify_exit})),
        Event::Ended { outcome, reason, cause, loom_exit, stderr_tail, exit_code, steps, spent_micro_usd, llm_calls } => {
            let mut fields = json!({"outcome": outcome, "reason": reason, "loom_exit_code": loom_exit, "stderr_tail": stderr_tail, "exit_code": exit_code, "steps": steps, "spent_micro_usd": spent_micro_usd, "llm_calls": llm_calls});
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
        if let Event::ModelNodeDecided { has_model_node, .. } = event {
            self.has_model_node.get_or_insert(*has_model_node);
        }
        let line = serde_json::to_string(&event_json(at_secs, event)).map_err(|e| e.to_string())?;
        let line = relative_text(&self.run_dir, &line);
        writeln!(self.log, "{line}").map_err(|e| format!("events.jsonl: {e}"))?;
        if let Event::Ended { outcome, reason, cause, loom_exit, stderr_tail, exit_code, steps, spent_micro_usd, llm_calls } = event {
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
            });
            if let (Some(cause), Some(map)) = (cause, v.as_object_mut()) {
                map.insert("cause".to_owned(), Value::from(cause.as_str()));
            }
            if let (Some(has), Some(map)) = (self.has_model_node, v.as_object_mut()) {
                map.insert("has_model_node".to_owned(), Value::from(has));
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
