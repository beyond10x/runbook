//! Whether a run has a model node: decided once, at the opening, and never again.
//!
//! The caps of a run (`--max-llm-calls`, `--cap-micro-usd`, the spend ledger) guard model calls. A
//! run that has no model node makes none, so it counts no call, reserves nothing and opens no
//! ledger; a run that has one keeps every cap as it is. The decision is a value of the run
//! ([`ModelNode`]), made after the run's copy of the bindings exists and before the first `tree
//! run`, from what the runner itself reads: the options, the settings, the decider the settings
//! build and the bindings copy. It is written into the records and read by every path that can
//! start a model program (`run_loop.rs`, `conformance.rs`), which refuses when the decision was
//! "no model node". A file a bound program writes later never turns "with" into "without"; it can
//! only make a path refuse (`Runner::refuse_model_path`).
//!
//! Unclear counts as "with": every reading that fails, every form the rule does not know.
//! The forms are listed with [`bindings_copy::model_node_in`] and below.

use crate::loom_run::bindings_copy;
use crate::loom_run::decide::Decider;
use crate::loom_run::ports::Conformance;
use crate::loom_run::Args;
use serde_json::Value;
use std::path::Path;

/// The named cause of a run that ended because a model program was about to start in a run opened
/// without a model node (`run-stopped.json`, `outcome.json`, `docs/loom-run.md`).
pub const CAUSE: &str = crate::loom_run::events::MODEL_CALL_CAUSE;

/// The decision of the opening.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelNode {
    /// The run has a model node (or it is unclear); `because` names the first form that counted.
    With { because: String },
    /// The run has none.
    Without,
}

impl ModelNode {
    /// Whether the caps apply.
    pub fn is_with(&self) -> bool {
        matches!(self, ModelNode::With { .. })
    }

    /// The value the records carry as `has_model_node`.
    pub fn has_model_node(&self) -> bool {
        self.is_with()
    }

    /// Why, for the opening event.
    pub fn because(&self) -> String {
        match self {
            ModelNode::With { because } => because.clone(),
            ModelNode::Without => "no model node: no binding, decider or option of the run starts a model program".to_owned(),
        }
    }
}

// What a test sets to make the opening decide a way the files would not: the only way to reach
// the model paths of a run that was opened without a model node by something other than a file a
// program rewrote. It does not exist outside the unit tests.
#[cfg(test)]
thread_local! {
    pub static FORCED: std::cell::RefCell<std::collections::VecDeque<ModelNode>> = const { std::cell::RefCell::new(std::collections::VecDeque::new()) };
}

/// Decides whether the run has a model node. `bindings` is the directory to read: the run's copy
/// at the opening (the source directory for the refusal that comes before the copy exists).
///
/// Counts as a model node, in this order:
///
/// 1. a re-drive of a recording (`--recorded`) or a `replay` setting: its calls may go live;
/// 2. `--conformance observe`: after the run a model writes the tables of the observation;
/// 3. a decider that may call a model (`kind` `model`, a scripted double that reserves an amount or
///    reports a call or a cost, any decider that does not say it calls none);
/// 4. a form of the bindings that names the model tool or that the rule cannot read
///    ([`bindings_copy::model_node_in`]).
///
/// The `adapter`, `models`, `efforts`, `review` and `login_file` settings only change the argv of a
/// model node that the bindings hold, so they decide nothing of their own.
pub fn decide(args: &Args, settings: &Value, decider: &dyn Decider, bindings: &Path) -> ModelNode {
    #[cfg(test)]
    if let Some(forced) = FORCED.with(|f| f.borrow_mut().pop_front()) {
        return forced;
    }
    let with = |because: &str| ModelNode::With { because: because.to_owned() };
    if args.ports.replay.recorded.is_some() || settings.get("replay").is_some() {
        return with("a re-drive of a recording: its calls may go live");
    }
    if args.ports.conformance == Conformance::Observe {
        return with("--conformance observe: a model writes the tables of the observation");
    }
    if decider.may_call_model() || decider.max_call_micro_usd() > 0 {
        return with("the decider calls a model");
    }
    match bindings_copy::model_node_in(bindings) {
        Some(why) => ModelNode::With { because: format!("the bindings name or may name the model tool {}: {why}", bindings_copy::MODEL_TOOL) },
        None => ModelNode::Without,
    }
}
