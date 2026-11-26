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

impl Terms {
    /// Whether a run with these terms makes model calls the daemon routes: its decider is the kind `model` at a level
    /// above `A0`. Such a run takes a model route (`docs/model-routes.md`, section 2) and its model processes reach it
    /// through the daemon's relay; any other run takes none, and a submit for it that names one is refused.
    pub fn makes_model_calls(&self) -> bool {
        self.decider.as_deref() == Some("model") && self.autonomy.is_some_and(|l| l != Autonomy::A0)
    }
}

/// The setting that chose a run's route.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RouteSource {
    /// The submit named it (`model_route`, `model_routes.<role>`).
    Run,
    /// The daemon's `route_policy.default`, or the route `default` of a lone `[gateway]` table.
    Default,
}

impl RouteSource {
    pub fn as_str(self) -> &'static str {
        match self {
            RouteSource::Run => "run",
            RouteSource::Default => "default",
        }
    }
}

/// The model route of a run as its record keeps it: the route of the role `default`, its alias at the gateway and the
/// setting that chose it, and the route of each further role the submit named. No address and no token.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRoute {
    pub route: String,
    pub alias: String,
    pub source: RouteSource,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub roles: BTreeMap<String, String>,
}

/// A runbook a runtime can start, as the view `runbooks` lists it: a name and the version the runtime
/// has for it, when it names one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunbookInfo {
    pub name: String,
    pub version: Option<String>,
}

/// What the catalog (`Daemon::catalog`) says of a runbook beside its name and version, as the runtime's runbook
/// directories hold it: its purpose in one line and its description (the root protocol's), the inputs a delegation's
/// request fills, and the text of its route entries file (`crate::admission`), or why that file cannot be read. No path.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunbookFacts {
    pub title: Option<String>,
    pub description: Option<String>,
    pub inputs: Vec<String>,
    /// None: the runbook has no route entries file.
    pub routes_file: Option<Result<String, String>>,
}

/// One case of a run's tree as the runtime reads it (`Runtime::cases`): what the view `cases` shows, and the last
/// stop its run log names, from which the daemon says where it stands. No path of the store or a case file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseRow {
    pub case_id: String,
    pub parent: Option<String>,
    /// The link name the opening action gave the case; none for the root.
    pub name: Option<String>,
    pub protocol: String,
    pub outcome: Option<String>,
    /// The stop of the case's last `Stopped` line (`AwaitingPerson`, `AwaitingAuthority`, another stop), if any.
    pub stop: Option<String>,
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
    /// The route of each role of a run that makes model calls (`Terms::makes_model_calls`) or is a delegation, resolved by
    /// the daemon at the submit: the run's model processes reach these routes through the daemon's relay. None: the run
    /// is on no route.
    pub routes: Option<crate::relay::RouteTable>,
    /// The route of a delegation (`Daemon::delegate`), by name: the run is a delegation, on that route as the role
    /// `default` (`routes`). None: the run is no delegation.
    pub model_route: Option<String>,
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
    /// What a request read from its case carries beside its question and its options (`docs/engine.md`,
    /// "A request read from the case"); none for a request the runtime built itself. Only a request that
    /// has one refuses an answer that names none of its options.
    pub artifact: Option<RequestArtifact>,
}

/// The option of `options` that `answer` names, and the text after it: its letter alone, or its letter,
/// a colon and text (`B: too short`; the text trimmed, none when empty). None when it names no option.
/// The option [`FREE`] takes text: its letter alone, or with nothing after the colon, names no option.
pub fn named_option<'a>(options: &'a [Value], answer: &'a str) -> Option<(&'a Value, Option<&'a str>)> {
    options.iter().find_map(|o| {
        let letter = o.get("letter")?.as_str().filter(|l| !l.is_empty())?;
        if answer == letter {
            return (letter != FREE).then_some((o, None));
        }
        let text = answer.strip_prefix(letter)?.strip_prefix(':')?.trim();
        match (letter == FREE, text.is_empty()) {
            (true, true) => None,
            (_, empty) => Some((o, (!empty).then_some(text))),
        }
    })
}

/// The letter of the option that takes text (`free: <text>`): a declared result `free` of an action is
/// offered as this option.
pub const FREE: &str = "free";

impl RequestArtifact {
    /// Whether a request with this artifact and `options` takes `answer`: it names an option, or the
    /// request takes a free answer.
    pub fn takes(&self, options: &[Value], answer: &str) -> bool {
        self.free_answer || named_option(options, answer).is_some()
    }
}

/// The recommendation, the grounds and the free answer of a request read from its case: the members of
/// the decision contract (`sdc.decision.DecisionRequest`) the daemon shows beside the options.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct RequestArtifact {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_option: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_answer: Option<String>,
    /// `none`, `weak`, `moderate` or `strong` (`sdc.decision.Strength`).
    pub recommendation_strength: String,
    #[serde(default)]
    pub grounds: Vec<Value>,
    /// Whether an answer that names no option is taken as a free answer.
    #[serde(default)]
    pub free_answer: bool,
}

/// A program of a run that is about to start (or has ended): what the runtime knows of it before
/// the program runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Started {
    /// Counts the programs of one run, from 1: a start and its end carry the same number.
    pub serial: u64,
    /// `program` for the program of an action, `check` for its `handoff_when` check.
    pub role: &'static str,
    /// The case the program runs for (Loom's `LOOM_CASE` of the child), when the child has one.
    pub case: Option<String>,
    /// The action the bindings bind to the program, when exactly one does.
    pub action: Option<String>,
    /// The nth start of this action in this case, for the program of an action.
    pub try_number: Option<u64>,
}

/// Where the runtime tells that a bound program is about to start and that it ended, on the thread
/// that starts the program.
pub trait StartTarget: Send + Sync {
    /// Called before the program starts.
    fn started(&self, start: &Started);
    /// Called after the program ended, however it ended.
    fn ended(&self, _start: &Started) {}
}

/// What the runtime reports while a run goes. Called on the run's thread, from inside `start`.
pub trait EventSink: Send + Sync {
    /// Where the runtime tells the start and the end of each bound program; none when the sink
    /// takes no such report (the stub runs no program). There is no default: a sink that wraps
    /// another forwards it, and one that takes no report says `None`.
    fn steps(&self) -> Option<std::sync::Arc<dyn StartTarget>>;

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
    /// There is no default: a sink that wraps another forwards it, and one that takes no answer says `false`.
    fn decision_answered(&self, request_id: &str, answer: &str, by: &str, principal: &str) -> bool;
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

    /// What the catalog says of the runbook `runbook` (a name `runbooks` lists) beside its name. The default (the stub,
    /// whose runbooks are scripts): nothing.
    fn runbook_facts(&self, _runbook: &str) -> RunbookFacts {
        RunbookFacts::default()
    }

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

    /// The cases of the run's tree, in the order the runtime lists them, or that they cannot be read now (the
    /// reason is for the log and is never shown to a client). A runtime with no tree (the stub) has none, and so
    /// has a run whose tree is not written yet.
    fn cases(&self, _run_id: &str) -> Result<Vec<CaseRow>, String> {
        Ok(Vec::new())
    }

    /// Whether the runtime runs the free run of a delegation (`crate::delegation`).
    fn free_run(&self) -> bool {
        false
    }

    /// Whether the runtime classifies a delegation that names no protocol when its run starts, on the daemon's classifier
    /// route (`[route_policy] classifier`; `crate::delegation`, `docs/model-routes.md` section 9). None (the stub): such a
    /// delegation is the free run.
    fn classifies(&self) -> bool {
        false
    }
}
