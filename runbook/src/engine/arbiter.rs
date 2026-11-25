//! Who answers a wait on the daemon's runs besides a client: the seam between the engine (library)
//! and the decider of `runbook run` (binary).
//!
//! The engine does not know a decider. It asks an [`Arbiter`] for the kinds the operator configured and,
//! for a run that has an autonomy level and a decider kind, for a [`RunArbiter`] that judges each
//! decision Loom printed. The binary implements both with the decider and the routing `runbook run`
//! uses (`loom_run::decider`), and hands the answer back as the text a client's answer has
//! (`decision::parse_answer`), so the engine applies it through the one call it applies a client's
//! answer with (`calls::apply`). A decision the level leaves to a person, and a decider that fails,
//! are `Judgement::Open`: the request stays open for `answer_decision`.

use crate::runtime::Terms;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// What a run's arbiter is opened with.
pub struct Opening<'a> {
    /// The run's directory: its store, its bindings copy and the record of its decider.
    pub run_dir: &'a Path,
    /// The run's workspace.
    pub workspace: &'a Path,
    /// The root bindings file of the runbook as it is installed (the original: it is never written).
    pub bindings: &'a Path,
    /// The run's level and kind.
    pub terms: &'a Terms,
    /// The run has a store already: its bindings copy was made when it started and is read, not made again.
    pub resume: bool,
    /// The run's route file when the run is on a route (`engine::route`): the decider's model program gets its path
    /// and nothing else of the route.
    pub route_file: Option<&'a Path>,
}

/// What opening a run's arbiter gives.
pub struct Opened {
    /// The root bindings file the tree is opened with: the run's own copy, in which the person steps the
    /// level gives to the decider name the decider's principal.
    pub bindings: PathBuf,
    pub judge: Box<dyn RunArbiter>,
}

/// What a judge makes of one decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Judgement {
    /// The decision stays open for a client. The words say why: the level gives it to a person, the
    /// decider could not decide, or the table of who decides could not be read.
    Open(String),
    /// The decider answered.
    Answer {
        /// The answer as a client writes it (`approve`, `deny`, a result name or a JSON object).
        answer: String,
        /// Who gave it: `rule:<name>` or `decider:<kind>`. Never a person and never a client's label.
        by: String,
        /// The principal Loom is given for it.
        principal: String,
    },
}

/// The deciders of a daemon.
pub trait Arbiter: std::fmt::Debug + Send + Sync + 'static {
    /// The names of the kinds the operator configured, sorted.
    fn kinds(&self) -> Vec<String>;

    /// A judge for one run, and the bindings copy it opens its tree with. An error ends the run before
    /// anything of the tree is written.
    fn open(&self, opening: &Opening<'_>) -> Result<Opened, String>;
}

/// Judges the decisions of one run, one at a time, on the run's thread.
pub trait RunArbiter: Send {
    /// `decision` is the entry of `decisions[]` as Loom printed it.
    fn judge(&mut self, decision: &Value) -> Judgement;
}
