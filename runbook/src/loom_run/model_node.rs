//! Whether a run has a model node: decided once, at the opening, and never again.
//!
//! The decision is a record and a gate for the login. It is a value of the run ([`ModelNode`]), made
//! after the run's copy of the bindings exists and before the first `tree run`, from what the runner
//! itself reads: the options, the settings, the decider the settings build and the bindings copy.
//! It is written into the records (the event `model_node_decided`, `has_model_node` in
//! `outcome.json`), and the link to the login file is made only for a run that has one. It limits
//! nothing: no cap, no refusal and no ledger depends on it (`--spend-ledger`, `--cap-micro-usd` and
//! `--max-llm-calls` are optional for every run, and a cap that is named counts every model call
//! the runner records, whatever the decision was).
//!
//! Unclear counts as "with": every reading that fails, every form the rule does not know.
//! The forms are listed with [`bindings_copy::model_node_in`] and below.

use crate::loom_run::bindings_copy;
use crate::loom_run::decide::Decider;
use crate::loom_run::ports::Conformance;
use crate::loom_run::Args;
use serde_json::Value;
use std::path::Path;

/// The decision of the opening.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelNode {
    /// The run has a model node (or it is unclear); `because` names the first form that counted.
    With { because: String },
    /// The run has none: no binding, decider or option of it starts a model program.
    Without,
}

impl ModelNode {
    /// The value the records carry as `has_model_node`: a form of the bindings, the decider or the
    /// options names a model node.
    pub fn has_model_node(&self) -> bool {
        matches!(self, ModelNode::With { .. })
    }

    /// Why, for the opening event.
    pub fn because(&self) -> String {
        match self {
            ModelNode::With { because } => because.clone(),
            ModelNode::Without => "no model node: no binding, decider or option of the run starts a model program".to_owned(),
        }
    }
}

/// Decides whether the run has a model node. `bindings` is the directory to read: the run's copy.
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
