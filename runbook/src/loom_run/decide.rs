//! The decider interface (declared by R1, filled by R2 in `decider.rs`) and the two doubles R1
//! ships: the refusing default and the scripted one.

use serde_json::Value;
use std::path::Path;

/// What the decider is given for one entry of `decisions[]`.
#[derive(Debug)]
pub struct DecisionRequest<'a> {
    /// The entry of `decisions[]`, unchanged (case, capability, action, kind, path, revision,
    /// subject, command, decides).
    pub decision: &'a Value,
    /// The run's settings, as read (the login file's path included).
    pub settings: &'a Value,
    /// The run directory.
    pub run_dir: &'a Path,
    /// The workspace of the tree (the run directory's `work`).
    pub workspace: &'a Path,
    /// The most the call may cost, micro USD: what the ledger reserved for it.
    pub budget_micro_usd: u64,
    /// The environment variables the runner built for `b10x-loom` children; a decider that starts
    /// a program builds its own environment and never inherits the runner's.
    pub child_env: &'a [(String, String)],
}

/// What a decider answers.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    /// The answer. `principal` None: the principal Loom printed in `command`. `cost_micro_usd` None
    /// is an answer without a cost: the run fails closed. A rule answer costs `Some(0)`.
    Answer { approve: bool, principal: Option<String>, cost_micro_usd: Option<u64>, model_calls: u64 },
    /// The decider cannot decide; the run stops with this reason (outcome `stopped: decider`).
    CannotDecide { reason: String },
}

/// Answers one decision.
pub trait Decider {
    /// The most one call can cost, micro USD (0 for a rule). The ledger reserves it before the call.
    fn max_call_micro_usd(&self) -> u64;
    /// Answers `request`.
    fn decide(&self, request: &DecisionRequest<'_>) -> Verdict;
}

/// The default: no decider is configured; the run stops at the first decision.
pub struct RefusingDecider;

impl Decider for RefusingDecider {
    fn max_call_micro_usd(&self) -> u64 {
        0
    }
    fn decide(&self, _request: &DecisionRequest<'_>) -> Verdict {
        Verdict::CannotDecide { reason: "no decider configured".to_owned() }
    }
}

/// A test double: answers from a list in order, then cannot decide. Selected by the settings
/// (`"decider": {"kind": "scripted", "answers": [...]}`, see `docs/loom-run.md`).
pub struct ScriptedDecider {
    answers: std::sync::Mutex<std::collections::VecDeque<Verdict>>,
    max_call: u64,
}

impl ScriptedDecider {
    /// A decider that gives `answers` in order; `max_call_micro_usd` is what each call reserves.
    pub fn new(answers: Vec<Verdict>, max_call_micro_usd: u64) -> Self {
        ScriptedDecider { answers: std::sync::Mutex::new(answers.into()), max_call: max_call_micro_usd }
    }

    /// Reads `{"kind": "scripted", "max_call_micro_usd": n, "answers": [{"approve": true,
    /// "principal": "person:owner", "cost_micro_usd": 0, "model_calls": 0} | {"cannot_decide": "reason"}]}`.
    pub fn from_value(_decider: &Value) -> Result<Self, String> {
        Err(crate::loom_run::not_built("scripted decider"))
    }
}

impl Decider for ScriptedDecider {
    fn max_call_micro_usd(&self) -> u64 {
        self.max_call
    }
    fn decide(&self, _request: &DecisionRequest<'_>) -> Verdict {
        let mut answers = self.answers.lock().unwrap();
        answers.pop_front().unwrap_or(Verdict::CannotDecide { reason: "the scripted decider has no answer left".to_owned() })
    }
}
