//! The one command layer. HTTP, and later MCP and the CLI, call only this: one function per client
//! command and per view of `ess/domains/daemon.yaml`, and one per runtime-side command the runtime's
//! reports map to. Every outcome and refusal has the specification's name, and the guards run in
//! the specification's order.
//!
//! The daemon mints run ids (UUID version 7) and fills `daemon_id`, the time and the facts it reads
//! from the host (`runbook_known`, `context_inside_roots`); a client never chooses them.

use crate::runtime::{AnswerFailure, Confinement, DecisionRequest as RuntimeRequest, EventSink, Finish, Recovery, Runtime, StartRequest};
use crate::state::{self, DaemonRow, DecisionRequest, DecisionState, Fact, Run, RunEvent, RunState, SessionState, State};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;

/// The intake limit: 1 MiB, counted in bytes of UTF-8 (the specification's character count is the
/// weaker bound).
pub const MAX_INTAKE_BYTES: usize = 1_048_576;
/// The outcome of a run that ended because the runtime held no live run of it when the daemon started
/// again: the cause, in the `outcome` of the run (`EndOrphanedRun` sets this literal).
pub const ORPHANED: &str = "orphaned";
/// The longest client label and idempotency key, in characters.
pub const MAX_LABEL_CHARS: usize = 200;

/// What the daemon was started with.
#[derive(Clone, Debug)]
pub struct Config {
    pub state_dir: PathBuf,
    pub confinement_floor: Confinement,
    /// Directories a context path must lie under; none means no context from a path.
    pub context_roots: Vec<PathBuf>,
    pub runbook_dir: Option<PathBuf>,
}

/// A request the daemon cannot take at all (no outcome of the specification), or a failure of its own.
#[derive(Debug, PartialEq, Eq)]
pub enum Fault {
    Invalid(String),
    Internal(String),
}

impl From<io::Error> for Fault {
    fn from(e: io::Error) -> Fault {
        Fault::Internal(e.to_string())
    }
}

pub type Reply = Result<Outcome, Fault>;

/// The outcome of a command: the branch the specification declares, taken.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub command: &'static str,
    pub name: &'static str,
    pub status: u16,
    /// The specification's error, for a refusal.
    pub error: Option<&'static str>,
    pub payload: Option<Value>,
    /// Events the branch emitted: qualified name and payload.
    pub published: Vec<(&'static str, Value)>,
    /// The run a submit created or replayed.
    pub run_id: Option<String>,
}

impl Outcome {
    pub fn is_refusal(&self) -> bool {
        self.error.is_some()
    }

    /// The body of the response the OpenAPI document gives this outcome.
    pub fn body(&self) -> Value {
        let published: Vec<Value> = self.published.iter().map(|(e, p)| json!({"event": e, "payload": p})).collect();
        let mut m = Map::new();
        m.insert("outcome".into(), json!(self.name));
        m.insert("published".into(), Value::Array(published));
        if let Some(e) = self.error {
            m.insert("error".into(), json!(format!("sdc.daemon.{e}")));
            m.insert("payload".into(), self.payload.clone().unwrap_or(Value::Null));
        }
        Value::Object(m)
    }
}

/// One declared outcome of a command: its name, the HTTP status the OpenAPI document gives it and
/// the error it answers.
pub struct OutcomeSpec {
    pub command: &'static str,
    pub outcome: &'static str,
    pub status: u16,
    pub error: Option<&'static str>,
}

const fn o(command: &'static str, outcome: &'static str, status: u16, error: Option<&'static str>) -> OutcomeSpec {
    OutcomeSpec { command, outcome, status, error }
}

/// Every outcome of every command of the domain, by wire name. A test holds it equal to the domain
/// file and to the OpenAPI document.
pub const OUTCOMES: &[OutcomeSpec] = &[
    o("open-session", "opened", 202, None),
    o("open-session", "session-exists", 422, Some("SessionExists")),
    o("close-session", "no-session", 404, Some("NoSuchSession")),
    o("close-session", "closed", 202, None),
    o("close-session", "wrong-state", 409, Some("SessionStateConflict")),
    o("start-daemon", "started", 202, None),
    o("start-daemon", "already-started", 422, Some("DaemonAlreadyStarted")),
    o("submit-run", "intake-too-large", 422, Some("IntakeTooLarge")),
    o("submit-run", "no-runbook", 422, Some("NoSuchRunbook")),
    o("submit-run", "context-outside-roots", 422, Some("ContextOutsideRoots")),
    o("submit-run", "no-daemon", 409, Some("NoSuchDaemon")),
    o("submit-run", "below-floor", 409, Some("BelowConfinementFloor")),
    o("submit-run", "no-session", 409, Some("NoSuchSession")),
    o("submit-run", "session-closed", 409, Some("SessionClosedToRuns")),
    o("submit-run", "already-submitted", 202, None),
    o("submit-run", "submitted", 202, None),
    o("cancel-run", "no-run", 404, Some("NoSuchRun")),
    o("cancel-run", "cancelled", 202, None),
    o("cancel-run", "already-ended", 409, Some("RunAlreadyEnded")),
    o("start-run", "no-run", 404, Some("NoSuchRun")),
    o("start-run", "started", 202, None),
    o("start-run", "wrong-state", 409, Some("RunStateConflict")),
    o("fail-run-start", "no-run", 404, Some("NoSuchRun")),
    o("fail-run-start", "failed", 202, None),
    o("fail-run-start", "wrong-state", 409, Some("RunStateConflict")),
    o("wait-on-decision", "no-run", 404, Some("NoSuchRun")),
    o("wait-on-decision", "waiting", 202, None),
    o("wait-on-decision", "wrong-state", 409, Some("RunStateConflict")),
    o("resume-run", "no-run", 404, Some("NoSuchRun")),
    o("resume-run", "still-open", 409, Some("DecisionStillOpen")),
    o("resume-run", "resumed", 202, None),
    o("resume-run", "wrong-state", 409, Some("RunStateConflict")),
    o("request-decision", "no-run", 409, Some("NoSuchRun")),
    o("request-decision", "run-not-waiting", 409, Some("RunNotWaiting")),
    o("request-decision", "requested", 202, None),
    o("withdraw-decision", "no-request", 404, Some("NoSuchDecisionRequest")),
    o("withdraw-decision", "withdrawn", 202, None),
    o("withdraw-decision", "not-open", 409, Some("DecisionNotOpen")),
    o("answer-decision", "no-request", 404, Some("NoSuchDecisionRequest")),
    o("answer-decision", "not-of-run", 409, Some("DecisionNotOfRun")),
    o("answer-decision", "withdrawn", 409, Some("DecisionWithdrawn")),
    o("answer-decision", "no-run", 409, Some("NoSuchRun")),
    o("answer-decision", "run-not-waiting", 409, Some("RunNotWaiting")),
    o("answer-decision", "answered", 202, None),
    o("answer-decision", "already-answered", 409, Some("DecisionAlreadyAnswered")),
    o("record-run-event", "no-run", 409, Some("NoSuchRun")),
    o("record-run-event", "run-closed", 409, Some("RunClosedToEvents")),
    o("record-run-event", "not-ascending", 409, Some("SequenceNotAscending")),
    o("record-run-event", "position-not-ascending", 409, Some("PositionNotAscending")),
    o("record-run-event", "recorded", 202, None),
    o("end-run", "no-run", 404, Some("NoSuchRun")),
    o("end-run", "ended", 202, None),
    o("end-run", "wrong-state", 409, Some("RunStateConflict")),
    o("end-orphaned-run", "no-run", 404, Some("NoSuchRun")),
    o("end-orphaned-run", "orphaned", 202, None),
    o("end-orphaned-run", "wrong-state", 409, Some("RunStateConflict")),
];

fn spec(command: &str, outcome: &str) -> &'static OutcomeSpec {
    OUTCOMES.iter().find(|s| s.command == command && s.outcome == outcome).unwrap_or_else(|| panic!("no outcome {command}/{outcome}"))
}

fn event_name(short: &'static str) -> &'static str {
    match short {
        "SessionOpened" => "sdc.daemon.SessionOpened",
        "SessionClosed" => "sdc.daemon.SessionClosed",
        "DaemonStarted" => "sdc.daemon.DaemonStarted",
        "RunSubmitted" => "sdc.daemon.RunSubmitted",
        "SubmitReplayed" => "sdc.daemon.SubmitReplayed",
        "RunCancelled" => "sdc.daemon.RunCancelled",
        "RunStarted" => "sdc.daemon.RunStarted",
        "RunFailedToStart" => "sdc.daemon.RunFailedToStart",
        "RunWaiting" => "sdc.daemon.RunWaiting",
        "RunResumed" => "sdc.daemon.RunResumed",
        "DecisionRequested" => "sdc.daemon.DecisionRequested",
        "DecisionRequestWithdrawn" => "sdc.daemon.DecisionRequestWithdrawn",
        "DecisionAnswered" => "sdc.daemon.DecisionAnswered",
        "RunEventRecorded" => "sdc.daemon.RunEventRecorded",
        "RunEnded" => "sdc.daemon.RunEnded",
        "RunOrphaned" => "sdc.daemon.RunOrphaned",
        other => panic!("no event {other}"),
    }
}

fn done(command: &'static str, outcome: &'static str, events: Vec<(&'static str, Value)>) -> Outcome {
    let s = spec(command, outcome);
    Outcome {
        command,
        name: s.outcome,
        status: s.status,
        error: None,
        payload: None,
        published: events.into_iter().map(|(e, p)| (event_name(e), p)).collect(),
        run_id: None,
    }
}

fn refuse(command: &'static str, outcome: &'static str, payload: Value) -> Outcome {
    let s = spec(command, outcome);
    Outcome {
        command,
        name: s.outcome,
        status: s.status,
        error: s.error,
        payload: Some(payload),
        published: vec![],
        run_id: None,
    }
}

// ---- inputs of the client commands ----

#[derive(Deserialize, Clone, Debug)]
pub struct OpenSession {
    pub session_id: String,
    pub client: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct CloseSession {
    pub session_id: String,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct SubmitRun {
    pub runbook_id: String,
    #[serde(default)]
    pub runbook_version: Option<String>,
    pub intake: String,
    #[serde(default)]
    pub context_path: Option<String>,
    #[serde(default)]
    pub settings: BTreeMap<String, String>,
    /// Absent means `linux`.
    #[serde(default)]
    pub confinement: Option<Confinement>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct CancelRun {
    pub run_id: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct AnswerDecision {
    pub run_id: String,
    pub request_id: String,
    pub answer: String,
    pub answered_by: String,
}

// ---- the daemon ----

struct Inner {
    cfg: Config,
    daemon_id: String,
    state: Mutex<State>,
    runtime: Arc<dyn Runtime>,
    cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
    threads: Mutex<Vec<JoinHandle<()>>>,
    stopping: AtomicBool,
}

/// The daemon: its state, its runtime and the command layer over both. Cheap to clone.
#[derive(Clone)]
pub struct Daemon {
    inner: Arc<Inner>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn long_enough(text: &str) -> bool {
    text.chars().count() <= MAX_LABEL_CHARS
}

impl Daemon {
    /// Open the state directory, record the configuration row (`StartDaemon`) and deal with the runs
    /// whose runtime is gone because the daemon stopped.
    pub fn start(cfg: Config, runtime: Arc<dyn Runtime>) -> io::Result<Daemon> {
        let mut roots = Vec::new();
        for r in &cfg.context_roots {
            let canon = std::fs::canonicalize(r).map_err(|e| io::Error::new(e.kind(), format!("context root {}: {e}", r.display())))?;
            if !canon.is_dir() {
                return Err(io::Error::other(format!("context root {} is not a directory", r.display())));
            }
            roots.push(canon);
        }
        let cfg = Config { context_roots: roots, ..cfg };
        let state = State::open(&cfg.state_dir)?;
        let d = Daemon {
            inner: Arc::new(Inner {
                daemon_id: uuid::Uuid::now_v7().to_string(),
                cfg,
                state: Mutex::new(state),
                runtime,
                cancels: Mutex::new(HashMap::new()),
                threads: Mutex::new(Vec::new()),
                stopping: AtomicBool::new(false),
            }),
        };
        let floor = d.inner.cfg.confinement_floor;
        match d.start_daemon(&d.inner.daemon_id, floor) {
            Ok(o) if !o.is_refusal() => {}
            other => return Err(io::Error::other(format!("StartDaemon: {other:?}"))),
        }
        d.mark_orphans().map_err(|f| io::Error::other(format!("{f:?}")))?;
        Ok(d)
    }

    fn st(&self) -> MutexGuard<'_, State> {
        lock(&self.inner.state)
    }

    pub fn daemon_id(&self) -> &str {
        &self.inner.daemon_id
    }

    pub fn config(&self) -> &Config {
        &self.inner.cfg
    }

    /// Read the state under its lock (the event streams, `api::sse`); `f` must not wait.
    pub fn read<R>(&self, f: impl FnOnce(&State) -> R) -> R {
        f(&self.st())
    }

    pub fn is_stopping(&self) -> bool {
        self.inner.stopping.load(Ordering::SeqCst)
    }

    /// Stop the daemon: every run's cancel flag is set (this is not a client cancel: the runs keep the
    /// state they had), the runs' threads are joined, and the runtime reports no more.
    pub fn stop(&self) {
        self.inner.stopping.store(true, Ordering::SeqCst);
        for (run_id, flag) in lock(&self.inner.cancels).iter() {
            flag.store(true, Ordering::SeqCst);
            self.inner.runtime.cancel(run_id);
        }
        let handles: Vec<_> = lock(&self.inner.threads).drain(..).collect();
        for h in handles {
            let _ = h.join();
        }
    }

    /// The runs the record holds as live when the daemon starts. Submitted: the specification has
    /// `fail_to_start` from Submitted, so it is FailedToStart. Running and WaitingOnDecision: the
    /// daemon asks the runtime (`Runtime::recover`); `Gone` is `EndOrphanedRun` (the run ends with the
    /// outcome `orphaned` and its Open requests are withdrawn), `Resumed` leaves the run as it is. A
    /// run that has ended is not asked about, so a second start changes nothing for it.
    fn mark_orphans(&self) -> Result<(), Fault> {
        let live: Vec<(String, RunState)> = self.st().runs().iter().filter(|r| r.state.is_live()).map(|r| (r.run_id.clone(), r.state)).collect();
        for (run_id, state) in live {
            if state == RunState::Submitted {
                fail_run_start_locked(&mut self.st(), &run_id, "the daemon stopped before the runtime took the run up")?;
            } else if self.inner.runtime.recover(&run_id) == Recovery::Gone {
                self.end_orphaned_run(&run_id)?;
            }
        }
        Ok(())
    }

    /// `EndOrphanedRun`, issued by the daemon itself (never by a client): the run ends with the outcome
    /// `orphaned`, then `WithdrawDecision` is issued for each of its Open requests, as a cancel does.
    pub fn end_orphaned_run(&self, run_id: &str) -> Reply {
        let mut st = self.st();
        let out = end_orphaned_run_locked(&mut st, run_id)?;
        if !out.is_refusal() {
            let open: Vec<String> = st.open_decisions_of(run_id).iter().map(|d| d.request_id.clone()).collect();
            for request_id in open {
                withdraw_decision_locked(&mut st, &request_id)?;
            }
        }
        Ok(out)
    }

    // ---- client commands ----

    pub fn open_session(&self, input: OpenSession) -> Reply {
        if !long_enough(&input.client) {
            return Err(Fault::Invalid(format!("client is longer than {MAX_LABEL_CHARS} characters")));
        }
        let mut st = self.st();
        if st.session(&input.session_id).is_some() {
            return Ok(refuse("open-session", "session-exists", json!({"session_id": input.session_id})));
        }
        st.commit(Fact::SessionOpened { session_id: input.session_id.clone(), client: input.client.clone(), at: state::now() })?;
        Ok(done("open-session", "opened", vec![("SessionOpened", json!({"session_id": input.session_id, "client": input.client}))]))
    }

    pub fn close_session(&self, input: CloseSession) -> Reply {
        let mut st = self.st();
        let id = input.session_id;
        match st.session(&id).map(|s| s.state) {
            None => Ok(refuse("close-session", "no-session", json!({"session_id": id}))),
            Some(SessionState::Closed) => Ok(refuse("close-session", "wrong-state", json!({"session_id": id}))),
            Some(SessionState::Open) => {
                st.commit(Fact::SessionClosed { session_id: id.clone(), at: state::now() })?;
                Ok(done("close-session", "closed", vec![("SessionClosed", json!({"session_id": id}))]))
            }
        }
    }

    /// `StartDaemon`, issued once by the operator's process before it takes a request.
    pub fn start_daemon(&self, daemon_id: &str, confinement_floor: Confinement) -> Reply {
        let mut st = self.st();
        if st.daemon().is_some() {
            return Ok(refuse("start-daemon", "already-started", json!({"daemon_id": daemon_id})));
        }
        st.commit(Fact::DaemonStarted(DaemonRow { daemon_id: daemon_id.to_string(), confinement_floor, started_at: state::now() }))?;
        Ok(done("start-daemon", "started", vec![("DaemonStarted", json!({"daemon_id": daemon_id, "confinement_floor": confinement_floor}))]))
    }

    pub fn submit_run(&self, input: SubmitRun) -> Reply {
        let id = self.inner.daemon_id.clone();
        self.submit_run_for(&id, input)
    }

    /// `SubmitRun` as the daemon `daemon_id` names (the HTTP handler always names its own).
    pub fn submit_run_for(&self, daemon_id: &str, input: SubmitRun) -> Reply {
        if input.idempotency_key.as_deref().is_some_and(|k| !long_enough(k)) {
            return Err(Fault::Invalid(format!("idempotency_key is longer than {MAX_LABEL_CHARS} characters")));
        }
        let run_id = uuid::Uuid::now_v7().to_string();
        let confinement = input.confinement.unwrap_or(Confinement::Linux);
        let refused = |name: &'static str, payload: Value| -> Reply { Ok(refuse("submit-run", name, payload)) };
        // The two facts the specification takes from the host.
        let runbook_known = self.runbook_known(&input.runbook_id, input.runbook_version.as_deref());
        let context = input.context_path.as_deref().map(|p| self.context_under_roots(p));

        if input.intake.len() > MAX_INTAKE_BYTES {
            return refused("intake-too-large", json!({"run_id": run_id}));
        }
        if !runbook_known {
            return refused("no-runbook", json!({"run_id": run_id}));
        }
        if matches!(context, Some(None)) {
            return refused("context-outside-roots", json!({"run_id": run_id}));
        }
        let context_dir = context.flatten();

        let mut st = self.st();
        let floor = match st.daemon() {
            Some(d) if d.daemon_id == daemon_id => d.confinement_floor,
            _ => return refused("no-daemon", json!({"daemon_id": daemon_id})),
        };
        if confinement.rank() < floor.rank() {
            return refused("below-floor", json!({"run_id": run_id, "confinement": confinement}));
        }
        if let Some(sid) = &input.session_id {
            match st.session(sid).map(|s| s.state) {
                None => return refused("no-session", json!({"session_id": sid})),
                Some(SessionState::Closed) => return refused("session-closed", json!({"run_id": run_id, "session_id": sid})),
                Some(SessionState::Open) => {}
            }
        }
        if let Some(key) = &input.idempotency_key {
            if let Some(first) = st.run_by_key(key) {
                // The answer names the first run twice: as data of the event it publishes (the body)
                // and as the run the HTTP binding puts in the Location header.
                let first = first.run_id.clone();
                let mut out = done("submit-run", "already-submitted", vec![("SubmitReplayed", json!({"idempotency_key": key, "run_id": first}))]);
                out.run_id = Some(first);
                return Ok(out);
            }
        }

        let (run_dir, workspace) = (st.run_dir(&run_id), st.workspace_dir(&run_id));
        std::fs::create_dir_all(&run_dir)?;
        std::fs::create_dir_all(&workspace)?;
        let run = Run {
            run_id: run_id.clone(),
            runbook_id: input.runbook_id.clone(),
            runbook_version: input.runbook_version.clone(),
            intake: input.intake.clone(),
            context_path: input.context_path.clone(),
            settings: input.settings.clone(),
            confinement,
            session_id: input.session_id.clone(),
            idempotency_key: input.idempotency_key.clone(),
            submitted_at: state::now(),
            started_at: None,
            ended_at: None,
            outcome: None,
            failure: None,
            state: RunState::Submitted,
        };
        st.commit(Fact::RunSubmitted(run))?;
        let payload = json!({"run_id": run_id, "runbook_id": input.runbook_id, "confinement": confinement});
        record_event_locked(&mut st, &run_id, "runbook.submitted", payload.clone())?;
        drop(st);

        let request = StartRequest {
            run_id: run_id.clone(),
            runbook: input.runbook_id,
            version: input.runbook_version,
            intake: input.intake,
            context_dir,
            settings: input.settings,
            confinement,
            state_dir: run_dir,
            workspace_dir: workspace,
        };
        self.spawn_run(request);
        let mut out = done("submit-run", "submitted", vec![("RunSubmitted", payload)]);
        out.run_id = Some(run_id);
        Ok(out)
    }

    pub fn cancel_run(&self, input: CancelRun) -> Reply {
        let id = input.run_id;
        let mut st = self.st();
        match st.run(&id).map(|r| r.state) {
            None => return Ok(refuse("cancel-run", "no-run", json!({"run_id": id}))),
            Some(s) if !s.is_live() => return Ok(refuse("cancel-run", "already-ended", json!({"run_id": id}))),
            Some(_) => {}
        }
        record_event_locked(&mut st, &id, "runbook.cancel-requested", json!({"run_id": id}))?;
        st.commit(Fact::RunCancelled { run_id: id.clone(), at: state::now() })?;
        let open: Vec<String> = st.open_decisions_of(&id).iter().map(|d| d.request_id.clone()).collect();
        for request_id in open {
            withdraw_decision_locked(&mut st, &request_id)?;
        }
        drop(st);
        if let Some(flag) = lock(&self.inner.cancels).get(&id) {
            flag.store(true, Ordering::SeqCst);
        }
        self.inner.runtime.cancel(&id);
        Ok(done("cancel-run", "cancelled", vec![("RunCancelled", json!({"run_id": id}))]))
    }

    pub fn answer_decision(&self, input: AnswerDecision) -> Reply {
        let mut st = self.st();
        let (run_id, request_id) = (input.run_id.clone(), input.request_id.clone());
        let Some(request) = st.decision(&request_id) else {
            return Ok(refuse("answer-decision", "no-request", json!({"request_id": request_id})));
        };
        if request.run_id != run_id {
            return Ok(refuse("answer-decision", "not-of-run", json!({"request_id": request_id, "run_id": run_id})));
        }
        // The request's state first, then the run's (decision 16): a second answer names the request
        // whatever its run is doing.
        match request.state {
            DecisionState::Withdrawn => return Ok(refuse("answer-decision", "withdrawn", json!({"request_id": request_id}))),
            DecisionState::Answered => return Ok(refuse("answer-decision", "already-answered", json!({"request_id": request_id}))),
            DecisionState::Open => {}
        }
        match st.run(&run_id).map(|r| r.state) {
            None => return Ok(refuse("answer-decision", "no-run", json!({"run_id": run_id}))),
            Some(s) if s != RunState::WaitingOnDecision => return Ok(refuse("answer-decision", "run-not-waiting", json!({"run_id": run_id}))),
            Some(_) => {}
        }
        st.commit(Fact::DecisionAnswered {
            request_id: request_id.clone(),
            answer: input.answer.clone(),
            answered_by: input.answered_by.clone(),
            at: state::now(),
        })?;
        drop(st);
        let published = json!({"request_id": request_id, "run_id": run_id, "answered_by": input.answered_by});
        match self.inner.runtime.answer(&run_id, &request_id, &input.answer, &input.answered_by) {
            Ok(()) => {}
            Err(AnswerFailure::UnknownRun) | Err(AnswerFailure::Refused(_)) => {
                // The answer stands; the runtime cannot take it (its run is gone). Say so in the run's record.
                let mut st = self.st();
                let _ = record_event_locked(&mut st, &run_id, "runbook.answer-undeliverable", json!({"request_id": request_id}));
            }
        }
        Ok(done("answer-decision", "answered", vec![("DecisionAnswered", published)]))
    }

    // ---- runtime-side commands: what the runtime's reports map to ----

    pub fn start_run(&self, run_id: &str) -> Reply {
        let mut st = self.st();
        match st.run(run_id).map(|r| r.state) {
            None => Ok(refuse("start-run", "no-run", json!({"run_id": run_id}))),
            Some(RunState::Submitted) => {
                st.commit(Fact::RunStarted { run_id: run_id.to_string(), at: state::now() })?;
                Ok(done("start-run", "started", vec![("RunStarted", json!({"run_id": run_id}))]))
            }
            Some(_) => Ok(refuse("start-run", "wrong-state", json!({"run_id": run_id}))),
        }
    }

    pub fn fail_run_start(&self, run_id: &str, reason: &str) -> Reply {
        fail_run_start_locked(&mut self.st(), run_id, reason)
    }

    pub fn wait_on_decision(&self, run_id: &str) -> Reply {
        wait_on_decision_locked(&mut self.st(), run_id)
    }

    pub fn request_decision(&self, run_id: &str, request: RuntimeRequest) -> Reply {
        request_decision_locked(&mut self.st(), run_id, request)
    }

    pub fn withdraw_decision(&self, request_id: &str) -> Reply {
        withdraw_decision_locked(&mut self.st(), request_id)
    }

    pub fn resume_run(&self, run_id: &str) -> Reply {
        let mut st = self.st();
        let Some(state) = st.run(run_id).map(|r| r.state) else {
            return Ok(refuse("resume-run", "no-run", json!({"run_id": run_id})));
        };
        if !st.open_decisions_of(run_id).is_empty() {
            return Ok(refuse("resume-run", "still-open", json!({"run_id": run_id})));
        }
        if state != RunState::WaitingOnDecision {
            return Ok(refuse("resume-run", "wrong-state", json!({"run_id": run_id})));
        }
        st.commit(Fact::RunResumed { run_id: run_id.to_string() })?;
        Ok(done("resume-run", "resumed", vec![("RunResumed", json!({"run_id": run_id}))]))
    }

    /// `RecordRunEvent`: the daemon mints the next `sequence` of the run and the next `position`.
    pub fn record_run_event(&self, run_id: &str, kind: &str, record: Value) -> Reply {
        record_event_locked_outcome(&mut self.st(), run_id, kind, record)
    }

    /// `RecordRunEvent` with the numbers given: refuses a sequence or a position that does not grow.
    pub fn record_run_event_numbered(&self, run_id: &str, sequence: u64, position: u64, kind: &str, record: Value) -> Reply {
        record_numbered_locked(&mut self.st(), run_id, sequence, position, kind, record)
    }

    pub fn end_run(&self, run_id: &str, outcome: &str) -> Reply {
        let mut st = self.st();
        match st.run(run_id).map(|r| r.state) {
            None => Ok(refuse("end-run", "no-run", json!({"run_id": run_id}))),
            Some(RunState::Running) => {
                st.commit(Fact::RunEnded { run_id: run_id.to_string(), outcome: outcome.to_string(), at: state::now() })?;
                Ok(done("end-run", "ended", vec![("RunEnded", json!({"run_id": run_id, "outcome": outcome}))]))
            }
            Some(_) => Ok(refuse("end-run", "wrong-state", json!({"run_id": run_id}))),
        }
    }

    // ---- views ----

    pub fn runs(&self) -> Vec<Value> {
        run_rows(self.st().runs().iter().rev())
    }

    pub fn runs_of_session(&self, session_id: &str) -> Vec<Value> {
        run_rows(self.st().runs().iter().rev().filter(|r| r.session_id.as_deref() == Some(session_id)))
    }

    pub fn runs_in_state(&self, state: &str) -> Result<Vec<Value>, Fault> {
        let state = parse_state(state)?;
        Ok(run_rows(self.st().runs().iter().rev().filter(|r| r.state == state)))
    }

    pub fn runs_of_session_in_state(&self, session_id: &str, state: &str) -> Result<Vec<Value>, Fault> {
        let state = parse_state(state)?;
        Ok(run_rows(self.st().runs().iter().rev().filter(|r| r.session_id.as_deref() == Some(session_id) && r.state == state)))
    }

    /// The view `RunDetail` (wire name `run`): zero or one row.
    pub fn run_detail(&self, run_id: &str) -> Vec<Value> {
        self.st()
            .run(run_id)
            .map(|r| {
                let mut row = run_row(r);
                let m = row.as_object_mut().unwrap();
                m.insert("intake".into(), json!(r.intake));
                m.insert("settings".into(), json!(r.settings));
                if let Some(c) = &r.context_path {
                    m.insert("context_path".into(), json!(c));
                }
                row
            })
            .into_iter()
            .collect()
    }

    pub fn open_decisions_of_run(&self, run_id: &str) -> Vec<Value> {
        self.st()
            .open_decisions_of(run_id)
            .into_iter()
            .map(|d| json!({"request_id": d.request_id, "run_id": d.run_id, "question": d.question, "options": d.options, "opened_at": d.opened_at}))
            .collect()
    }

    pub fn decisions_of_run(&self, run_id: &str) -> Vec<Value> {
        self.st()
            .decisions_of(run_id)
            .into_iter()
            .map(|d| {
                let mut m = Map::new();
                m.insert("request_id".into(), json!(d.request_id));
                m.insert("run_id".into(), json!(d.run_id));
                m.insert("state".into(), json!(d.state));
                m.insert("question".into(), json!(d.question));
                m.insert("options".into(), json!(d.options));
                m.insert("opened_at".into(), json!(d.opened_at));
                put(&mut m, "answer", &d.answer);
                put(&mut m, "answered_by", &d.answered_by);
                put(&mut m, "answered_at", &d.answered_at);
                Value::Object(m)
            })
            .collect()
    }

    /// The view `EventsAfter` (wire name `events`).
    pub fn events_after(&self, run_id: &str, after_position: u64) -> Vec<Value> {
        event_rows(self.st().events_after(Some(run_id), None, after_position))
    }

    /// The view `EventsOfSessionAfter` (wire name `session-events`).
    pub fn events_of_session_after(&self, session_id: &str, after_position: u64) -> Vec<Value> {
        event_rows(self.st().events_after(None, Some(session_id), after_position))
    }

    /// The view `EventsOfDaemonAfter` (wire name `daemon-events`).
    pub fn events_of_daemon_after(&self, after_position: u64) -> Vec<Value> {
        event_rows(self.st().events_after(None, None, after_position))
    }

    pub fn sessions(&self) -> Vec<Value> {
        self.st()
            .sessions()
            .iter()
            .map(|s| {
                let mut m = Map::new();
                m.insert("session_id".into(), json!(s.session_id));
                m.insert("client".into(), json!(s.client));
                m.insert("state".into(), json!(s.state));
                m.insert("opened_at".into(), json!(s.opened_at));
                put(&mut m, "closed_at", &s.closed_at);
                Value::Object(m)
            })
            .collect()
    }

    /// The view `DaemonConfiguration` (wire name `configuration`).
    pub fn daemon_configuration(&self) -> Vec<Value> {
        self.st()
            .daemon()
            .map(|d| json!({"daemon_id": d.daemon_id, "confinement_floor": d.confinement_floor, "started_at": d.started_at}))
            .into_iter()
            .collect()
    }

    // ---- the host's facts ----

    fn runbook_known(&self, name: &str, version: Option<&str>) -> bool {
        if self.inner.runtime.knows_runbook(name, version) {
            return true;
        }
        let Some(dir) = &self.inner.cfg.runbook_dir else { return false };
        let plain = !name.is_empty() && !name.starts_with('.') && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
        plain && version.is_none() && (dir.join(name).is_dir() || dir.join(format!("{name}.protocol.yaml")).is_file())
    }

    /// The canonical form of a context path when it is an existing directory under a context root.
    /// Symbolic links and `..` are resolved first, so neither leads out of a root.
    fn context_under_roots(&self, path: &str) -> Option<PathBuf> {
        let p = Path::new(path);
        if !p.is_absolute() {
            return None;
        }
        let canon = std::fs::canonicalize(p).ok()?;
        (canon.is_dir() && self.inner.cfg.context_roots.iter().any(|r| canon.starts_with(r))).then_some(canon)
    }

    // ---- the run's thread ----

    fn spawn_run(&self, request: StartRequest) {
        let flag = Arc::new(AtomicBool::new(false));
        lock(&self.inner.cancels).insert(request.run_id.clone(), flag.clone());
        let d = self.clone();
        let name = format!("run-{}", request.run_id);
        let handle = std::thread::Builder::new().name(name).spawn(move || d.run_thread(request, &flag)).expect("spawn a thread for the run");
        let mut threads = lock(&self.inner.threads);
        threads.retain(|h| !h.is_finished());
        threads.push(handle);
    }

    fn run_thread(&self, request: StartRequest, cancel: &AtomicBool) {
        let run_id = request.run_id.clone();
        if let (Some(from), false) = (&request.context_dir, cancel.load(Ordering::SeqCst)) {
            if let Err(e) = copy_dir(from, &request.workspace_dir) {
                if !self.is_stopping() {
                    let _ = self.fail_run_start(&run_id, &format!("the context could not be copied into the workspace: {e}"));
                }
                lock(&self.inner.cancels).remove(&run_id);
                return;
            }
        }
        let sink = RunSink { daemon: self.clone(), run_id: run_id.clone(), started: AtomicBool::new(false) };
        let result = if cancel.load(Ordering::SeqCst) { Ok(Finish::Cancelled) } else { self.inner.runtime.start(request, &sink, cancel) };
        if !self.is_stopping() {
            // A refusal below means the run is Cancelled already (a report after a cancel).
            match result {
                Ok(Finish::Outcome(outcome)) => {
                    let _ = self.end_run(&run_id, &outcome);
                }
                // CancelRun moved the run already.
                Ok(Finish::Cancelled) => {}
                Err(f) if sink.started.load(Ordering::SeqCst) => {
                    let _ = self.record_run_event(&run_id, "runbook.runtime-failed", json!({"reason": f.reason}));
                    let _ = self.end_run(&run_id, "runtime-failure");
                }
                Err(f) => {
                    let _ = self.fail_run_start(&run_id, &f.reason);
                }
            }
        }
        lock(&self.inner.cancels).remove(&run_id);
    }
}

/// What the runtime reports, mapped to the runtime-side commands. A refusal is not an error here:
/// a report that comes after a cancel finds the run Cancelled and changes nothing.
struct RunSink {
    daemon: Daemon,
    run_id: String,
    started: AtomicBool,
}

impl EventSink for RunSink {
    fn started(&self) {
        if self.daemon.is_stopping() {
            return;
        }
        self.started.store(true, Ordering::SeqCst);
        let _ = self.daemon.start_run(&self.run_id);
    }

    fn record(&self, kind: &str, record: Value) {
        if !self.daemon.is_stopping() {
            let _ = self.daemon.record_run_event(&self.run_id, kind, record);
        }
    }

    fn decision_requested(&self, request: RuntimeRequest) {
        if self.daemon.is_stopping() {
            return;
        }
        let mut st = self.daemon.st();
        if st.run(&self.run_id).map(|r| r.state) == Some(RunState::Running) {
            let _ = wait_on_decision_locked(&mut st, &self.run_id);
        }
        let _ = request_decision_locked(&mut st, &self.run_id, request);
    }

    fn resumed(&self) {
        if !self.daemon.is_stopping() {
            let _ = self.daemon.resume_run(&self.run_id);
        }
    }
}

// ---- commands that run with the state already locked ----

fn fail_run_start_locked(st: &mut State, run_id: &str, reason: &str) -> Reply {
    match st.run(run_id).map(|r| r.state) {
        None => Ok(refuse("fail-run-start", "no-run", json!({"run_id": run_id}))),
        Some(RunState::Submitted) => {
            record_event_locked(st, run_id, "runbook.start-failed", json!({"reason": reason}))?;
            st.commit(Fact::RunFailedToStart { run_id: run_id.to_string(), reason: reason.to_string(), at: state::now() })?;
            Ok(done("fail-run-start", "failed", vec![("RunFailedToStart", json!({"run_id": run_id, "reason": reason}))]))
        }
        Some(_) => Ok(refuse("fail-run-start", "wrong-state", json!({"run_id": run_id}))),
    }
}

fn end_orphaned_run_locked(st: &mut State, run_id: &str) -> Reply {
    match st.run(run_id).map(|r| r.state) {
        None => Ok(refuse("end-orphaned-run", "no-run", json!({"run_id": run_id}))),
        Some(state @ (RunState::Running | RunState::WaitingOnDecision)) => {
            // The run's stream says so while the run is still live (a closed run takes no event).
            let record = json!({"run_id": run_id, "state": state.as_str(), "reason": "the daemon started again and the runtime holds no live run of this id"});
            record_event_locked(st, run_id, "runbook.run-orphaned", record)?;
            st.commit(Fact::RunEnded { run_id: run_id.to_string(), outcome: ORPHANED.to_string(), at: state::now() })?;
            Ok(done("end-orphaned-run", "orphaned", vec![("RunOrphaned", json!({"run_id": run_id}))]))
        }
        Some(_) => Ok(refuse("end-orphaned-run", "wrong-state", json!({"run_id": run_id}))),
    }
}

fn wait_on_decision_locked(st: &mut State, run_id: &str) -> Reply {
    match st.run(run_id).map(|r| r.state) {
        None => Ok(refuse("wait-on-decision", "no-run", json!({"run_id": run_id}))),
        Some(RunState::Running) => {
            st.commit(Fact::RunWaiting { run_id: run_id.to_string() })?;
            Ok(done("wait-on-decision", "waiting", vec![("RunWaiting", json!({"run_id": run_id}))]))
        }
        Some(_) => Ok(refuse("wait-on-decision", "wrong-state", json!({"run_id": run_id}))),
    }
}

fn request_decision_locked(st: &mut State, run_id: &str, request: RuntimeRequest) -> Reply {
    match st.run(run_id).map(|r| r.state) {
        None => return Ok(refuse("request-decision", "no-run", json!({"run_id": run_id}))),
        Some(s) if s != RunState::WaitingOnDecision => return Ok(refuse("request-decision", "run-not-waiting", json!({"run_id": run_id}))),
        Some(_) => {}
    }
    if st.decision(&request.request_id).is_some() {
        return Err(Fault::Invalid(format!("decision request {} exists already: the runtime mints unique ids", request.request_id)));
    }
    let id = request.request_id.clone();
    st.commit(Fact::DecisionRequested(DecisionRequest {
        request_id: request.request_id,
        run_id: run_id.to_string(),
        case_id: request.case_id,
        action: request.action,
        capability: request.capability,
        question: request.question,
        options: request.options,
        opened_at: state::now(),
        state: DecisionState::Open,
        answer: None,
        answered_by: None,
        answered_at: None,
    }))?;
    Ok(done("request-decision", "requested", vec![("DecisionRequested", json!({"request_id": id, "run_id": run_id}))]))
}

fn withdraw_decision_locked(st: &mut State, request_id: &str) -> Reply {
    match st.decision(request_id).map(|d| d.state) {
        None => Ok(refuse("withdraw-decision", "no-request", json!({"request_id": request_id}))),
        Some(DecisionState::Open) => {
            st.commit(Fact::DecisionWithdrawn { request_id: request_id.to_string() })?;
            Ok(done("withdraw-decision", "withdrawn", vec![("DecisionRequestWithdrawn", json!({"request_id": request_id}))]))
        }
        Some(_) => Ok(refuse("withdraw-decision", "not-open", json!({"request_id": request_id}))),
    }
}

fn record_event_locked_outcome(st: &mut State, run_id: &str, kind: &str, record: Value) -> Reply {
    let (sequence, position) = (st.next_sequence(run_id), st.last_position() + 1);
    record_numbered_locked(st, run_id, sequence, position, kind, record)
}

/// Record an event of the daemon's own or of the runtime with the next numbers; an error only when
/// the state could not be written.
fn record_event_locked(st: &mut State, run_id: &str, kind: &str, record: Value) -> Result<(), Fault> {
    record_event_locked_outcome(st, run_id, kind, record).map(|_| ())
}

fn record_numbered_locked(st: &mut State, run_id: &str, sequence: u64, position: u64, kind: &str, record: Value) -> Reply {
    let session_id = match st.run(run_id) {
        None => return Ok(refuse("record-run-event", "no-run", json!({"run_id": run_id}))),
        Some(r) if !r.state.is_live() => return Ok(refuse("record-run-event", "run-closed", json!({"run_id": run_id}))),
        Some(r) => r.session_id.clone(),
    };
    if st.next_sequence(run_id) > sequence {
        return Ok(refuse("record-run-event", "not-ascending", json!({"run_id": run_id, "sequence": sequence})));
    }
    if st.last_position() >= position {
        return Ok(refuse("record-run-event", "position-not-ascending", json!({"run_id": run_id, "position": position})));
    }
    let event_id = format!("event-{position}");
    st.commit(Fact::EventRecorded(RunEvent {
        event_id: event_id.clone(),
        run_id: run_id.to_string(),
        session_id,
        sequence,
        position,
        kind: kind.to_string(),
        record,
        at: state::now(),
    }))?;
    Ok(done(
        "record-run-event",
        "recorded",
        vec![("RunEventRecorded", json!({"event_id": event_id, "run_id": run_id, "sequence": sequence, "position": position}))],
    ))
}

// ---- rows ----

fn put(m: &mut Map<String, Value>, key: &str, value: &Option<String>) {
    if let Some(v) = value {
        m.insert(key.to_string(), json!(v));
    }
}

fn parse_state(text: &str) -> Result<RunState, Fault> {
    RunState::parse(text).ok_or_else(|| Fault::Invalid(format!("state {text:?} is not one of {}", RunState::ALL.map(RunState::as_str).join(", "))))
}

fn run_row(r: &Run) -> Value {
    let mut m = Map::new();
    m.insert("run_id".into(), json!(r.run_id));
    m.insert("runbook_id".into(), json!(r.runbook_id));
    put(&mut m, "runbook_version", &r.runbook_version);
    m.insert("state".into(), json!(r.state));
    m.insert("confinement".into(), json!(r.confinement));
    put(&mut m, "session_id", &r.session_id);
    m.insert("submitted_at".into(), json!(r.submitted_at));
    put(&mut m, "started_at", &r.started_at);
    put(&mut m, "ended_at", &r.ended_at);
    put(&mut m, "outcome", &r.outcome);
    put(&mut m, "failure", &r.failure);
    Value::Object(m)
}

fn run_rows<'a>(runs: impl Iterator<Item = &'a Run>) -> Vec<Value> {
    runs.map(run_row).collect()
}

pub(crate) fn event_rows(events: Vec<&RunEvent>) -> Vec<Value> {
    events
        .into_iter()
        .map(|e| {
            let mut m = Map::new();
            m.insert("event_id".into(), json!(e.event_id));
            m.insert("run_id".into(), json!(e.run_id));
            put(&mut m, "session_id", &e.session_id);
            m.insert("sequence".into(), json!(e.sequence));
            m.insert("position".into(), json!(e.position));
            m.insert("kind".into(), json!(e.kind));
            m.insert("record".into(), e.record.clone());
            m.insert("at".into(), json!(e.at));
            Value::Object(m)
        })
        .collect()
}

/// Copy a directory tree into the workspace: directories and regular files only; a symbolic link is
/// skipped, so a link inside a context root cannot carry a file from outside it into a run.
fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
