//! The runtime the daemon runs its runs on.
//!
//! The shape is the one Loom's library offers: a start call that BLOCKS until the run ends, takes an
//! observer that is called once per run-log line, and takes an `AtomicBool` that the runtime checks
//! between steps. The daemon runs each run on its own plain thread. Loom replaces the stub of
//! `stub.rs` by implementing this trait; the daemon, its commands and its API do not change.

use crate::caps::{Caps, Usage};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

/// How strongly a run is sandboxed, weakest first (the runtime's names).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confinement {
    Local,
    Linux,
}

impl Confinement {
    pub const ALL: [Confinement; 2] = [Confinement::Local, Confinement::Linux];

    pub fn as_str(self) -> &'static str {
        match self {
            Confinement::Local => "local",
            Confinement::Linux => "linux",
        }
    }

    pub fn parse(text: &str) -> Option<Confinement> {
        Confinement::ALL.into_iter().find(|c| c.as_str() == text)
    }

    /// Position in the order, weakest first.
    pub fn rank(self) -> u8 {
        self as u8
    }
}

/// A runbook a runtime can start, as the view `runbooks` lists it: a name and the version the runtime
/// has for it, when it names one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunbookInfo {
    pub name: String,
    pub version: Option<String>,
}

/// Everything a run is started with.
#[derive(Clone, Debug)]
pub struct StartRequest {
    pub run_id: String,
    pub runbook: String,
    pub version: Option<String>,
    pub intake: String,
    /// The context directory as submitted (already copied into `workspace_dir` by the daemon).
    pub context_dir: Option<PathBuf>,
    pub settings: BTreeMap<String, String>,
    pub confinement: Confinement,
    /// The run's record: its store and logs.
    pub state_dir: PathBuf,
    /// The run's workspace, a child of the one shared `workspaces` directory.
    pub workspace_dir: PathBuf,
}

/// A question the runtime raised for a run.
#[derive(Clone, Debug, PartialEq)]
pub struct DecisionRequest {
    pub request_id: String,
    pub case_id: String,
    pub action: String,
    pub capability: String,
    pub question: String,
    pub options: Vec<Value>,
}

/// What the runtime reports while a run goes. Called on the run's thread, from inside `start`.
pub trait EventSink: Send + Sync {
    /// The runtime took the run up.
    fn started(&self);
    /// One run-log line: its kind and the record, unchanged.
    fn record(&self, kind: &str, record: Value);
    /// The run stops on a decision.
    fn decision_requested(&self, request: DecisionRequest);
    /// Every decision the run waited on is answered and the runtime took the answers up.
    fn resumed(&self);
}

/// How a run that started ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Finish {
    /// The run reached an outcome (open text: the root case's outcome name).
    Outcome(String),
    /// The cancel flag was seen and the run stopped.
    Cancelled,
}

/// The runtime could not start the run, or failed after it started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeFailure {
    pub reason: String,
}

/// The runtime cannot take an answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnswerFailure {
    /// The runtime holds no live run of this id (for instance after a restart of the daemon).
    UnknownRun,
    Refused(String),
}

/// What the runtime answers when the daemon starts and asks about a run its record holds as
/// `Running` or `WaitingOnDecision`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recovery {
    /// The runtime has the run and goes on with it; the daemon leaves the run as it is.
    Resumed,
    /// The runtime holds no live run of this id: the daemon ends it as orphaned.
    Gone,
}

pub trait Runtime: Send + Sync + 'static {
    /// Asked once at the daemon's start for each run the record holds as live (`Running` or
    /// `WaitingOnDecision`). A runtime whose store lets a run go on answers `Resumed`; the stub
    /// answers `Gone`. How a resumed run reports to the daemon again is the engine's story.
    fn recover(&self, run_id: &str) -> Recovery;

    /// Whether the runtime has a runbook of this name (and version).
    fn knows_runbook(&self, runbook: &str, version: Option<&str>) -> bool;

    /// The runbooks this runtime has, each one a name `knows_runbook` accepts. Order does not matter.
    fn runbooks(&self) -> Vec<RunbookInfo>;

    /// The runtime's name as the view `runbooks` gives it: `stub` or `loom`.
    fn kind(&self) -> &'static str;

    /// Run to the end. Blocks. `sink.started()` is called once the run is up; an `Err` before that
    /// is a run that could not start. The `cancel` flag is checked between steps.
    fn start(&self, request: StartRequest, sink: &dyn EventSink, cancel: &AtomicBool) -> Result<Finish, RuntimeFailure>;

    /// Hand an answer to a waiting run.
    fn answer(&self, run_id: &str, request_id: &str, answer: &str, principal: &str) -> Result<(), AnswerFailure>;

    /// Ask the run to stop (the daemon has also set the run's cancel flag).
    fn cancel(&self, run_id: &str);

    /// The caps a run submitted with these `settings` has, or why they are refused (a cap over the
    /// runtime's ceiling). `None`: this runtime runs nothing that a cap bounds (the stub). Asked at
    /// submit, before anything is written.
    fn resolve_caps(&self, _settings: &BTreeMap<String, String>) -> Result<Option<Caps>, String> {
        Ok(None)
    }

    /// What the run has used so far, when the runtime counts it.
    fn usage(&self, _run_id: &str) -> Option<Usage> {
        None
    }

    /// Why a run of the runbook is refused at load, when it is (the engine: a per-call budget that is
    /// no number of USD, or programs that could call a model with no budget anywhere). Asked at
    /// submit, before anything is written; the reason names no path.
    fn check_runbook(&self, _runbook: &str, _version: Option<&str>) -> Result<(), String> {
        Ok(())
    }
}
