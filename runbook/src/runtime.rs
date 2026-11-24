//! The runtime the daemon runs its runs on.
//!
//! The shape is the one Loom's library offers: a start call that BLOCKS until the run ends, takes an
//! observer that is called once per run-log line, and takes an `AtomicBool` that the runtime checks
//! between steps. The daemon runs each run on its own plain thread. Loom replaces the stub of
//! `stub.rs` by implementing this trait; the daemon, its commands and its API do not change.

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

    /// Whether `self` is at least as strong as `floor`. The one comparison of two levels: every pair
    /// is written out, so a level added to the enum does not compile until it is placed here; there
    /// is no arm that passes what it did not name.
    pub fn at_least(self, floor: Confinement) -> bool {
        match (self, floor) {
            (Confinement::Local, Confinement::Local) => true,
            (Confinement::Linux, Confinement::Local) => true,
            (Confinement::Linux, Confinement::Linux) => true,
            (Confinement::Local, Confinement::Linux) => false,
        }
    }
}

/// Where the level of a run came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfinementSource {
    /// The submit named it.
    Submit,
    /// The daemon's default level (`--default-confinement`, or `confinement` of `runbook.toml`).
    DaemonDefault,
}

impl ConfinementSource {
    pub fn as_str(self) -> &'static str {
        match self {
            ConfinementSource::Submit => "submit",
            ConfinementSource::DaemonDefault => "daemon-default",
        }
    }
}

/// How much of a run's decisions a decider takes over from a person (`docs/autonomy-levels.md`), the
/// levels of the specification (`sdc.decision.AutonomyLevel`). What a level gives to the decider is
/// the table of the runbook's own `tools/autonomy-levels.json`; the daemon only carries the name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Autonomy {
    A0,
    A1,
    A2,
    A3,
    A4,
    A5,
}

impl Autonomy {
    pub const ALL: [Autonomy; 6] = [Autonomy::A0, Autonomy::A1, Autonomy::A2, Autonomy::A3, Autonomy::A4, Autonomy::A5];

    pub fn as_str(self) -> &'static str {
        match self {
            Autonomy::A0 => "A0",
            Autonomy::A1 => "A1",
            Autonomy::A2 => "A2",
            Autonomy::A3 => "A3",
            Autonomy::A4 => "A4",
            Autonomy::A5 => "A5",
        }
    }

    pub fn parse(text: &str) -> Option<Autonomy> {
        Autonomy::ALL.into_iter().find(|a| a.as_str() == text)
    }
}

/// What a run is given besides its runbook: the autonomy level and the kind of decider that takes the
/// decisions the level leaves to it. Fixed when the run is submitted and read from its record after
/// that, never from the daemon's policy of the day. Neither: every decision waits for a client.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Terms {
    #[serde(default)]
    pub autonomy: Option<Autonomy>,
    /// A kind name among the kinds the operator configured (`runbook.toml`, `[deciders.<kind>]`).
    #[serde(default)]
    pub decider: Option<String>,
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
    /// The autonomy level and the decider kind of the run.
    pub terms: Terms,
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
    /// A rule or a decider of the run answered a request the run raised. `by` and `principal` are
    /// `rule:<name>` or `decider:<kind>`; the daemon records the answer under exactly those and refuses
    /// anything else, a person's name or a label a client typed. `true`: it is recorded and the runtime
    /// may apply the answer; `false`: the request stays open for a client and the runtime applies nothing.
    fn decision_answered(&self, _request_id: &str, _answer: &str, _by: &str, _principal: &str) -> bool {
        false
    }
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
}
