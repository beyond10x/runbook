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
}

impl BasicFiles {
    /// Opens the writer for `run_dir`.
    pub fn open(run_dir: &Path) -> Result<BasicFiles, String> {
        let path = run_dir.join("events.jsonl");
        let log = OpenOptions::new().create_new(true).append(true).open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(BasicFiles { run_dir: run_dir.to_path_buf(), log })
    }
}

/// The JSON object of one event: `{"at": secs, "event": "<name>", ...}`.
pub fn event_json(at_secs: u64, event: &Event) -> Value {
    let (name, fields) = match event {
        Event::RunStarted { run, settings } => ("run_started", json!({"run": run, "settings": settings})),
        Event::StepMade { index, exit, stop, steps } => ("step_made", json!({"index": index, "exit": exit, "stop": stop, "steps": steps})),
        Event::DecisionAsked { decision } => ("decision_asked", json!({"decision": decision})),
        Event::DecisionAnswered { decision, approve, principal, cost_micro_usd, model_calls } => (
            "decision_answered",
            json!({"decision": decision, "approve": approve, "principal": principal, "cost_micro_usd": cost_micro_usd, "model_calls": model_calls}),
        ),
        Event::CostedCall { source, cost_micro_usd, record } => {
            let source = match source {
                CostSource::Step => "step",
                CostSource::Decider => "decider",
            };
            ("costed_call", json!({"source": source, "cost_micro_usd": cost_micro_usd, "record": record}))
        }
        Event::CapReached { cap, detail } => {
            let cap = match cap {
                Cap::Spend => "spend",
                Cap::LlmCalls => "llm_calls",
                Cap::WallTime => "wall_time",
            };
            ("cap_reached", json!({"cap": cap, "detail": detail}))
        }
        Event::TreeStored { status_exit, verify_exit } => ("tree_stored", json!({"status_exit": status_exit, "verify_exit": verify_exit})),
        Event::Ended { outcome, reason, loom_exit, stderr_tail, exit_code, steps, spent_micro_usd, llm_calls } => (
            "ended",
            json!({"outcome": outcome, "reason": reason, "loom_exit_code": loom_exit, "stderr_tail": stderr_tail, "exit_code": exit_code, "steps": steps, "spent_micro_usd": spent_micro_usd, "llm_calls": llm_calls}),
        ),
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
        writeln!(self.log, "{line}").map_err(|e| format!("events.jsonl: {e}"))?;
        if let Event::Ended { outcome, reason, loom_exit, stderr_tail, exit_code, steps, spent_micro_usd, llm_calls } = event {
            let v = json!({
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
            let text = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())? + "\n";
            std::fs::write(self.run_dir.join("outcome.json"), text).map_err(|e| format!("outcome.json: {e}"))?;
        }
        Ok(())
    }
}

/// Writes `tree-status.json` (what `tree status` printed, byte for byte) and `tree-verify.json`
/// (`{"exit", "stdout", "stderr"}` of `tree verify`).
pub fn write_tree_files(run_dir: &Path, status_stdout: &[u8], verify_exit: i32, verify_stdout: &[u8], verify_stderr: &[u8]) -> Result<(), String> {
    std::fs::write(run_dir.join("tree-status.json"), status_stdout).map_err(|e| format!("tree-status.json: {e}"))?;
    let v = json!({
        "exit": verify_exit,
        "stdout": String::from_utf8_lossy(verify_stdout),
        "stderr": String::from_utf8_lossy(verify_stderr),
    });
    let text = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())? + "\n";
    std::fs::write(run_dir.join("tree-verify.json"), text).map_err(|e| format!("tree-verify.json: {e}"))
}
