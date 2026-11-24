//! The decider interface (declared by R1, filled by R2 in `decider.rs`) and the two doubles R1
//! ships: the refusing default and the scripted one.

pub use crate::loom_run::answers::{AskRequest, Asked};
use serde_json::Value;
use std::path::Path;

/// What the decider is given for one entry of `decisions[]`.
#[derive(Debug)]
pub struct DecisionRequest<'a> {
    /// The entry of `decisions[]`, unchanged (case, capability, action, kind, path, revision,
    /// subject, command, decides).
    pub decision: &'a Value,
    /// The run directory.
    pub run_dir: &'a Path,
    /// The workspace of the tree (the run directory's `work`).
    pub workspace: &'a Path,
    /// The most the call may cost, micro USD: what the ledger reserved for it.
    pub budget_micro_usd: u64,
    /// The environment variables the runner built for `b10x-loom` children; a decider that starts
    /// a program builds its own environment and never inherits the runner's.
    pub child_env: &'a [(String, String)],
    /// The link to the login file in the run's private directory (`private.rs`), when the settings
    /// name a login file.
    pub login_link: Option<&'a Path>,
}

/// What a decider answers.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    /// The answer. `principal` None: the principal Loom printed in `command`. `cost_micro_usd` None
    /// is an answer without a cost: the run fails closed. A rule answer costs `Some(0)`. `reasons`
    /// is the decider's text of a rejection (None: it gave none); it goes to Loom as the reasons of
    /// `tree approve --result rejected`.
    Answer { approve: bool, principal: Option<String>, cost_micro_usd: Option<u64>, model_calls: u64, reasons: Option<String> },
    /// The decider cannot decide; the run stops with this reason (outcome `stopped: decider`).
    CannotDecide { reason: String },
}

/// Answers one decision.
pub trait Decider {
    /// Reads, once, what the decider routes by (the table of who decides, from the run directory's
    /// copy). The runner calls it after the bindings copy is made and before the first action of
    /// the run: a bound program can write the copy, and a decision of the run is routed from this
    /// read, never from the file as it is later. A decider that routes by nothing does nothing.
    fn pin_routing(&self, _run_dir: &Path) {}
    /// The most one call can cost, micro USD (0 for a rule). The ledger reserves it before the call.
    fn max_call_micro_usd(&self) -> u64;
    /// Whether this decider can start a model program or reserve an amount for one. A run whose
    /// bindings hold no model node still has a model node when its decider says `true` (the
    /// default, so a decider that does not say counts); the answer is read once, at the opening
    /// (`model_node.rs`).
    fn may_call_model(&self) -> bool {
        true
    }
    /// Answers `request`.
    fn decide(&self, request: &DecisionRequest<'_>) -> Verdict;
    /// Whether this decider answers the requests of the answers file (a question or a finding, see
    /// `answers.rs`); when not, a declared decision goes to `decide` as any other.
    fn answers_requests(&self) -> bool {
        false
    }
    /// Answers one request of the answers file.
    fn ask(&self, _request: &AskRequest<'_>) -> Asked {
        Asked::cannot("this decider answers no question or finding")
    }
}

/// The default: no decider is configured; the run stops at the first decision.
pub struct RefusingDecider;

impl Decider for RefusingDecider {
    fn max_call_micro_usd(&self) -> u64 {
        0
    }
    fn may_call_model(&self) -> bool {
        false
    }
    fn decide(&self, _request: &DecisionRequest<'_>) -> Verdict {
        Verdict::CannotDecide { reason: "no decider configured".to_owned() }
    }
}

/// The scripted decider when the settings ask for it (`decider.kind == "scripted"`), else `None`.
/// The loop asks this first, then `decider::from_settings`.
pub fn scripted_from_settings(settings: &Value) -> Option<Result<Box<dyn Decider>, String>> {
    let d = settings.get("decider")?;
    if d.get("kind").and_then(Value::as_str) != Some("scripted") {
        return None;
    }
    Some(ScriptedDecider::from_value(d).map(|s| Box::new(s) as Box<dyn Decider>))
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
    /// "principal": "person:owner", "cost_micro_usd": 0, "model_calls": 0, "reasons": "text of a
    /// rejection"} | {"cannot_decide": "reason"}]}`;
    /// `decider.principal` is the principal of every answer that names none.
    pub fn from_value(decider: &Value) -> Result<Self, String> {
        let max_call = decider.get("max_call_micro_usd").and_then(Value::as_u64).unwrap_or(0);
        let default_principal = decider.get("principal").and_then(Value::as_str);
        let list = decider.get("answers").and_then(Value::as_array).ok_or("decider.answers is not a list")?;
        let mut answers = Vec::new();
        for a in list {
            if let Some(reason) = a.get("cannot_decide").and_then(Value::as_str) {
                answers.push(Verdict::CannotDecide { reason: reason.to_owned() });
                continue;
            }
            let approve = a.get("approve").and_then(Value::as_bool).ok_or("a scripted answer has no approve")?;
            answers.push(Verdict::Answer {
                approve,
                principal: a.get("principal").and_then(Value::as_str).or(default_principal).map(str::to_owned),
                cost_micro_usd: a.get("cost_micro_usd").and_then(Value::as_u64),
                model_calls: a.get("model_calls").and_then(Value::as_u64).unwrap_or(0),
                reasons: a.get("reasons").and_then(Value::as_str).map(str::to_owned),
            });
        }
        Ok(ScriptedDecider::new(answers, max_call))
    }
}

impl Decider for ScriptedDecider {
    fn max_call_micro_usd(&self) -> u64 {
        self.max_call
    }
    /// A scripted decider calls a model when it reserves an amount or any answer says it made a
    /// call or spent something: the double stands for a model decider then.
    fn may_call_model(&self) -> bool {
        let answers = self.answers.lock().unwrap();
        self.max_call > 0 || answers.iter().any(|a| matches!(a, Verdict::Answer { cost_micro_usd, model_calls, .. } if *model_calls > 0 || cost_micro_usd.unwrap_or(0) > 0))
    }
    fn decide(&self, _request: &DecisionRequest<'_>) -> Verdict {
        let mut answers = self.answers.lock().unwrap();
        answers.pop_front().unwrap_or(Verdict::CannotDecide { reason: "the scripted decider has no answer left".to_owned() })
    }
}
