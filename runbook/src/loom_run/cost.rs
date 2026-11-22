//! The cost of a step: the cost records the bound programs wrote since the last step.
//!
//! A model step (`bindings/tools/llm-node.sh`) writes one directory `NNN-<action>/` per call under
//! `<workspace>/.sdc/llm/` when it starts, and one line per finished call into
//! `<workspace>/.sdc/llm/calls.jsonl` with `cost_usd` and `record` (the call's directory).
//! A call directory without a line with a number `cost_usd` is a call of unknown cost.

use serde_json::Value;
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
    #[allow(dead_code)]
    lines_seen: usize,
}

impl CostReader {
    /// The new cost records of `workspace` since the last call. Errors (fail closed): a call
    /// directory with no cost line, a line that is not JSON, a `cost_usd` that is not a finite
    /// non-negative number.
    pub fn read_new(&mut self, _workspace: &Path) -> Result<StepCost, String> {
        Err(crate::loom_run::not_built("cost read"))
    }
}
