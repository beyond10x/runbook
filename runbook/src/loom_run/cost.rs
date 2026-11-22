//! The cost of a step: the cost records the bound programs wrote since the last step.
//!
//! A model step (`bindings/tools/llm-node.sh`) writes one directory `NNN-<action>/` per call under
//! `<workspace>/.sdc/llm/` when it starts, and one line per finished call into
//! `<workspace>/.sdc/llm/calls.jsonl` with `cost_usd` and `record` (the call's directory).
//! A call directory without a line with a number `cost_usd` is a call of unknown cost.

use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;

/// What one reading of the records found.
#[derive(Debug, Default, PartialEq)]
pub struct StepCost {
    /// Cost of the new calls, micro USD.
    pub micro_usd: u64,
    /// The new cost lines as read.
    pub records: Vec<Value>,
}

/// Remembers how far the records were read.
#[derive(Debug, Default)]
pub struct CostReader {
    lines_seen: usize,
    recorded_dirs: HashSet<String>,
    checked_dirs: HashSet<String>,
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

impl CostReader {
    /// The new cost records of `workspace` since the last call. Errors (fail closed): a call
    /// directory with no cost line, a line that is not JSON, a `cost_usd` that is not a finite
    /// non-negative number.
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
                let usd = v.get("cost_usd").ok_or("a cost record has no cost_usd")?;
                let micro = usd_to_micro(usd)?;
                cost.micro_usd = cost.micro_usd.checked_add(micro).ok_or("cost overflow")?;
                if let Some(name) = v.get("record").and_then(Value::as_str).and_then(|p| p.rsplit('/').next()) {
                    self.recorded_dirs.insert(name.to_owned());
                }
                cost.records.push(v);
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
                    return Err(format!("model call {name} has no cost record"));
                }
                self.checked_dirs.insert(name);
            }
        }
        Ok(cost)
    }
}
