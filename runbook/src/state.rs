//! The state of the daemon: sessions, runs, decision requests and run events as the specification
//! (`ess/domains/daemon.yaml`) gives them, and the journal that keeps them across a restart.
//!
//! One writer, one journal: every change of the state is a `Fact`, written as one JSON line to
//! `<state>/journal.jsonl` and then applied; opening the state replays the journal. Nothing but
//! `commit` changes the state, so a replay gives the state the process had. The journal is not
//! synced to the disk per fact: it survives the process, not a power cut.

use crate::clock::{format_instant, Clock, SystemClock};
use crate::runtime::{Confinement, ConfinementSource, Terms};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RunState {
    Submitted,
    /// The run names a serial key and a live run with that key was submitted before it: the daemon holds the run until
    /// none is left.
    WaitingOnKey,
    /// A route of the run could not answer at the submit: the daemon holds the run until it can, or its wait is over.
    WaitingForRoute,
    Running,
    WaitingOnDecision,
    Ended,
    Cancelled,
    FailedToStart,
}

impl RunState {
    pub const ALL: [RunState; 8] = [
        RunState::Submitted,
        RunState::WaitingOnKey,
        RunState::WaitingForRoute,
        RunState::Running,
        RunState::WaitingOnDecision,
        RunState::Ended,
        RunState::Cancelled,
        RunState::FailedToStart,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            RunState::Submitted => "Submitted",
            RunState::WaitingOnKey => "WaitingOnKey",
            RunState::WaitingForRoute => "WaitingForRoute",
            RunState::Running => "Running",
            RunState::WaitingOnDecision => "WaitingOnDecision",
            RunState::Ended => "Ended",
            RunState::Cancelled => "Cancelled",
            RunState::FailedToStart => "FailedToStart",
        }
    }

    pub fn parse(text: &str) -> Option<RunState> {
        RunState::ALL.into_iter().find(|s| s.as_str() == text)
    }

    /// Not terminal: Submitted, WaitingOnKey, WaitingForRoute, Running or WaitingOnDecision.
    pub fn is_live(self) -> bool {
        matches!(self, RunState::Submitted | RunState::WaitingOnKey | RunState::WaitingForRoute | RunState::Running | RunState::WaitingOnDecision)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    Open,
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionState {
    Open,
    Answered,
    Withdrawn,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DaemonRow {
    pub daemon_id: String,
    pub confinement_floor: Confinement,
    /// Who the daemon acts as; a journal line written before the daemon had one has none (the row lives as long as the process, so a replayed one is never read).
    #[serde(default)]
    pub principal: String,
    pub started_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub session_id: String,
    pub client: String,
    pub state: SessionState,
    pub opened_at: String,
    pub closed_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Run {
    pub run_id: String,
    pub runbook_id: String,
    pub runbook_version: Option<String>,
    pub intake: String,
    pub context_path: Option<String>,
    pub settings: BTreeMap<String, String>,
    pub confinement: Confinement,
    /// Where the level came from; a journal line of a run submitted before the daemon kept it has none.
    #[serde(default)]
    pub confinement_source: Option<ConfinementSource>,
    /// The autonomy level and the decider kind the run was submitted with; a journal line of a run
    /// submitted before the daemon kept them has neither.
    #[serde(default)]
    pub terms: Terms,
    /// The model route of a run that makes model calls, fixed at the submit; none for any other run, and in a journal
    /// line of a run submitted before the daemon kept it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_route: Option<crate::runtime::RunRoute>,
    /// The last phase the gateway gave of a route of the run that could not answer at the submit: set while the run
    /// waits for it and when it ends `stopped: route`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_phase: Option<String>,
    pub session_id: Option<String>,
    pub idempotency_key: Option<String>,
    /// The serial key the submit named: the run waits while a live run with the same key submitted before it has not
    /// ended. None for a run that named none, and in a journal line of a run submitted before the daemon kept it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial_key: Option<String>,
    pub submitted_at: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub outcome: Option<String>,
    pub failure: Option<String>,
    pub state: RunState,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DecisionRequest {
    pub request_id: String,
    pub run_id: String,
    pub case_id: String,
    pub action: String,
    pub capability: String,
    pub question: String,
    pub options: Vec<Value>,
    /// What the request read from its case; absent in a fact of a request that read nothing, and in a
    /// fact written before the member existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<crate::runtime::RequestArtifact>,
    pub opened_at: String,
    pub state: DecisionState,
    pub answer: Option<String>,
    pub answered_by: Option<String>,
    /// The daemon's principal at the answer; none in a fact written before the daemon had one.
    #[serde(default)]
    pub principal: Option<String>,
    pub answered_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunEvent {
    pub event_id: String,
    pub run_id: String,
    pub session_id: Option<String>,
    pub sequence: u64,
    pub position: u64,
    pub kind: String,
    pub record: Value,
    pub at: String,
}

/// The gateway's answer to a wake of a run that was closed (cancelled, ended) before the answer came: a line of the
/// daemon's own in the journal (`route.wake-answered`), because a closed run takes no event. `record` is the record the
/// run event `runbook.route-wake-answered` would have held: route, status, code.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WakeAnswer {
    pub run_id: String,
    pub record: Value,
    pub at: String,
}

/// The outcome of a run whose wait for its route is over (`EndRouteWait`).
pub const STOPPED_ON_ROUTE: &str = "stopped: route";

/// One change of the state, as the journal keeps it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "fact", rename_all = "snake_case")]
pub enum Fact {
    DaemonStarted(DaemonRow),
    SessionOpened { session_id: String, client: String, at: String },
    SessionClosed { session_id: String, at: String },
    RunSubmitted(Run),
    RunStarted { run_id: String, at: String },
    RunFailedToStart { run_id: String, reason: String, at: String },
    RunWaiting { run_id: String },
    RunWaitingForRoute { run_id: String, phase: Option<String> },
    /// The run waits behind the live runs of its serial key submitted before it.
    RunWaitingOnKey { run_id: String },
    /// No live run of the run's serial key was submitted before it any more: the run is Submitted again.
    RunTookKey { run_id: String },
    /// A delegation the run classified took the routes of the runbook the classifier picked: the run's record names them.
    RunRouted { run_id: String, model_route: crate::runtime::RunRoute },
    RunStoppedOnRoute { run_id: String, phase: Option<String>, at: String },
    RunResumed { run_id: String },
    RunEnded { run_id: String, outcome: String, at: String },
    RunCancelled { run_id: String, at: String },
    DecisionRequested(DecisionRequest),
    DecisionAnswered {
        request_id: String,
        answer: String,
        answered_by: String,
        #[serde(default)]
        principal: Option<String>,
        at: String,
    },
    DecisionWithdrawn { request_id: String },
    EventRecorded(RunEvent),
    /// The daemon got its stop signal and no longer takes a submit: the ids of the runs it waits for.
    #[serde(rename = "daemon.draining")]
    DaemonDraining { run_ids: Vec<String>, at: String },
    /// The last live run ended while the daemon drained.
    #[serde(rename = "daemon.drained")]
    DaemonDrained { at: String },
    /// The gateway answered a wake after the wake's run was closed.
    #[serde(rename = "route.wake-answered")]
    RouteWakeAnswered(WakeAnswer),
}

/// All of the state, for comparing two states.
#[derive(Serialize)]
pub struct Snapshot<'a> {
    pub daemon: &'a Option<DaemonRow>,
    pub sessions: &'a [Session],
    pub runs: &'a [Run],
    pub decisions: &'a [DecisionRequest],
    pub events: &'a [RunEvent],
    pub wake_answers: &'a [WakeAnswer],
}

pub struct State {
    root: PathBuf,
    journal: File,
    _lock: File,
    daemon: Option<DaemonRow>,
    /// Set by `daemon.draining`; like the configuration row it lives as long as the process.
    draining: bool,
    sessions: Vec<Session>,
    session_index: HashMap<String, usize>,
    runs: Vec<Run>,
    run_index: HashMap<String, usize>,
    keys: HashMap<String, usize>,
    decisions: Vec<DecisionRequest>,
    decision_index: HashMap<String, usize>,
    events: Vec<RunEvent>,
    events_of_run: HashMap<String, Vec<usize>>,
    /// The events `runbook.route-wake` and `runbook.route-wake-answered` of every run, by the route of their record, in
    /// position order: the booking of a route's wakes and the view `routes` walk these, never the whole record.
    wake_events_of_route: HashMap<String, Vec<usize>>,
    /// The answers to wakes of runs closed before them (`route.wake-answered`), in the order of the journal.
    wake_answers: Vec<WakeAnswer>,
    /// The runs this process took up (past the wait for their serial key) whose run thread has not
    /// returned yet: the runtime may still run one of them after it left the live states (a cancel
    /// moves the run at once). Not journaled: a new start has none.
    taken_up: HashSet<String>,
    /// A version that grows with every commit: what an event stream waits on.
    changes: tokio::sync::watch::Sender<u64>,
    clock: Arc<dyn Clock>,
}

/// A read of the events after a position: the events, and how far the read looked.
pub struct EventPage<'a> {
    pub events: Vec<&'a RunEvent>,
    /// Every event with a position up to this one has been looked at; at least the position asked after.
    pub scanned: u64,
}

fn invalid(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

impl State {
    /// Open the state directory: make `runs/` and `workspaces/`, take its lock, replay the journal.
    pub fn open(root: &Path) -> io::Result<State> {
        State::open_with_clock(root, Arc::new(SystemClock))
    }

    /// `open` with the clock the state reads every recorded instant from (a test sets one; the
    /// daemon's own start takes the system clock).
    pub fn open_with_clock(root: &Path, clock: Arc<dyn Clock>) -> io::Result<State> {
        std::fs::create_dir_all(root.join("runs"))?;
        std::fs::create_dir_all(root.join("workspaces"))?;
        let lock = OpenOptions::new().create(true).truncate(false).write(true).open(root.join("daemon.lock"))?;
        if lock.try_lock().is_err() {
            return Err(io::Error::other(format!("{} is in use by another daemon", root.display())));
        }
        let path = root.join("journal.jsonl");
        let journal = OpenOptions::new().create(true).read(true).append(true).open(&path)?;
        let mut state = State {
            root: root.to_path_buf(),
            journal,
            _lock: lock,
            daemon: None,
            draining: false,
            sessions: vec![],
            session_index: HashMap::new(),
            runs: vec![],
            run_index: HashMap::new(),
            keys: HashMap::new(),
            decisions: vec![],
            decision_index: HashMap::new(),
            events: vec![],
            events_of_run: HashMap::new(),
            wake_events_of_route: HashMap::new(),
            wake_answers: vec![],
            taken_up: HashSet::new(),
            changes: tokio::sync::watch::channel(0).0,
            clock,
        };
        state.replay(&path)?;
        Ok(state)
    }

    fn replay(&mut self, path: &Path) -> io::Result<()> {
        let mut bytes = Vec::new();
        File::open(path)?.read_to_end(&mut bytes)?;
        let whole = bytes.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
        if whole < bytes.len() {
            // A last line without its newline is a write that did not finish: drop it.
            self.journal.set_len(whole as u64)?;
        }
        for (i, line) in bytes[..whole].split(|b| *b == b'\n').filter(|l| !l.is_empty()).enumerate() {
            let fact: Fact = serde_json::from_slice(line).map_err(|e| invalid(format!("{}: line {}: {e}", path.display(), i + 1)))?;
            self.apply(&fact).map_err(|e| invalid(format!("{}: line {}: {e}", path.display(), i + 1)))?;
        }
        // The configuration row lives as long as the process: a new start writes its own.
        self.daemon = None;
        self.draining = false;
        Ok(())
    }

    /// The RFC 3339 instant now by the state's clock, in UTC, with milliseconds: the one place the
    /// daemon reads the time for a record.
    pub fn now(&self) -> String {
        format_instant(self.clock.now_millis())
    }

    /// Milliseconds since the epoch by the state's clock.
    pub fn now_millis(&self) -> i64 {
        self.clock.now_millis()
    }

    /// The newest event of the run.
    pub fn last_event_of(&self, run_id: &str) -> Option<&RunEvent> {
        self.events_of_run.get(run_id).and_then(|v| v.last()).map(|i| &self.events[*i])
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn run_dir(&self, run_id: &str) -> PathBuf {
        self.root.join("runs").join(run_id)
    }

    pub fn workspace_dir(&self, run_id: &str) -> PathBuf {
        self.root.join("workspaces").join(run_id)
    }

    /// Write the fact to the journal, then apply it.
    pub fn commit(&mut self, fact: Fact) -> io::Result<()> {
        self.append(&fact)?;
        self.applied(&fact)
    }

    /// Write the fact to the journal, ask `accept` while the state still lacks it and apply the fact
    /// when `accept` says yes. When it says no the line is taken out of the journal again and nothing
    /// changed. `accept` runs under the caller's hold of the lock; it is how the daemon hands an
    /// answer to a run only after the answer is on the journal, and takes it back when the run cannot
    /// take it. The answer is `false` when `accept` refused.
    pub fn commit_if(&mut self, fact: Fact, accept: impl FnOnce() -> bool) -> io::Result<bool> {
        let before = self.append(&fact)?;
        if !accept() {
            self.journal.set_len(before)?;
            return Ok(false);
        }
        self.applied(&fact)?;
        Ok(true)
    }

    /// The fact as one line at the end of the journal; the length the journal had before it. A line
    /// that was not written whole is taken out again.
    fn append(&mut self, fact: &Fact) -> io::Result<u64> {
        let mut line = serde_json::to_vec(fact).map_err(io::Error::other)?;
        line.push(b'\n');
        let before = self.journal.metadata()?.len();
        if let Err(e) = self.journal.write_all(&line) {
            let _ = self.journal.set_len(before);
            return Err(e);
        }
        Ok(before)
    }

    fn applied(&mut self, fact: &Fact) -> io::Result<()> {
        self.apply(fact).map_err(invalid)?;
        // After the change and inside the caller's hold of the lock: whoever sees the new version and
        // then reads the state finds the change.
        self.changes.send_modify(|version| *version += 1);
        Ok(())
    }

    /// A receiver whose value changes whenever the state does. Never blocks a writer.
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<u64> {
        self.changes.subscribe()
    }

    fn apply(&mut self, fact: &Fact) -> Result<(), String> {
        match fact {
            Fact::DaemonStarted(row) => self.daemon = Some(row.clone()),
            Fact::SessionOpened { session_id, client, at } => {
                self.session_index.insert(session_id.clone(), self.sessions.len());
                self.sessions.push(Session {
                    session_id: session_id.clone(),
                    client: client.clone(),
                    state: SessionState::Open,
                    opened_at: at.clone(),
                    closed_at: None,
                });
            }
            Fact::SessionClosed { session_id, at } => {
                let s = self.session_mut(session_id)?;
                s.state = SessionState::Closed;
                s.closed_at = Some(at.clone());
            }
            Fact::RunSubmitted(run) => {
                let at = self.runs.len();
                self.run_index.insert(run.run_id.clone(), at);
                if let Some(key) = &run.idempotency_key {
                    self.keys.entry(key.clone()).or_insert(at);
                }
                self.runs.push(run.clone());
            }
            Fact::RunStarted { run_id, at } => {
                let r = self.run_mut(run_id)?;
                r.state = RunState::Running;
                r.started_at = Some(at.clone());
            }
            Fact::RunFailedToStart { run_id, reason, at } => {
                let r = self.run_mut(run_id)?;
                r.state = RunState::FailedToStart;
                r.failure = Some(reason.clone());
                r.ended_at = Some(at.clone());
            }
            Fact::RunWaiting { run_id } => self.run_mut(run_id)?.state = RunState::WaitingOnDecision,
            Fact::RunWaitingForRoute { run_id, phase } => {
                let r = self.run_mut(run_id)?;
                r.state = RunState::WaitingForRoute;
                r.route_phase = phase.clone();
            }
            Fact::RunWaitingOnKey { run_id } => self.run_mut(run_id)?.state = RunState::WaitingOnKey,
            Fact::RunTookKey { run_id } => self.run_mut(run_id)?.state = RunState::Submitted,
            Fact::RunRouted { run_id, model_route } => self.run_mut(run_id)?.model_route = Some(model_route.clone()),
            Fact::RunStoppedOnRoute { run_id, phase, at } => {
                let r = self.run_mut(run_id)?;
                r.state = RunState::Ended;
                r.outcome = Some(STOPPED_ON_ROUTE.to_owned());
                r.route_phase = phase.clone();
                r.ended_at = Some(at.clone());
            }
            Fact::RunResumed { run_id } => self.run_mut(run_id)?.state = RunState::Running,
            Fact::RunEnded { run_id, outcome, at } => {
                let r = self.run_mut(run_id)?;
                r.state = RunState::Ended;
                r.outcome = Some(outcome.clone());
                r.ended_at = Some(at.clone());
            }
            Fact::RunCancelled { run_id, at } => {
                let r = self.run_mut(run_id)?;
                r.state = RunState::Cancelled;
                r.ended_at = Some(at.clone());
            }
            Fact::DecisionRequested(request) => {
                self.decision_index.insert(request.request_id.clone(), self.decisions.len());
                self.decisions.push(request.clone());
            }
            Fact::DecisionAnswered { request_id, answer, answered_by, principal, at } => {
                let d = self.decision_mut(request_id)?;
                d.state = DecisionState::Answered;
                d.answer = Some(answer.clone());
                d.answered_by = Some(answered_by.clone());
                d.principal = principal.clone();
                d.answered_at = Some(at.clone());
            }
            Fact::DecisionWithdrawn { request_id } => self.decision_mut(request_id)?.state = DecisionState::Withdrawn,
            Fact::EventRecorded(event) => {
                self.events_of_run.entry(event.run_id.clone()).or_default().push(self.events.len());
                let wake = event.kind == crate::commands::ROUTE_WAKE || event.kind == crate::commands::ROUTE_WAKE_ANSWERED;
                if let Some(route) = event.record["route"].as_str().filter(|_| wake) {
                    self.wake_events_of_route.entry(route.to_owned()).or_default().push(self.events.len());
                }
                self.events.push(event.clone());
            }
            Fact::DaemonDraining { .. } => self.draining = true,
            Fact::DaemonDrained { .. } => {}
            Fact::RouteWakeAnswered(answer) => self.wake_answers.push(answer.clone()),
        }
        Ok(())
    }

    fn session_mut(&mut self, id: &str) -> Result<&mut Session, String> {
        let i = *self.session_index.get(id).ok_or(format!("no session {id}"))?;
        Ok(&mut self.sessions[i])
    }

    fn run_mut(&mut self, id: &str) -> Result<&mut Run, String> {
        let i = *self.run_index.get(id).ok_or(format!("no run {id}"))?;
        Ok(&mut self.runs[i])
    }

    fn decision_mut(&mut self, id: &str) -> Result<&mut DecisionRequest, String> {
        let i = *self.decision_index.get(id).ok_or(format!("no decision request {id}"))?;
        Ok(&mut self.decisions[i])
    }

    pub fn snapshot(&self) -> Snapshot<'_> {
        Snapshot { daemon: &self.daemon, sessions: &self.sessions, runs: &self.runs, decisions: &self.decisions, events: &self.events, wake_answers: &self.wake_answers }
    }

    pub fn daemon(&self) -> Option<&DaemonRow> {
        self.daemon.as_ref()
    }

    /// Whether this process got its stop signal and refuses a submit.
    pub fn draining(&self) -> bool {
        self.draining
    }

    /// The ids of the runs that are not terminal, oldest first.
    pub fn live_run_ids(&self) -> Vec<String> {
        self.runs.iter().filter(|r| r.state.is_live()).map(|r| r.run_id.clone()).collect()
    }

    pub fn session(&self, id: &str) -> Option<&Session> {
        self.session_index.get(id).map(|i| &self.sessions[*i])
    }

    /// Oldest first.
    pub fn sessions(&self) -> &[Session] {
        &self.sessions
    }

    pub fn run(&self, id: &str) -> Option<&Run> {
        self.run_index.get(id).map(|i| &self.runs[*i])
    }

    /// Oldest first.
    pub fn runs(&self) -> &[Run] {
        &self.runs
    }

    /// The run a run with a serial key waits behind: the newest run with the same key that was submitted before it (the
    /// journal's order is the submit order) and holds the key: a live run, or one this process took up whose run thread
    /// has not returned (a cancelled run whose runtime has not let it go). None for a run with no key, and for the first
    /// such run of its key.
    pub fn ahead_on_key(&self, run_id: &str) -> Option<&Run> {
        let i = *self.run_index.get(run_id)?;
        let key = self.runs[i].serial_key.as_deref()?;
        self.runs[..i].iter().rev().find(|r| r.serial_key.as_deref() == Some(key) && (r.state.is_live() || self.taken_up.contains(&r.run_id)))
    }

    /// The run thread of `run_id` took the run up: it holds its serial key until [`State::let_go`].
    pub fn take_up(&mut self, run_id: &str) {
        self.taken_up.insert(run_id.to_owned());
    }

    /// The run thread of `run_id` returned: the runtime let the run go.
    pub fn let_go(&mut self, run_id: &str) {
        self.taken_up.remove(run_id);
    }

    /// Whether this process took the run up ([`State::take_up`]) and its run thread has not returned: also after a cancel
    /// moved the run out of the live states, while the thread still waits, for one, on the answer to the run's wake.
    pub fn is_taken_up(&self, run_id: &str) -> bool {
        self.taken_up.contains(run_id)
    }

    pub fn run_by_key(&self, key: &str) -> Option<&Run> {
        self.keys.get(key).map(|i| &self.runs[*i])
    }

    pub fn decision(&self, id: &str) -> Option<&DecisionRequest> {
        self.decision_index.get(id).map(|i| &self.decisions[*i])
    }

    pub fn decisions_of(&self, run_id: &str) -> Vec<&DecisionRequest> {
        self.decisions.iter().filter(|d| d.run_id == run_id).collect()
    }

    pub fn open_decisions_of(&self, run_id: &str) -> Vec<&DecisionRequest> {
        self.decisions.iter().filter(|d| d.run_id == run_id && d.state == DecisionState::Open).collect()
    }

    pub fn last_position(&self) -> u64 {
        self.events.last().map_or(0, |e| e.position)
    }

    /// The `sequence` the next event of this run takes: per run, from 1.
    pub fn next_sequence(&self, run_id: &str) -> u64 {
        self.events_of_run.get(run_id).and_then(|v| v.last()).map_or(1, |i| self.events[*i].sequence + 1)
    }

    /// The events of one run, in position order, through the index of the run's events: a walk of that run's events
    /// only, never of the whole record.
    pub fn events_of(&self, run_id: &str) -> impl Iterator<Item = &RunEvent> {
        self.events_of_run.get(run_id).into_iter().flatten().map(|i| &self.events[*i])
    }

    /// The events `runbook.route-wake` and `runbook.route-wake-answered` of `route`, of every run, in position order,
    /// through the index of the route's wake events: a walk of those events only, never of the whole record.
    pub fn wake_events_of(&self, route: &str) -> impl Iterator<Item = &RunEvent> {
        self.wake_events_of_route.get(route).into_iter().flatten().map(|i| &self.events[*i])
    }

    /// The answers to wakes of runs closed before them (`route.wake-answered`), in the order of the journal.
    pub fn wake_answers(&self) -> &[WakeAnswer] {
        &self.wake_answers
    }

    /// Events with a position above `after`, in position order; of one run, of one session, or of all.
    pub fn events_after(&self, run_id: Option<&str>, session_id: Option<&str>, after: u64) -> Vec<&RunEvent> {
        let from = self.events.partition_point(|e| e.position <= after);
        self.events[from..]
            .iter()
            .filter(|e| run_id.is_none_or(|r| e.run_id == r) && session_id.is_none_or(|s| e.session_id.as_deref() == Some(s)))
            .collect()
    }

    /// Like `events_after`, at most `limit` events of the filter; `scanned` is the position of the last
    /// event looked at (of the filter or not), so that a reader that continues from it does not look at
    /// the same events again.
    pub fn events_page(&self, run_id: Option<&str>, session_id: Option<&str>, after: u64, limit: usize) -> EventPage<'_> {
        let from = self.events.partition_point(|e| e.position <= after);
        let (mut events, mut scanned) = (Vec::new(), after);
        for e in &self.events[from..] {
            scanned = e.position;
            if run_id.is_none_or(|r| e.run_id == r) && session_id.is_none_or(|s| e.session_id.as_deref() == Some(s)) {
                events.push(e);
                if events.len() >= limit {
                    break;
                }
            }
        }
        EventPage { events, scanned }
    }
}
