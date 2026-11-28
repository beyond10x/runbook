//! The one command layer. HTTP, and later MCP and the CLI, call only this: one function per client
//! command and per view of `ess/domains/daemon.yaml`, and one per runtime-side command the runtime's
//! reports map to. Every outcome and refusal has the specification's name, and the guards run in
//! the specification's order.
//!
//! The daemon mints run ids (UUID version 7) and fills `daemon_id`, the time and the facts it reads
//! from the host (`runbook_known`, `context_inside_roots`); a client never chooses them.

use crate::clock::{parse_instant, Clock, SystemClock};
use crate::runtime::{AnswerFailure, Autonomy, Confinement, ConfinementSource, DecisionRequest as RuntimeRequest, EventSink, Finish, ProgramSteps, Recovery, RouteSource, RunRoute, Runtime, StartRequest, StartTarget, Started, Terms};
use crate::state::{DaemonRow, DecisionRequest, DecisionState, Fact, Run, RunEvent, RunState, SessionState, State};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::io;
use std::os::unix::fs::DirBuilderExt;
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
/// The run event that says a run is ended `orphaned`, written just before its end.
const RUN_ORPHANED: &str = "runbook.run-orphaned";
/// The reason of `runbook.run-orphaned` when the runtime holds no run of the id at the next start.
pub const ORPHANED_AT_START: &str = "the daemon started again and the runtime holds no live run of this id";
/// The reason of `runbook.run-orphaned` when the daemon ends while the run lives (a second stop signal,
/// or the drain bound): the run does not wait for the next start to be orphaned.
pub const ORPHANED_AT_STOP: &str = "the daemon was stopped while the run was live";
/// The reason of `runbook.start-failed` for a run that was WaitingForRoute when the daemon ended (at the stop, or
/// at the next start).
pub const ROUTE_WAIT_AT_STOP: &str = "the daemon stopped while the run waited for its route";
/// The longest client label and idempotency key, in characters.
pub const MAX_LABEL_CHARS: usize = 200;
/// The kind of the run event the daemon records when a program of a run is about to start.
pub const ACTION_STARTED: &str = "runbook.action-started";
/// The kind of the run event the daemon records, with the runtime's reason, just before it ends a run that failed after it started.
pub const RUNTIME_FAILED: &str = "runbook.runtime-failed";
/// The kind of the run event the daemon records just before it ends a run whose step failed with nothing else to do
/// (`Finish::StepFailed`): the case, the action, the exit of its program and the reason.
pub const STEP_FAILED: &str = "runbook.step-failed";

/// Whether `text` is a principal: `person:<id>` or `agent:<id>`, `<id>` of `[a-z0-9][a-z0-9._-]{0,62}`.
pub fn check_principal(text: &str) -> Result<(), String> {
    let id = text.strip_prefix("person:").or_else(|| text.strip_prefix("agent:"));
    let ok = id.is_some_and(|id| {
        let mut chars = id.chars();
        let first = chars.next().is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
        first && id.len() <= 63 && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
    });
    if ok {
        Ok(())
    } else {
        Err(format!("--principal {text}: not a principal: use person:<id> or agent:<id>, <id> of 1 to 63 characters from a-z, 0-9, '.', '_' and '-', starting with a letter or digit"))
    }
}

/// What the daemon was started with.
#[derive(Clone, Debug)]
pub struct Config {
    pub state_dir: PathBuf,
    pub confinement_floor: Confinement,
    /// Directories a context path must lie under; none means no context from a path.
    pub context_roots: Vec<PathBuf>,
    pub runbook_dir: Option<PathBuf>,
    /// Who the daemon acts as (`check_principal`): fixed for the life of the process, and the only
    /// principal the runtime is ever given.
    pub principal: String,
}

/// What the operator sets besides `Config`: what a submit that names less gets. Nothing here is
/// read from a request, and the floor is never read as a default.
#[derive(Clone, Debug, Default)]
pub struct Policy {
    /// The level of a submit that names none (`--default-confinement`, else `confinement` of
    /// `runbook.toml`); none: a submit that names no level is refused.
    pub default_confinement: Option<Confinement>,
    /// The autonomy level of a submit that names none; none: every decision of such a run waits for a client.
    pub autonomy: Option<Autonomy>,
    /// The decider kind of a submit that names none.
    pub decider: Option<String>,
    /// The decider kinds the operator configured, sorted: the only names a run may take. The runtime
    /// that answers for them is the engine's; none configured: no run takes a level or a kind.
    pub kinds: Vec<String>,
    /// The model routes of the daemon's `runbook.toml` (`[gateways.*]`, `[routes.*]`, `[route_policy]`), their
    /// token files read; empty: no route. The view `routes` lists them, and a run that makes model calls takes one of
    /// them (`model_route`, else `route_policy.default`).
    pub routes: Arc<crate::routes::RouteTable>,
    /// The route `default` of a lone `[gateway]` table (the model decider's model as its alias), the route of a model
    /// run on a daemon whose file declares no route, as before routes; none without such a table.
    pub gateway_route: Option<crate::relay::RouteTable>,
    /// `[repositories.<name>]` of the daemon's `runbook.toml`: the repositories a delegation may change (`crate::handback`);
    /// empty: a delegation that names one is refused.
    pub repositories: Arc<crate::handback::Repositories>,
    /// `[delegation] copy_exclude`, `copy_max_entries` and `copy_max_bytes` of the daemon's `runbook.toml`: what the copy of
    /// a run's context leaves out and its bounds (`crate::context_copy`).
    pub copy: crate::context_copy::CopyRules,
}

/// The configured kinds as a sentence part.
fn listed(kinds: &[String]) -> String {
    if kinds.is_empty() {
        "none".to_owned()
    } else {
        kinds.join(", ")
    }
}

/// A name a client typed, short and on one line, for a refusal.
fn shown(text: &str) -> String {
    text.chars().take(64).map(|c| if c.is_control() { '?' } else { c }).collect()
}

/// The refusal of a level or a kind a daemon cannot give: a kind that is not one the operator
/// configured, or a level with no kind to take it. Names the kinds that are configured and no others.
/// It runs before anything of the run is written, and at the start of the daemon for its own defaults.
pub fn check_terms(terms: &Terms, kinds: &[String]) -> Result<(), String> {
    if let Some(kind) = &terms.decider {
        if !kinds.contains(kind) {
            return Err(format!("the decider kind {}: not configured; the configured kinds are {}", shown(kind), listed(kinds)));
        }
    }
    if let (Some(level), None) = (terms.autonomy, &terms.decider) {
        return Err(format!(
            "the autonomy level {} needs a decider kind to take the decisions it leaves to one: name one in `decider`, or start the daemon with a default; the configured kinds are {}",
            level.as_str(),
            listed(kinds)
        ));
    }
    Ok(())
}

/// Whether `text` is a name a rule or a decider answers under: `rule:<name>` or `decider:<kind>`.
fn decider_label(text: &str) -> bool {
    let rest = text.strip_prefix("rule:").or_else(|| text.strip_prefix("decider:"));
    rest.is_some_and(|r| !r.is_empty()) && long_enough(text) && !text.chars().any(char::is_control)
}

/// The label a client's answer is recorded under. The one function that reads it: the text is
/// trimmed of ASCII whitespace, and what is left is ASCII with no control character (a label with
/// any other character is refused, so no look-alike of a reserved prefix reaches the record), at
/// most `MAX_LABEL_CHARS` long, and does not start with `rule:`, `decider:` or `router:` in any
/// case: those are the daemon's own names for an answer of a rule, a decider or a router. A
/// refusal names the field and the rule and prints no byte of the label.
pub fn client_label(field: &str, text: &str) -> Result<String, String> {
    if !long_enough(text) {
        return Err(format!("{field} is longer than {MAX_LABEL_CHARS} characters"));
    }
    let label = text.trim_matches(|c: char| c.is_ascii_whitespace());
    if !label.bytes().all(|b| b.is_ascii_graphic() || b == b' ') {
        return Err(format!("{field}: a label is printable ASCII; this one holds a character that is not (or a control character) and is refused"));
    }
    let lower = label.to_ascii_lowercase();
    if ["rule:", "decider:", "router:"].iter().any(|p| lower.starts_with(p)) {
        return Err(format!("{field}: the names that start with `rule:`, `decider:` or `router:` are the daemon's own for an answer of a rule, a decider or a router; a client's answer takes another label"));
    }
    Ok(label.to_owned())
}

/// The refusal of a default level below the floor, naming both values. The daemon does not start
/// with one: a default that every submit it applies to would refuse is a configuration that
/// disagrees with itself.
pub fn check_default_confinement(floor: Confinement, default: Confinement) -> Result<(), String> {
    if default.at_least(floor) {
        return Ok(());
    }
    Err(format!(
        "the default confinement {} is below the floor {}: --default-confinement (or `confinement` of runbook.toml) and --min-confinement disagree; raise the default or lower the floor",
        default.as_str(),
        floor.as_str()
    ))
}

/// A request the daemon cannot take at all (no outcome of the specification), or a failure of its own.
#[derive(Debug, PartialEq, Eq)]
pub enum Fault {
    Invalid(String),
    Internal(String),
    /// The daemon got its stop signal and waits for `live_runs` runs to end: it takes no new run.
    Draining { live_runs: usize },
    /// A read the daemon refuses by name, with its HTTP status: a run it does not know, or a file of a run it does not
    /// serve (`engine::products::Refusal`). No outcome of the specification: a view has none.
    Refused { status: u16, what: &'static str, message: String },
}

impl Fault {
    fn no_such_run(run_id: &str) -> Fault {
        Fault::Refused { status: 404, what: "no such run", message: format!("the daemon has no run {}", crate::engine::products::said(run_id)) }
    }
}

impl From<crate::engine::products::Refusal> for Fault {
    fn from(r: crate::engine::products::Refusal) -> Fault {
        Fault::Refused { status: r.status(), what: r.word(), message: r.message() }
    }
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
    /// The view the `Location` of the answer names the run in, when not the command's own (`api::routes`).
    pub location_view: Option<&'static str>,
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

/// Every outcome of every command of the domain, by wire name, in the order the domain file declares
/// them. A test holds it equal to the domain file, order included, and to the OpenAPI document.
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
    o("submit-run", "route-refused", 422, Some("RouteRefused")),
    o("submit-run", "no-daemon", 409, Some("NoSuchDaemon")),
    o("submit-run", "no-session", 409, Some("NoSuchSession")),
    o("submit-run", "below-floor", 409, Some("BelowConfinementFloor")),
    o("submit-run", "session-closed", 409, Some("SessionClosedToRuns")),
    o("submit-run", "already-submitted", 202, None),
    o("submit-run", "submitted", 202, None),
    o("delegate", "intake-too-large", 422, Some("IntakeTooLarge")),
    o("delegate", "route-refused", 422, Some("RouteRefused")),
    o("delegate", "no-runbook", 422, Some("NoSuchRunbook")),
    o("delegate", "context-outside-roots", 422, Some("ContextOutsideRoots")),
    o("delegate", "repository-refused", 422, Some("RepositoryRefused")),
    o("delegate", "no-daemon", 409, Some("NoSuchDaemon")),
    o("delegate", "no-session", 409, Some("NoSuchSession")),
    o("delegate", "session-closed", 409, Some("SessionClosedToRuns")),
    o("delegate", "already-submitted", 202, None),
    o("delegate", "delegated", 202, None),
    o("cancel-run", "no-run", 404, Some("NoSuchRun")),
    o("cancel-run", "cancelled", 202, None),
    o("cancel-run", "already-ended", 409, Some("RunAlreadyEnded")),
    o("start-run", "no-run", 404, Some("NoSuchRun")),
    o("start-run", "started", 202, None),
    o("start-run", "wrong-state", 409, Some("RunStateConflict")),
    o("fail-run-start", "no-run", 404, Some("NoSuchRun")),
    o("fail-run-start", "failed", 202, None),
    o("fail-run-start", "wrong-state", 409, Some("RunStateConflict")),
    o("wait-on-key", "no-run", 404, Some("NoSuchRun")),
    o("wait-on-key", "waiting-on-key", 202, None),
    o("wait-on-key", "wrong-state", 409, Some("RunStateConflict")),
    o("take-key", "no-run", 404, Some("NoSuchRun")),
    o("take-key", "took-key", 202, None),
    o("take-key", "wrong-state", 409, Some("RunStateConflict")),
    o("wait-for-route", "no-run", 404, Some("NoSuchRun")),
    o("wait-for-route", "waiting-for-route", 202, None),
    o("wait-for-route", "wrong-state", 409, Some("RunStateConflict")),
    o("end-route-wait", "no-run", 404, Some("NoSuchRun")),
    o("end-route-wait", "stopped", 202, None),
    o("end-route-wait", "wrong-state", 409, Some("RunStateConflict")),
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
    o("answer-decision", "unknown-option", 422, Some("UnknownOption")),
    o("answer-decision", "no-request", 404, Some("NoSuchDecisionRequest")),
    o("answer-decision", "not-of-run", 409, Some("DecisionNotOfRun")),
    o("answer-decision", "withdrawn", 409, Some("DecisionWithdrawn")),
    o("answer-decision", "already-answered", 409, Some("DecisionAlreadyAnswered")),
    o("answer-decision", "no-run", 409, Some("NoSuchRun")),
    o("answer-decision", "run-not-waiting", 409, Some("RunNotWaiting")),
    o("answer-decision", "answered", 202, None),
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
        "RunDelegated" => "sdc.daemon.RunDelegated",
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
        "RunWaitingForRoute" => "sdc.daemon.RunWaitingForRoute",
        "RunStoppedOnRoute" => "sdc.daemon.RunStoppedOnRoute",
        "RunWaitingOnKey" => "sdc.daemon.RunWaitingOnKey",
        "RunTookKey" => "sdc.daemon.RunTookKey",
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
        location_view: None,
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
        location_view: None,
    }
}

// ---- inputs of the client commands ----

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OpenSession {
    pub session_id: String,
    pub client: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CloseSession {
    pub session_id: String,
}

/// The settings of a submit: every value a string. A value of another JSON type is refused naming
/// the setting, and for a cap (`crate::caps::KEYS`) saying that a cap is written as a string.
fn settings_of_strings<'de, D: serde::Deserializer<'de>>(d: D) -> Result<BTreeMap<String, String>, D::Error> {
    let raw = BTreeMap::<String, Value>::deserialize(d)?;
    raw.into_iter()
        .map(|(key, value)| match value {
            Value::String(text) => Ok((key, text)),
            other => {
                let kind = match other {
                    Value::Null => "null",
                    Value::Bool(_) => "a boolean",
                    Value::Number(_) => "a number",
                    Value::Array(_) => "an array",
                    _ => "an object",
                };
                let hint = if crate::caps::KEYS.contains(&key.as_str()) { ": a cap is a string of a whole number of 1 or more, such as \"50\"" } else { "" };
                Err(serde::de::Error::custom(format!("the setting {key} must be a string, not {kind}{hint}")))
            }
        })
        .collect()
}

/// What a body serde refused for a number out of range (`1e999`) is refused for, naming the member
/// whose value it was: serde refuses the whole body before a setting is read, and its message names
/// only a line and a column. `None` when the error is another, or no member stands before the number.
pub fn out_of_range_member(body: &[u8], e: &serde_json::Error) -> Option<String> {
    if !e.to_string().starts_with("number out of range") {
        return None;
    }
    let text = std::str::from_utf8(body).ok()?;
    let mut offset = 0usize;
    for (n, line) in text.split('\n').enumerate() {
        if n + 1 == e.line() {
            offset += line.char_indices().nth(e.column()).map_or(line.len(), |(i, _)| i);
            break;
        }
        offset += line.len() + 1;
    }
    let before = text.get(..offset.min(text.len()))?;
    let colon = before.rfind(':')?;
    let after = before[colon + 1..].trim_start();
    let digits = after.find(|c: char| !(c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E'))).unwrap_or(after.len());
    if digits == 0 || !after[digits..].chars().all(|c| c.is_whitespace() || matches!(c, ',' | '}' | ']')) {
        return None;
    }
    let quoted = before[..colon].trim_end().strip_suffix('"')?;
    let key = &quoted[quoted.rfind('"')? + 1..];
    let hint = if crate::caps::KEYS.contains(&key) { ": a cap is a string of a whole number of 1 or more, such as \"50\"" } else { "" };
    Some(format!("the setting {key} is a number out of range, not a string{hint}"))
}

/// The names a submit does not carry, as a setting or as a member of the request: the daemon's operator names the
/// gateway of the model programs (`runbook.toml`, `[gateway]`; `docs/engine.md`, "A gateway"), never a client.
const GATEWAY_NAMES: [&str; 4] = ["gateway", "base_url", "token", "token_file"];
/// The members of a `[gateway]` table a refusal may echo after `gateway.`: any other text there could be a value.
const GATEWAY_MEMBERS: [&str; 4] = ["base_url", "token", "token_file", "model"];

/// How a refusal names `key` when it names a gateway, in any case: the key itself when it is one of the names, else
/// the words "a gateway.* name" (the rest of such a key is the client's text and could be a value). `None`: no gateway.
fn gateway_name(key: &str) -> Option<String> {
    let lower = key.to_ascii_lowercase();
    if GATEWAY_NAMES.contains(&lower.as_str()) || lower.strip_prefix("gateway.").is_some_and(|m| GATEWAY_MEMBERS.contains(&m)) {
        return Some(key.to_owned());
    }
    lower.starts_with("gateway.").then(|| "a gateway.* name".to_owned())
}

fn gateway_refusal(what: &str, name: &str) -> Fault {
    Fault::Invalid(format!("{what} {name} is refused: a submit names no gateway, address or token; the daemon's operator names the gateway of the model programs in its runbook.toml (docs/engine.md, \"A gateway\")"))
}

/// The members of a submit's body, before it is read as one: a member that names a gateway is refused by its name and
/// never with its value (`SubmitRun` has no such member, and a member it does not know would be dropped unread).
pub fn refuse_gateway_members(body: &[u8]) -> Result<(), Fault> {
    let Ok(Value::Object(members)) = serde_json::from_slice::<Value>(body) else { return Ok(()) };
    match members.keys().find_map(|k| gateway_name(k)) {
        Some(name) => Err(gateway_refusal("the member", &name)),
        None => Ok(()),
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct SubmitRun {
    pub runbook_id: String,
    #[serde(default)]
    pub runbook_version: Option<String>,
    pub intake: String,
    #[serde(default)]
    pub context_path: Option<String>,
    #[serde(default, deserialize_with = "settings_of_strings")]
    pub settings: BTreeMap<String, String>,
    /// Absent: the daemon's default level; a daemon with none refuses the submit by name.
    #[serde(default)]
    pub confinement: Option<Confinement>,
    /// Absent: the daemon's default level, else none and every decision waits for a client.
    #[serde(default)]
    pub autonomy: Option<Autonomy>,
    /// Absent: the daemon's default kind. A kind name among those the operator configured.
    #[serde(default)]
    pub decider: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    /// The serial key of the run: runs with one key run one at a time, in submit order; the submitter names it, the
    /// daemon never derives one. None: the run waits for no other.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial_key: Option<String>,
    /// The route of the run's role `default`, by the name the daemon's file declares; absent: `route_policy.default`
    /// for a run that makes model calls.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_route: Option<String>,
    /// The route of each further role, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_routes: Option<BTreeMap<String, String>>,
    /// Names or relative paths the copy of the context leaves out besides the daemon's (`crate::context_copy`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workspace_exclude: Vec<String>,
}

/// The members of a submit the handler fills about the run's route: a client never sends them, and one that does is
/// refused by the member's name (an alias in particular is the daemon's to fill, never a client's to name).
const ROUTE_FILLED: [&str; 3] = ["model_route_alias", "model_route_source", "route_refused"];

/// The members of a submit's body the handler fills about the route, refused by name before the body is read.
pub fn refuse_route_members(body: &[u8]) -> Result<(), Fault> {
    let Ok(Value::Object(members)) = serde_json::from_slice::<Value>(body) else { return Ok(()) };
    match members.keys().find(|k| ROUTE_FILLED.contains(&k.as_str())) {
        Some(name) => Err(Fault::Invalid(format!("the member {name} is refused: the daemon fills it from its runbook.toml; a submit names a route by its name in model_route (docs/model-routes.md, section 2)"))),
        None => Ok(()),
    }
}

/// Why `name`, given in the submit's member `member`, is no route the daemon declares; none when it is one. The
/// reason names what the value is (empty, an address, a gateway's alias, a token or another text that is no route
/// name, or a name the file does not declare) and never holds the value.
fn route_name_refusal(routes: &crate::routes::RouteTable, member: &str, name: &str) -> Option<String> {
    if routes.route(name).is_some() {
        return None;
    }
    let why = if name.is_empty() {
        "is empty".to_owned()
    } else if routes.routes().any(|r| r.alias == name) {
        "names an alias of a gateway's model, not a route: a submit names the route by its name, and the daemon fills its alias".to_owned()
    } else if name.contains(['/', ':']) {
        "names an address: a submit names no address; the gateway's address is the daemon's".to_owned()
    } else if !crate::routes::is_name(name) {
        "is no route name (1 to 64 letters, digits, '.', '_' or '-'): a token or any other secret is refused, and its value is never shown".to_owned()
    } else {
        "names a route the daemon declares no route of".to_owned()
    };
    Some(format!("{member} {why}; name a declared route"))
}

/// The run's route as the handler resolves it at the submit (`docs/model-routes.md`, sections 2 and 11): the route of
/// each role and the record of it, or none for a run that takes no route; `Err` is the reason of `route-refused`. The
/// run's word comes first (`model_route`, `model_routes.<role>`), then `route_policy.default`. A run may take a route
/// only when it has a model step: its decider is the kind model above A0, or a program of its runbook calls a model
/// through the route file (`ProgramSteps::ModelCall`: an agent node, or a program whose per-call budget is above 0,
/// `crate::budget::calls_a_model`). Any other run makes no model call and takes no route; the default is not applied to
/// it. Of those, a run whose runbook runs no program is refused when it names a route (the reason names the member);
/// one whose programs call no model (`ProgramSteps::NoModelCall`) is admitted with no route, and the submit records the
/// routes it named as not taken ([`route_not_taken`]). A run
/// whose model calls come from its programs only and that names no route takes `route_policy.default` only when the
/// file declares one and the relay serves runs (`serves_routes`); else it takes none and runs as before routes. A
/// daemon whose file declares no route gives a model run the route `default` of its lone `[gateway]`, as before
/// routes, and refuses any other named route; a run whose model calls come from its programs only does not take it, and
/// one that names it is refused naming the member. A runbook with route entries (`protocol`, a daemon that declares
/// routes) adds its word between the run's and the default (`crate::admission::for_run`): its entry for a role the run
/// names none for, its entry for the role `default` before `route_policy.default`, and every route of the run within its
/// `routes_allowed`.
fn route_of_run(policy: &Policy, input: &SubmitRun, terms: &Terms, programs: ProgramSteps, serves_routes: bool, protocol: Option<&ProtocolWord>) -> Result<Option<(RunRoute, crate::relay::RouteTable)>, String> {
    let routes = &policy.routes;
    let roles = input.model_routes.clone().unwrap_or_default();
    if let Some(name) = &input.model_route {
        if let Some(why) = route_name_refusal(routes, "model_route", name) {
            return Err(why);
        }
    }
    for (role, name) in &roles {
        if !crate::routes::is_name(role) {
            return Err("model_routes: a role is 1 to 64 letters, digits, '.', '_' or '-'; this one is not, and is not shown".to_owned());
        }
        if role == "default" {
            return Err("model_routes.default: the role default is the run's model_route; name it there".to_owned());
        }
        if let Some(why) = crate::relay::prefixed_role_refusal(role, &format!("model_routes.{role}")) {
            return Err(why);
        }
        if let Some(why) = route_name_refusal(routes, &format!("model_routes.{role}"), name) {
            return Err(why);
        }
    }
    let decider_calls = terms.makes_model_calls();
    if !decider_calls && programs == ProgramSteps::NoProgram {
        let member = match (&input.model_route, roles.keys().next()) {
            (Some(_), _) => Some("model_route".to_owned()),
            (None, Some(role)) => Some(format!("model_routes.{role}")),
            (None, None) => None,
        };
        if let Some(member) = member {
            return Err(format!("{member} names a route, and the run makes no model call (its runbook runs no bound program and no agent node, and its decider is not the kind model at a level above A0): a submit for such a run names no route"));
        }
        return Ok(None);
    }
    if !decider_calls {
        let named = input.model_route.iter().map(|name| ("model_route".to_owned(), name)).chain(roles.iter().map(|(role, name)| (format!("model_routes.{role}"), name)));
        for (member, name) in named {
            if routes.is_interim(name) {
                return Err(format!("{member} names the interim route default of the daemon's lone [gateway], which only a run whose decider is the kind model above A0 takes: a run whose model calls come from its programs only names a route the daemon's file declares in [routes]"));
            }
        }
        if programs == ProgramSteps::NoModelCall {
            return Ok(None);
        }
    }
    let protocol = protocol.filter(|_| !routes.names().is_empty());
    if !decider_calls && input.model_route.is_none() && roles.is_empty() {
        let declared = routes.default_route().filter(|name| !routes.is_interim(name));
        let protocol_default = protocol.is_some_and(|p| p.declared.has_role("default"));
        if (declared.is_none() && !protocol_default) || !serves_routes {
            return Ok(None);
        }
    }
    if routes.names().is_empty() {
        let Some(table) = policy.gateway_route.clone() else { return Ok(None) };
        let Some((route, _)) = table.get("default") else { return Ok(None) };
        let record = RunRoute { route: route.name.clone(), alias: route.alias.clone(), source: RouteSource::Default, roles: BTreeMap::new(), sources: BTreeMap::new() };
        return Ok(Some((record, table)));
    }
    let chosen = match protocol {
        Some(p) => p.for_run(input.model_route.as_deref(), &roles, routes.default_route())?,
        None => {
            let (name, source) = match (&input.model_route, routes.default_route()) {
                (Some(named), _) => (named.clone(), RouteSource::Run),
                (None, Some(default)) => (default.to_owned(), RouteSource::Default),
                (None, None) => return Err(NO_DEFAULT.to_owned()),
            };
            std::iter::once(("default".to_owned(), (name, source))).chain(roles.iter().map(|(role, name)| (role.clone(), (name.clone(), RouteSource::Run)))).collect()
        }
    };
    table_of(routes, chosen, NO_DEFAULT).map(Some)
}

/// Why a run whose runbook's programs call no model and whose decider is no model takes no route, also the one its submit
/// names.
const NOT_TAKEN: &str = "the run makes no model call (its runbook runs no agent node and no program whose per-call budget is above 0, and its decider is not the kind model at a level above A0): it takes no route and wakes none";

/// The routes a submit named for a run that takes none because it makes no model call (`route_of_run`,
/// `ProgramSteps::NoModelCall`): the member `model_route_not_taken` of its first event `runbook.submitted`, with the
/// reason. None when the run may make model calls or the submit named no route.
fn route_not_taken(input: &SubmitRun, terms: &Terms, programs: ProgramSteps) -> Option<Value> {
    let roles = input.model_routes.as_ref().filter(|r| !r.is_empty());
    if terms.makes_model_calls() || programs != ProgramSteps::NoModelCall || (input.model_route.is_none() && roles.is_none()) {
        return None;
    }
    let mut record = json!({"reason": NOT_TAKEN});
    if let Some(name) = &input.model_route {
        record["model_route"] = json!(name);
    }
    if let Some(roles) = roles {
        record["model_routes"] = json!(roles);
    }
    Some(record)
}

/// The refusal of a run that makes model calls and that nothing gives a route.
const NO_DEFAULT: &str = "the run makes model calls and names no route, and the daemon's runbook.toml has no route_policy.default: name one in model_route";

/// The route entries of the runbook a run or a delegation names (`crate::admission`), read at its submit, with the facts of
/// the declared routes (the view `routes`) the entries are admitted against.
struct ProtocolWord {
    runbook: String,
    declared: crate::admission::Declared,
    facts: Vec<crate::admission::Facts>,
}

impl ProtocolWord {
    /// The route of each role of the run (`crate::admission::for_run`) and the setting that chose it.
    fn for_run(&self, route: Option<&str>, roles: &BTreeMap<String, String>, default: Option<&str>) -> Result<BTreeMap<String, (String, RouteSource)>, String> {
        use crate::admission::{for_run, Asked, By};
        let chosen = for_run(&self.runbook, &self.declared, &self.facts, Asked { route, roles, default })?;
        let source = |by: By| match by {
            By::Run => RouteSource::Run,
            By::Protocol => RouteSource::Protocol,
            By::Default => RouteSource::Default,
        };
        Ok(chosen.into_iter().map(|(role, (name, by))| (role, (name, source(by)))).collect())
    }
}

/// The record and the relay's table of a run whose role `default` and further roles are on the routes of `chosen`; with no
/// role `default` the run is refused with `no_default`.
fn table_of(routes: &crate::routes::RouteTable, mut chosen: BTreeMap<String, (String, RouteSource)>, no_default: &str) -> Result<(RunRoute, crate::relay::RouteTable), String> {
    let (name, source) = chosen.remove("default").ok_or_else(|| no_default.to_owned())?;
    let resolve = |name: &str| routes.resolve(name).map(crate::relay::ResolvedRoute::from).ok_or_else(|| format!("the route {name} has no gateway the daemon declares"));
    let first = resolve(&name)?;
    let record = RunRoute {
        route: first.name.clone(),
        alias: first.alias.clone(),
        source,
        roles: chosen.iter().map(|(role, (name, _))| (role.clone(), name.clone())).collect(),
        sources: chosen.iter().map(|(role, (_, source))| (role.clone(), *source)).collect(),
    };
    let mut table = crate::relay::RouteTable::default().with("default", first, source.as_str());
    for (role, (name, source)) in &chosen {
        table = table.with(role, resolve(name)?, source.as_str());
    }
    Ok((record, table))
}

/// The relay's table of a run from its record (`table_of` made it at the admission): a run that waited on its serial key
/// across a restart is taken up again on the routes it was admitted with, resolved against the routes the daemon declares
/// now. `Err` is why the record cannot be resolved.
fn table_of_record(policy: &Policy, record: &RunRoute) -> Result<crate::relay::RouteTable, String> {
    let routes = &policy.routes;
    if routes.names().is_empty() {
        // the interim route `default` of a lone [gateway]
        return policy.gateway_route.clone().filter(|t| t.get("default").is_some()).ok_or_else(|| "the daemon declares no route and no [gateway]: the run's route default is gone".to_owned());
    }
    let mut chosen = BTreeMap::from([("default".to_owned(), (record.route.clone(), record.source))]);
    for (role, name) in &record.roles {
        let source = record.sources.get(role).copied().ok_or_else(|| format!("the run's record does not say what chose the route of its role {role}"))?;
        chosen.insert(role.clone(), (name.clone(), source));
    }
    table_of(routes, chosen, "the run's record names no route of its role default").map(|(_, table)| table)
}

/// A delegation's route, chosen as a run's (`route_of_run`): the route it names (`model_route`), else the daemon's
/// `route_policy.default`, as the role `default`; `Err` is the reason of `route-refused`, with the same refusals of a named
/// route. A delegation always makes model calls, so a daemon with no default route refuses one that names none. The
/// runbook a delegation names as its protocol adds its route entries as a run's runbook does (`protocol`).
fn route_of_delegation(policy: &Policy, named: Option<&str>, protocol: Option<&ProtocolWord>) -> Result<(RunRoute, crate::relay::RouteTable), String> {
    const NO_ROUTE: &str = "the delegation names no route, and the daemon's runbook.toml has no route_policy.default: name one in model_route";
    let routes = &policy.routes;
    if let Some(why) = named.and_then(|name| route_name_refusal(routes, "model_route", name)) {
        return Err(why);
    }
    let chosen = match protocol {
        Some(p) => p.for_run(named, &BTreeMap::new(), routes.default_route())?,
        None => match (named, routes.default_route()) {
            (Some(named), _) => BTreeMap::from([("default".to_owned(), (named.to_owned(), RouteSource::Run))]),
            (None, Some(default)) => BTreeMap::from([("default".to_owned(), (default.to_owned(), RouteSource::Default))]),
            (None, None) => return Err(NO_ROUTE.to_owned()),
        },
    };
    table_of(routes, chosen, NO_ROUTE)
}

/// The input of `Delegate` a client sends; the handler fills the rest (`run_id`, `runbook_id`, `confinement`, `route`, `path`, the
/// facts and the instant).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Delegate {
    pub request: String,
    /// The route by the name the daemon declares it under; none: the daemon's default route (`[route_policy] default`).
    #[serde(default)]
    pub model_route: Option<String>,
    /// The runbook to run with the request as its intake; none: the free run (`crate::delegation`).
    #[serde(default)]
    pub protocol: Option<String>,
    /// A directory under a context root, copied into the run's workspace.
    #[serde(default)]
    pub workspace: Option<String>,
    /// Names or relative paths the copy of the workspace leaves out besides the daemon's (`crate::context_copy`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workspace_exclude: Vec<String>,
    #[serde(default, deserialize_with = "settings_of_strings")]
    pub settings: BTreeMap<String, String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    /// The serial key of the run, as a submit's (`SubmitRun::serial_key`).
    #[serde(default)]
    pub serial_key: Option<String>,
    /// Who asked: the MCP client's name (`runbook mcp` sends `clientInfo.name`), recorded in `runbook.delegated`.
    #[serde(default)]
    pub caller: Option<String>,
    /// The session's label (`RUNBOOK_CALLER` of the session `runbook mcp` serves), recorded in `runbook.delegated`.
    #[serde(default)]
    pub caller_label: Option<String>,
    /// The repository the delegation changes, by the name of its entry in `runbook.toml` (`[repositories.<name>]`, never a
    /// path): the run is the protocol `delegation-change`, working in a worktree of it, and its change is pushed as the branch
    /// `runbook/<run-id>` when it delivers (`crate::handback`). None: the run changes no repository.
    #[serde(default)]
    pub repository: Option<String>,
}

/// A run its command admitted: what `Daemon::enter` writes and starts.
struct Admitted {
    run_id: String,
    runbook_id: String,
    runbook_version: Option<String>,
    intake: String,
    context_path: Option<String>,
    context_dir: Option<PathBuf>,
    /// What the submit or the delegation adds to the daemon's exclude list: `runbook.submitted` records it.
    workspace_exclude: Vec<String>,
    settings: BTreeMap<String, String>,
    confinement: Confinement,
    source: ConfinementSource,
    terms: Terms,
    session_id: Option<String>,
    idempotency_key: Option<String>,
    /// The serial key the submit named; none: the run waits for no other.
    serial_key: Option<String>,
    /// The run's model route as its record keeps it and the route of each of its roles (`route_of_run`, `route_of_delegation`);
    /// none: the run is on no route.
    route: Option<(RunRoute, crate::relay::RouteTable)>,
    /// The routes the submit named for a run that takes none because it makes no model call, with the reason
    /// ([`route_not_taken`]): the member `model_route_not_taken` of its first event. None: the submit named none, or the
    /// run took one.
    route_not_taken: Option<Value>,
    /// The route of a delegation, by name; none: the run is no delegation.
    delegation: Option<String>,
    /// The wait for the run's routes when one could not answer at the submit (`read_routes`); none: the run starts at once.
    wait: Option<RouteWait>,
}

/// How often a run that waits for its route reads the route's status, in milliseconds of the daemon's clock
/// (`docs/model-routes.md`, section 3).
pub const ROUTE_READ_EVERY_MILLIS: i64 = 5_000;

/// A run whose routes could not all answer at the submit: the daemon holds it until they can, or its wait is over.
#[derive(Clone, Debug)]
struct RouteWait {
    /// The declared routes of the run, each read again until all can answer.
    routes: Vec<String>,
    /// The route that could not answer with the least wait (the first of equals), and the phase its gateway gave.
    route: String,
    phase: Option<String>,
    /// The wait: the least `start_wait_seconds` of the routes that could not answer (none declared: no wait).
    wait_millis: i64,
    /// The routes the run wakes once when its wait starts: managed, the wake on, the phase `stopped` at the submit.
    wake: Vec<(String, Option<String>)>,
}

/// The run event that books a wake of a route, or says the day's wake cap does not cover one.
pub const ROUTE_WAKE: &str = "runbook.route-wake";
/// The run event of a run that waits behind the runs of its serial key (record: serial_key, behind).
pub const KEY_WAIT_EVENT: &str = "runbook.waiting-on-key";
/// The run event of a run that took its serial key after a wait (record: serial_key).
pub const KEY_TAKEN_EVENT: &str = "runbook.key-taken";

/// How often a run that waits on its serial key looks at the runs ahead of it.
const KEY_LOOK_EVERY: std::time::Duration = std::time::Duration::from_millis(20);

/// What the wait of a run on its serial key came to (`Daemon::wait_for_key`).
enum Keyed {
    /// The run names no key, or no live run with it was submitted before it: it waited for nothing.
    Free,
    /// It waited and took the key: what was read at its submit is old, and its routes are read again.
    Took,
    /// It was cancelled while it waited, or the daemon stops: it keeps the state it has.
    Left,
}

/// A run its thread took up (`State::take_up`): when the thread returns, also by a panic, the state lets it go and the
/// run frees its serial key (`State::ahead_on_key`).
struct TakenUp {
    daemon: Daemon,
    run_id: String,
}

impl Drop for TakenUp {
    fn drop(&mut self) {
        self.daemon.st().let_go(&self.run_id);
    }
}

/// The run event of the gateway's answer to a wake.
pub const ROUTE_WAKE_ANSWERED: &str = "runbook.route-wake-answered";
/// The reason of a wake that was not sent.
pub const WAKE_CAP_REACHED: &str = "wake cap reached";

/// The reason of a wake that was not sent because a wake of the same route was sent within the route's wait.
pub const WAKE_JOINED: &str = "joined a wake sent within the wait";

/// A wake of a route a run sent, from the journal: its `runbook.route-wake` and the answer the daemon wrote for it.
struct SentWake<'a> {
    run_id: &'a str,
    at: &'a str,
    record: &'a Value,
    /// The record of the answer to this wake: the run's next `runbook.route-wake-answered` of the same route (a run sends
    /// one wake of a route at a time), or, when the run was closed before the answer came, the daemon's line
    /// `route.wake-answered` of that run and route. None while the wake is out, and for a wake whose daemon went away
    /// before its answer came.
    answer: Option<&'a Value>,
}

/// The sent wakes of `route` of every run, in the order of the journal, each with its answer: a walk of the route's wake
/// events only (`State::wake_events_of`).
fn sent_wakes<'a>(st: &'a State, route: &str) -> Vec<SentWake<'a>> {
    let mut wakes: Vec<SentWake<'a>> = Vec::new();
    // of each run, the indexes of its wakes not yet answered, oldest first
    let mut out: HashMap<&'a str, std::collections::VecDeque<usize>> = HashMap::new();
    for e in st.wake_events_of(route) {
        if e.kind == ROUTE_WAKE && e.record["sent"] == true {
            out.entry(e.run_id.as_str()).or_default().push_back(wakes.len());
            wakes.push(SentWake { run_id: &e.run_id, at: &e.at, record: &e.record, answer: None });
        } else if e.kind == ROUTE_WAKE_ANSWERED {
            if let Some(i) = out.get_mut(e.run_id.as_str()).and_then(std::collections::VecDeque::pop_front) {
                wakes[i].answer = Some(&e.record);
            }
        }
    }
    // an answer that came after its run was closed: the run sends no wake after it, so it answers the run's wake still out
    for a in st.wake_answers().iter().filter(|a| a.record["route"] == route) {
        if let Some(i) = out.get_mut(a.run_id.as_str()).and_then(std::collections::VecDeque::pop_front) {
            wakes[i].answer = Some(&a.record);
        }
    }
    wakes
}

/// Whether the run `run_id` is live.
fn run_lives(st: &State, run_id: &str) -> bool {
    st.run(run_id).is_some_and(|r| r.state.is_live())
}

/// The newest sent wake of `route` that is less than `wait_millis` older than `now_millis` and that may still start the
/// model, from the run events of every run (the journal, so a daemon started again finds it): its run and its instant.
/// A wake may start the model when the gateway took it (a 2xx), and while its run lives and its answer is not in or
/// was one the run sends the wake again on (a 5xx, no answer). A wake answered `spend-ceiling` is joined too: its answer
/// ends the run that joins it. A wake answered otherwise (a 4xx), or whose run ended without a 2xx (also one whose
/// daemon went away before its answer came: nothing says the gateway took it), is not joined.
fn wake_sent_within(st: &State, route: &str, now_millis: i64, wait_millis: i64) -> Option<(String, String)> {
    sent_wakes(st, route).into_iter().rev().find_map(|w| {
        let at = crate::clock::parse_instant(w.at)?;
        let within = at <= now_millis && now_millis - at < wait_millis;
        let joinable = match w.answer {
            Some(a) if wake_taken(a) || a["code"] == crate::routes::SPEND_CEILING => true,
            Some(a) => a["status"].as_u64().is_none_or(|s| s >= 500) && run_lives(st, w.run_id),
            None => run_lives(st, w.run_id),
        };
        (within && joinable).then(|| (w.run_id.to_owned(), w.at.to_owned()))
    })
}

/// How long a run that sent a wake the gateway could not serve waits before it sends it again, in milliseconds of the
/// daemon's clock (`docs/model-routes.md`, section 3 point 4): within the run's wait for its route, never past it.
pub const WAKE_AGAIN_AFTER_MILLIS: i64 = 30_000;

/// What a wake of [`Daemon::wake_route`] came to for the run.
enum Woke {
    /// Sent and answered other than `spend-ceiling`, a 5xx or no answer, or not sent (no cap, the cap reached, a booking
    /// not written).
    Sent,
    /// Sent and answered a 5xx other than `spend-ceiling`, or not answered in time: it booked nothing, and the run sends
    /// it again after [`WAKE_AGAIN_AFTER_MILLIS`] within its wait.
    Failed,
    /// The gateway answered `spend-ceiling`.
    Ceiling,
    /// The run joined the wake the named run sent within the route's wait.
    Joined(String),
}

/// Whether the gateway answered the wake the run `run` sent for `route` with `spend-ceiling`.
fn answered_ceiling(st: &State, run: &str, route: &str) -> bool {
    let ceiling = |record: &Value| record["route"] == route && record["code"] == crate::routes::SPEND_CEILING;
    st.events_of(run).any(|e| e.kind == ROUTE_WAKE_ANSWERED && ceiling(&e.record)) || st.wake_answers().iter().any(|a| a.run_id == run && ceiling(&a.record))
}

/// Whether the record of a `runbook.route-wake-answered` says the gateway took the wake: a 2xx answer.
fn wake_taken(answer: &Value) -> bool {
    answer["status"].as_u64().is_some_and(|s| (200..300).contains(&s))
}

/// What the sent wakes of `route` booked on `day` (the first 10 characters of the daemon's instant), from the run
/// events `runbook.route-wake` of every run: the journal is the booking, so a daemon started again counts them. A wake
/// books only when the gateway took it: its answer ([`SentWake::answer`], also one that came after its run was closed)
/// is a 2xx; a 5xx, a 4xx or no answer within the bound books nothing. A wake whose answer is not written stays booked:
/// while it is out, so two runs cannot both pass the cap while their wakes are out, and for good when the daemon went
/// away before its answer came (killed with the wake out): the gateway may have taken it, and the cap counts more,
/// never less (decision D11 of the unit's coordinator).
fn wakes_booked(st: &State, route: &str, day: &str) -> u64 {
    sent_wakes(st, route)
        .into_iter()
        .filter(|w| w.record["day"] == day && w.answer.is_none_or(wake_taken))
        .filter_map(|w| w.record["cost_micro_usd"].as_u64())
        .fold(0u64, u64::saturating_add)
}

/// The run event of a read of the pod of a priced managed route that changed whether a run that takes the route has
/// the pod: ready or not (record: route, phase). The daemon reads such a route while a started run takes it.
pub const ROUTE_POD_PHASE: &str = "runbook.route-pod-phase";
/// The run event of the run's share of the pod's time of a priced managed route, written just before the run ends
/// (record: route, pod_millis, share_millis, price_per_hour_micro_usd, cost_micro_usd).
pub const ROUTE_POD_TIME: &str = "runbook.route-pod-time";

/// The pod's price per hour of `name` when it is a managed route that declares one (`price_per_hour_micro_usd`).
fn pod_price(routes: &crate::routes::RouteTable, name: &str) -> Option<u64> {
    routes.route(name).filter(|r| r.kind == crate::routes::RouteKind::Managed).and_then(|r| r.wake.price_per_hour_micro_usd)
}

/// Whether the run takes `route`: as its route or as the route of one of its roles.
fn takes_route(run: &Run, route: &str) -> bool {
    run.model_route.as_ref().is_some_and(|r| r.route == route || r.roles.values().any(|v| v == route))
}

/// The uses of the pod of `route` ([`crate::pod_time`]) by the runs that started on it and whose time on it meets
/// `[from, to)`, in submit order, each with its run's id. A live run uses it up to `to`, any other run up to its end:
/// a run the daemon ended `orphaned` (at its stop, or at a restart that found it live) up to that mark, so the time
/// while no daemon read the pod counts against the cap, never less. The reads of the pod while a run lived
/// (`runbook.route-pod-phase`) cut its use.
fn pod_uses(st: &State, route: &str, from: i64, to: i64) -> Vec<(String, crate::pod_time::Uses)> {
    st.runs()
        .iter()
        .filter(|r| takes_route(r, route))
        .filter_map(|r| {
            let start = parse_instant(r.started_at.as_deref()?)?;
            if start >= to {
                return None;
            }
            let end = r.ended_at.as_deref().and_then(parse_instant).unwrap_or(to).clamp(start, to.max(start));
            if end <= from {
                return None;
            }
            let reads: Vec<(i64, bool)> = st.events_of(&r.run_id).filter(|e| e.kind == ROUTE_POD_PHASE && e.record["route"] == route).filter_map(|e| Some((parse_instant(&e.at)?, e.record["phase"] == "ready"))).collect();
            Some((r.run_id.clone(), crate::pod_time::uses_of(start, end, &reads)))
        })
        .collect()
}

/// What the pod's time of `route` at `price` per hour booked on `day` (the date, UTC) up to `now`: each run's share of
/// the time inside the day ([`crate::pod_time::shares`]) at the price, rounded up per run.
fn pod_booked(st: &State, route: &str, price: u64, day: &str, now: i64) -> u64 {
    let Some(from) = parse_instant(&format!("{day}T00:00:00Z")) else { return 0 };
    let to = from.saturating_add(86_400_000).min(now);
    if to <= from {
        return 0;
    }
    let uses: Vec<crate::pod_time::Uses> = pod_uses(st, route, from, to).into_iter().map(|(_, u)| u).collect();
    crate::pod_time::shares(&uses, from, to).into_iter().map(|millis| crate::pod_time::cost(millis, price)).fold(0u64, u64::saturating_add)
}

/// The day a wake books on: the date (UTC) of the daemon's clock, the day a route's daily caps count on too
/// ([`crate::route_day::day_of`]).
fn wake_day(st: &State) -> String {
    crate::route_day::day_of(st.now_millis())
}

/// What the daily cap of the route `name` counts on `day` up to `now`: what its wakes booked ([`wakes_booked`]) and, for a
/// priced managed route, what its pod's time booked ([`pod_booked`]). The cap of a wake ([`Daemon::wake_route`]) and the
/// view `routes` ([`wake_figures`]) read this one figure.
fn day_booked(st: &State, routes: &crate::routes::RouteTable, name: &str, day: &str, now: i64) -> (u64, Option<u64>) {
    let pod = pod_price(routes, name).map(|price| pod_booked(st, name, price, day, now));
    (wakes_booked(st, name, day), pod)
}

/// Whether the run, while it waits for its routes, waits on `route`: it takes the route ([`takes_route`]), or it is a
/// delegation the daemon classifies and `route` is the classifier route, which its wait reads with its own
/// ([`route_wait`] with `also`).
fn waits_on_route(routes: &crate::routes::RouteTable, run: &Run, route: &str) -> bool {
    let classifies = run.runbook_id == crate::delegation::CLASSIFY_RUNBOOK && run.model_route.is_some() && routes.classifier() == Some(route);
    takes_route(run, route) || classifies
}

/// The `at` of the newest answer `spend-ceiling` to a wake of `route` on `day`: a run event `runbook.route-wake-answered`,
/// or the daemon's line `route.wake-answered` of a run closed before the answer came.
fn ceiling_refused_at<'a>(st: &'a State, route: &str, day: &str) -> Option<&'a str> {
    let ceiling = |record: &Value| record["route"] == route && record["code"] == crate::routes::SPEND_CEILING;
    let events = st.wake_events_of(route).filter(|e| e.kind == ROUTE_WAKE_ANSWERED && ceiling(&e.record)).map(|e| e.at.as_str());
    let lines = st.wake_answers().iter().filter(|a| ceiling(&a.record)).map(|a| a.at.as_str());
    events.chain(lines).filter(|at| at.get(..10) == Some(day)).max()
}

/// The figures of the day of the daemon's clock of the route `name` for its row of the view `routes` (story
/// `the-routes-view-answers-the-days-wake-figures`; `phase`: the phase of the row, the last read of its gateway). For a
/// managed route that declares `wake_daily_cap_micro_usd`: `wake_booked_micro_usd`, `wake_cap_micro_usd`,
/// `wake_cost_micro_usd`, `wake_last_sent_at`, `wake_needed` and `ceiling_refused_at`; for one that declares
/// `price_per_hour_micro_usd`: `pod_booked_micro_usd`. Each is absent where it does not apply. The booked figures are
/// [`day_booked`], what the cap of the next wake reads. `wake_needed`: the wake is on, the phase is `stopped`, a run
/// waits on the route ([`waits_on_route`]), no wake of the route is out (not answered yet, its run live, also past the
/// route's wait) and none may be joined ([`wake_sent_within`]): a run waits
/// and the daemon sends no wake for it.
fn wake_figures(st: &State, routes: &crate::routes::RouteTable, name: &str, phase: Option<&str>) -> Map<String, Value> {
    let mut m = Map::new();
    let Some(route) = routes.route(name).filter(|r| r.kind == crate::routes::RouteKind::Managed) else { return m };
    let (now, day) = (st.now_millis(), wake_day(st));
    let (wakes, pod) = day_booked(st, routes, name, &day, now);
    if let Some(cap) = route.wake.daily_cap_micro_usd {
        m.insert("wake_booked_micro_usd".into(), json!(wakes));
        m.insert("wake_cap_micro_usd".into(), json!(cap));
        if let Some(cost) = route.wake.cost_micro_usd {
            m.insert("wake_cost_micro_usd".into(), json!(cost));
        }
        if let Some(last) = sent_wakes(st, name).last() {
            m.insert("wake_last_sent_at".into(), json!(last.at));
        }
        let waits = || st.runs().iter().any(|r| r.state == RunState::WaitingForRoute && waits_on_route(routes, r, name));
        let wait_millis = i64::try_from(route.start_wait_seconds.unwrap_or(0)).unwrap_or(i64::MAX / 2).saturating_mul(1000);
        // a wake not answered yet whose run lives is out, also past the route's wait (the daemon waits up to 5 s for an answer)
        let out = || sent_wakes(st, name).iter().any(|w| w.answer.is_none() && run_lives(st, w.run_id));
        let needed = route.wake.on_submit && phase == Some("stopped") && waits() && !out() && wake_sent_within(st, name, now, wait_millis).is_none();
        m.insert("wake_needed".into(), json!(needed));
        if let Some(at) = ceiling_refused_at(st, name, &day) {
            m.insert("ceiling_refused_at".into(), json!(at));
        }
    }
    if let Some(pod) = pod {
        m.insert("pod_booked_micro_usd".into(), json!(pod));
    }
    m
}

/// Writes the run's share of the pod's time of each priced managed route it takes, up to `until` (its end), in the run
/// event `runbook.route-pod-time`, just before it ends. A run that never started took no time.
fn record_pod_time_locked(st: &mut State, routes: &crate::routes::RouteTable, run_id: &str, until: i64) {
    let Some(run) = st.run(run_id) else { return };
    let Some(start) = run.started_at.as_deref().and_then(parse_instant) else { return };
    let Some(record) = run.model_route.clone() else { return };
    let mut names: Vec<String> = std::iter::once(record.route).chain(record.roles.into_values()).collect();
    names.sort();
    names.dedup();
    for name in names {
        let Some(price) = pod_price(routes, &name) else { continue };
        let uses = pod_uses(st, &name, start, until);
        let Some(at) = uses.iter().position(|(r, _)| r == run_id) else { continue };
        let uses: Vec<crate::pod_time::Uses> = uses.into_iter().map(|(_, u)| u).collect();
        let share = crate::pod_time::shares(&uses, start, until)[at];
        let pod = crate::pod_time::length(&uses[at], start, until);
        let record = json!({"route": name, "pod_millis": pod, "share_millis": share, "price_per_hour_micro_usd": price, "cost_micro_usd": crate::pod_time::cost(share, price)});
        let _ = record_event_locked(st, run_id, ROUTE_POD_TIME, record);
    }
}

/// The thread that reads the pods of the priced managed routes while runs use them ([`Daemon::read_pods`]), until the
/// daemon stops or is gone: it holds the daemon only for one look, and looks at most once a second of the daemon's
/// clock (a look walks the runs).
fn watch_pods(inner: &std::sync::Weak<Inner>) {
    let mut read_at: HashMap<String, i64> = HashMap::new();
    let mut looked_at: Option<i64> = None;
    loop {
        std::thread::sleep(std::time::Duration::from_millis(20));
        let Some(inner) = inner.upgrade() else { return };
        let d = Daemon { inner };
        if d.is_stopping() {
            return;
        }
        let now = d.st().now_millis();
        if looked_at.is_some_and(|at| now >= at && now < at.saturating_add(1_000)) {
            continue;
        }
        looked_at = Some(now);
        d.read_pods(&mut read_at);
    }
}

/// Why a route of the run cannot take it at the submit.
enum NotReady {
    /// The gateway is not ready or does not answer: `503 route-unavailable`.
    Unavailable(String),
    /// The gateway is ready and does not list the route's alias: `route-refused`.
    Refused(String),
}

/// The declared routes of the run's record: its route and the route of each role, each once, the interim route of a
/// lone `[gateway]` left out (it is taken as before routes).
fn declared_routes_of(routes: &crate::routes::RouteTable, record: &RunRoute) -> Vec<String> {
    let mut names: Vec<String> = std::iter::once(record.route.clone()).chain(record.roles.values().cloned()).collect();
    names.sort();
    names.dedup();
    names.retain(|n| !routes.is_interim(n));
    names
}

/// Whether the declared routes of `record` can take a run now (`docs/model-routes.md`, section 3), read before anything
/// is written, with no call and no wake: every one can answer (`Ok(None)`), a model cannot answer yet (`Ok(Some(wait))`, the
/// wait up to the route's `start_wait_seconds`), or a route cannot take the run at all. A submit and a delegation ask it;
/// `also` is a route the run calls besides its declared ones (the classifier route of a delegation the run classifies).
fn route_wait(routes: &crate::routes::RouteTable, record: &RunRoute, also: Option<&str>) -> Result<Option<RouteWait>, NotReady> {
    let mut names = declared_routes_of(routes, record);
    if let Some(name) = also.filter(|n| !routes.is_interim(n) && !names.iter().any(|m| m == n)) {
        names.push(name.to_owned());
        names.sort();
    }
    let waiting = not_callable(routes, &names)?;
    let wake: Vec<(String, Option<String>)> = waiting
        .iter()
        .filter(|(name, phase)| phase.as_deref() == Some("stopped") && routes.route(name).is_some_and(|r| r.kind == crate::routes::RouteKind::Managed && r.wake.on_submit))
        .cloned()
        .collect();
    Ok(least_wait(routes, waiting).map(|(route, phase)| {
        let seconds = routes.route(&route).and_then(|r| r.start_wait_seconds).unwrap_or(0);
        RouteWait { routes: names, route, phase, wait_millis: i64::try_from(seconds).unwrap_or(i64::MAX / 2).saturating_mul(1000), wake }
    }))
}

/// What the gateways say of `names` now (`docs/model-routes.md`, section 3): every route can answer (`Ok(None)`), some
/// cannot yet (`Ok(Some(wait))`: of those, the one with the least `start_wait_seconds`, the first of equals, with its
/// phase; its wait is the run's), or a route cannot take the run at all.
fn read_routes(routes: &crate::routes::RouteTable, names: &[String]) -> Result<Option<(String, Option<String>)>, NotReady> {
    Ok(least_wait(routes, not_callable(routes, names)?))
}

/// The routes of `names` that cannot answer yet, in order, each with the phase its gateway gave; an error when a route
/// cannot take the run at all.
fn not_callable(routes: &crate::routes::RouteTable, names: &[String]) -> Result<Vec<(String, Option<String>)>, NotReady> {
    use crate::routes::Readiness;
    let until = std::time::Instant::now() + crate::routes::READ_BOUND;
    let mut waiting = Vec::new();
    for name in names {
        let Some(route) = routes.route(name) else { continue };
        match routes.readiness(name, until) {
            Some(Readiness::Callable) | None => {}
            Some(Readiness::Unreachable) => return Err(NotReady::Unavailable(format!("route {name}: the gateway {} does not answer", route.gateway))),
            Some(Readiness::NotReady) => return Err(NotReady::Unavailable(format!("route {name}: gateway not ready (the gateway {} answered GET /ready, not 200)", route.gateway))),
            Some(Readiness::Unlisted) => return Err(NotReady::Refused(format!("route {name}: the gateway {} serves no model {}", route.gateway, route.alias))),
            Some(Readiness::NotCallable { phase }) => waiting.push((name.clone(), phase)),
        }
    }
    Ok(waiting)
}

/// Of the routes that cannot answer yet, the one with the least `start_wait_seconds` (the first of equals), with its phase.
fn least_wait(routes: &crate::routes::RouteTable, waiting: Vec<(String, Option<String>)>) -> Option<(String, Option<String>)> {
    let mut least: Option<(u64, String, Option<String>)> = None;
    for (name, phase) in waiting {
        let seconds = routes.route(&name).and_then(|r| r.start_wait_seconds).unwrap_or(0);
        if least.as_ref().is_none_or(|(s, _, _)| seconds < *s) {
            least = Some((seconds, name, phase));
        }
    }
    least.map(|(_, name, phase)| (name, phase))
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CancelRun {
    pub run_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AnswerDecision {
    pub run_id: String,
    pub request_id: String,
    pub answer: String,
    pub answered_by: String,
}

// ---- the daemon ----

/// The step of a live run whose program is running now. It lives in the process only: the event
/// `runbook.action-started` that told of its start is in the journal, a restarted daemon has no step
/// in flight and its runs are ended as orphaned.
struct Step {
    serial: u64,
    case: Option<String>,
    action: Option<String>,
    /// The `at` of the event that recorded the start.
    started_at: String,
}

struct Inner {
    cfg: Config,
    policy: Policy,
    daemon_id: String,
    state: Mutex<State>,
    runtime: Arc<dyn Runtime>,
    cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
    steps: Mutex<HashMap<String, Step>>,
    threads: Mutex<Vec<JoinHandle<()>>>,
    stopping: AtomicBool,
    /// The view `routes` over `policy.routes`, with its last read of the gateways.
    routes: crate::routes::Listing,
    /// The day's figures of the declared routes: what the relay booked and sent on each per day of the daemon's clock,
    /// which its daily caps are held against (`crate::route_day`).
    days: Arc<crate::route_day::RouteDays>,
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

/// A serial key is 1 to `MAX_LABEL_CHARS` characters; another is refused before anything is written.
fn check_serial_key(key: Option<&str>) -> Result<(), Fault> {
    match key {
        Some("") => Err(Fault::Invalid("serial_key is empty: name a key of 1 to 200 characters, or none".to_owned())),
        Some(k) if !long_enough(k) => Err(Fault::Invalid(format!("serial_key is longer than {MAX_LABEL_CHARS} characters"))),
        _ => Ok(()),
    }
}

impl Daemon {
    /// Open the state directory, record the configuration row (`StartDaemon`) and deal with the runs
    /// whose runtime is gone because the daemon stopped.
    pub fn start(cfg: Config, runtime: Arc<dyn Runtime>) -> io::Result<Daemon> {
        Daemon::start_with_clock(cfg, runtime, Arc::new(SystemClock))
    }

    /// `start` with the clock every recorded instant is read from. For tests that build the daemon in
    /// process (`clock::SetClock`); `runbook serve` calls `start`, and no option, environment variable
    /// or request reaches this.
    pub fn start_with_clock(cfg: Config, runtime: Arc<dyn Runtime>, clock: Arc<dyn Clock>) -> io::Result<Daemon> {
        Daemon::start_full(cfg, Policy::default(), runtime, clock)
    }

    /// `start` with the operator's policy.
    pub fn start_with(cfg: Config, policy: Policy, runtime: Arc<dyn Runtime>) -> io::Result<Daemon> {
        Daemon::start_full(cfg, policy, runtime, Arc::new(SystemClock))
    }

    /// Everything `start` takes. A default level below the floor is refused before the state directory is opened.
    pub fn start_full(cfg: Config, policy: Policy, runtime: Arc<dyn Runtime>, clock: Arc<dyn Clock>) -> io::Result<Daemon> {
        check_principal(&cfg.principal).map_err(io::Error::other)?;
        if let Some(default) = policy.default_confinement {
            check_default_confinement(cfg.confinement_floor, default).map_err(io::Error::other)?;
        }
        let defaults = Terms { autonomy: policy.autonomy, decider: policy.decider.clone() };
        check_terms(&defaults, &policy.kinds).map_err(|e| io::Error::other(format!("the daemon's default decider: {e}")))?;
        let mut roots = Vec::new();
        for r in &cfg.context_roots {
            let canon = std::fs::canonicalize(r).map_err(|e| io::Error::new(e.kind(), format!("context root {}: {e}", r.display())))?;
            if !canon.is_dir() {
                return Err(io::Error::other(format!("context root {} is not a directory", r.display())));
            }
            roots.push(canon);
        }
        let state_dir = std::path::absolute(&cfg.state_dir).map_err(|_| io::Error::other("the state directory cannot be made absolute: the daemon has no current directory"))?;
        let cfg = Config { context_roots: roots, state_dir, ..cfg };
        let state = State::open_with_clock(&cfg.state_dir, clock.clone())?;
        let routes = crate::routes::Listing::new(policy.routes.clone());
        // the figures of the routes' days are rebuilt from the spend ledgers of the daemon's runs (`engine::Gate::open`)
        let days = Arc::new(crate::route_day::RouteDays::open(&policy.routes, clock, &cfg.state_dir.join("ledgers")));
        let d = Daemon {
            inner: Arc::new(Inner {
                daemon_id: uuid::Uuid::now_v7().to_string(),
                cfg,
                policy,
                routes,
                days,
                state: Mutex::new(state),
                runtime,
                cancels: Mutex::new(HashMap::new()),
                steps: Mutex::new(HashMap::new()),
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
        d.requeue();
        // the pods of the priced managed routes are read while runs use them
        if d.inner.policy.routes.routes().any(|r| pod_price(&d.inner.policy.routes, &r.name).is_some()) {
            let inner = Arc::downgrade(&d.inner);
            let handle = std::thread::Builder::new().name("pod-reads".into()).spawn(move || watch_pods(&inner))?;
            lock(&d.inner.threads).push(handle);
        }
        Ok(d)
    }

    /// One look at the pods of the priced managed routes (`docs/daemon-api.md`, "The pod's time of a managed route"): a
    /// route that a started, live run takes is read every [`ROUTE_READ_EVERY_MILLIS`] of the daemon's clock, the first
    /// time that long after the earliest start of those runs, and a read that changes whether a run has the pod (ready
    /// or not) is that run's event `runbook.route-pod-phase`. A read that gives no status of the model changes nothing.
    fn read_pods(&self, read_at: &mut HashMap<String, i64>) {
        let routes = &self.inner.policy.routes;
        for route in routes.routes().filter(|r| pod_price(routes, &r.name).is_some()) {
            let (now, users) = {
                let st = self.st();
                // each live run that started on the route, its start, and whether its last read said the pod is ready
                let users: Vec<(String, i64, bool)> = st
                    .runs()
                    .iter()
                    .filter(|r| r.state.is_live() && takes_route(r, &route.name))
                    .filter_map(|r| {
                        let start = parse_instant(r.started_at.as_deref()?)?;
                        let ready = st.events_of(&r.run_id).filter(|e| e.kind == ROUTE_POD_PHASE && e.record["route"] == route.name.as_str()).last().is_none_or(|e| e.record["phase"] == "ready");
                        Some((r.run_id.clone(), start, ready))
                    })
                    .collect();
                (st.now_millis(), users)
            };
            let Some(first) = users.iter().map(|u| u.1).min() else { continue };
            let since = read_at.get(&route.name).copied().unwrap_or(first).max(first);
            if now < since.saturating_add(ROUTE_READ_EVERY_MILLIS) {
                continue;
            }
            read_at.insert(route.name.clone(), now);
            // a phase only from a status the gateway answered: no answer, a gateway not ready, an alias it no longer lists
            // or a status route that failed says nothing of the pod
            let Some(phase) = routes.pod_phase(&route.name, std::time::Instant::now() + crate::routes::READ_BOUND) else { continue };
            let ready = phase == "ready";
            let mut st = self.st();
            for (run, _, _) in users.into_iter().filter(|u| u.2 != ready) {
                // a run that ended since takes no event
                let _ = record_event_locked(&mut st, &run, ROUTE_POD_PHASE, json!({"route": route.name, "phase": phase}));
            }
        }
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

    /// Whether the daemon got its stop signal and refuses a submit.
    pub fn is_draining(&self) -> bool {
        self.st().draining()
    }

    /// The ids of the runs that are not terminal.
    pub fn live_runs(&self) -> Vec<String> {
        self.st().live_run_ids()
    }

    /// A receiver whose value changes whenever the state does (`State::subscribe`).
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<u64> {
        self.st().subscribe()
    }

    /// Begin the drain on the stop signal: from now a submit is refused (`Fault::Draining`); the runs go
    /// on, reads and decisions are served. The journal records `daemon.draining` with the ids of the live
    /// runs, and they are returned. The refusal and the flag are decided under the one lock a submit takes,
    /// so no run is submitted after the ids are read. A second call changes nothing and returns the live runs.
    pub fn begin_drain(&self) -> Result<Vec<String>, Fault> {
        let mut st = self.st();
        let run_ids = st.live_run_ids();
        if !st.draining() {
            let at = st.now();
            st.commit(Fact::DaemonDraining { run_ids: run_ids.clone(), at })?;
        }
        Ok(run_ids)
    }

    /// The drain ended because no run is live: the journal records `daemon.drained`.
    pub fn finish_drain(&self) -> Result<(), Fault> {
        let mut st = self.st();
        let at = st.now();
        st.commit(Fact::DaemonDrained { at })?;
        Ok(())
    }

    /// The daemon ends now with runs still live (a second stop signal, or the drain bound): it stops
    /// reporting for them, and each run that is `Running` or `WaitingOnDecision` ends as `orphaned` with an
    /// event that says the daemon was stopped while it ran, and its Open decision requests are withdrawn; a run that is
    /// `WaitingForRoute` never started and fails to start (`runbook.start-failed`, [`ROUTE_WAIT_AT_STOP`]).
    /// A run that is `WaitingOnKey` keeps its place: the queue of its key holds across the restart (`requeue`).
    /// No thread is joined: `stop` does that. The number of runs that were live and are ended now.
    pub fn orphan_live_runs(&self) -> usize {
        self.inner.stopping.store(true, Ordering::SeqCst);
        let live: Vec<String> = {
            let st = self.st();
            st.live_run_ids().into_iter().filter(|id| st.run(id).is_some_and(|r| r.state != RunState::WaitingOnKey)).collect()
        };
        for run_id in &live {
            // a run that waits for its route never started: it fails to start, as after a restart (`mark_orphans`)
            let waiting = self.st().run(run_id).is_some_and(|r| r.state == RunState::WaitingForRoute);
            if waiting {
                let _ = fail_run_start_locked(&mut self.st(), run_id, ROUTE_WAIT_AT_STOP);
            } else {
                let _ = self.end_orphaned_run_because(run_id, ORPHANED_AT_STOP);
            }
        }
        live.len()
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
            } else if state == RunState::WaitingForRoute {
                fail_run_start_locked(&mut self.st(), &run_id, ROUTE_WAIT_AT_STOP)?;
            } else if state == RunState::WaitingOnKey {
                // the queue of its key holds across the restart: `requeue` takes it up again
            } else if self.inner.runtime.recover(&run_id) == Recovery::Gone {
                self.end_orphaned_run(&run_id)?;
            }
        }
        Ok(())
    }

    /// `EndOrphanedRun`, issued by the daemon itself (never by a client): the run ends with the outcome
    /// `orphaned`, then `WithdrawDecision` is issued for each of its Open requests, as a cancel does.
    pub fn end_orphaned_run(&self, run_id: &str) -> Reply {
        self.end_orphaned_run_because(run_id, ORPHANED_AT_START)
    }

    fn end_orphaned_run_because(&self, run_id: &str, reason: &str) -> Reply {
        let mut st = self.st();
        let out = end_orphaned_run_locked(&mut st, &self.inner.policy.routes, run_id, reason)?;
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
        let now = st.now();
        st.commit(Fact::SessionOpened { session_id: input.session_id.clone(), client: input.client.clone(), at: now })?;
        Ok(done("open-session", "opened", vec![("SessionOpened", json!({"session_id": input.session_id, "client": input.client}))]))
    }

    pub fn close_session(&self, input: CloseSession) -> Reply {
        let mut st = self.st();
        let id = input.session_id;
        match st.session(&id).map(|s| s.state) {
            None => Ok(refuse("close-session", "no-session", json!({"session_id": id}))),
            Some(SessionState::Closed) => Ok(refuse("close-session", "wrong-state", json!({"session_id": id}))),
            Some(SessionState::Open) => {
                let now = st.now();
                st.commit(Fact::SessionClosed { session_id: id.clone(), at: now })?;
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
        let now = st.now();
        let principal = self.inner.cfg.principal.clone();
        st.commit(Fact::DaemonStarted(DaemonRow { daemon_id: daemon_id.to_string(), confinement_floor, principal, started_at: now }))?;
        Ok(done("start-daemon", "started", vec![("DaemonStarted", json!({"daemon_id": daemon_id, "confinement_floor": confinement_floor}))]))
    }

    pub fn submit_run(&self, input: SubmitRun) -> Reply {
        let id = self.inner.daemon_id.clone();
        self.submit_run_for(&id, input)
    }

    /// `SubmitRun` as the daemon `daemon_id` names (the HTTP handler always names its own).
    pub fn submit_run_for(&self, daemon_id: &str, input: SubmitRun) -> Reply {
        if let Some(name) = input.settings.keys().find_map(|k| gateway_name(k)) {
            return Err(gateway_refusal("the setting", &name));
        }
        check_serial_key(input.serial_key.as_deref())?;
        if input.idempotency_key.as_deref().is_some_and(|k| !long_enough(k)) {
            return Err(Fault::Invalid(format!("idempotency_key is longer than {MAX_LABEL_CHARS} characters")));
        }
        check_workspace_exclude(&input.workspace_exclude)?;
        draining_fault(&self.st())?;
        // The level and the kind of the run: the submit's, else the daemon's. Refused before anything is written.
        let terms = Terms { autonomy: input.autonomy.or(self.inner.policy.autonomy), decider: input.decider.clone().or_else(|| self.inner.policy.decider.clone()) };
        check_terms(&terms, &self.inner.policy.kinds).map_err(Fault::Invalid)?;
        let run_id = uuid::Uuid::now_v7().to_string();
        let (confinement, source) = match (input.confinement, self.inner.policy.default_confinement) {
            (Some(named), _) => (named, ConfinementSource::Submit),
            (None, Some(default)) => (default, ConfinementSource::DaemonDefault),
            (None, None) => return Err(Fault::Invalid("confinement: name it in the submit, or start the daemon with --default-confinement or a `confinement` in runbook.toml".to_owned())),
        };
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
        // The run's model route: the submit's word, then the runbook's route entries, then the daemon's default; a refusal
        // lists the declared routes.
        let programs = self.inner.runtime.program_steps(&input.runbook_id, input.runbook_version.as_deref());
        let route = self.protocol_word(&input.runbook_id, input.runbook_version.as_deref()).and_then(|word| route_of_run(&self.inner.policy, &input, &terms, programs, self.inner.runtime.serves_routes(), word.as_ref()));
        let route = match route {
            Ok(route) => route,
            Err(reason) => return refused("route-refused", json!({"run_id": run_id, "reason": reason, "declared": self.inner.policy.routes.names()})),
        };
        // A run that makes no model call takes no route, also the one its submit names: its first event says so.
        let route_not_taken = route.is_none().then(|| route_not_taken(&input, &terms, programs)).flatten();
        // Can the routes take the run (docs/model-routes.md, section 3)? Read before anything is written; no call, no
        // wake. A gateway not ready is 503, an alias it does not list route-refused, a model not callable a wait. A replay
        // of an idempotency key is not asked about: it answers the run that exists (below), as a delegation's does.
        let replay = input.idempotency_key.as_deref().is_some_and(|k| self.st().run_by_key(k).is_some());
        let wait = match &route {
            None => None,
            Some(_) if replay => None,
            Some((record, _)) => {
                let routes = &self.inner.policy.routes;
                match route_wait(routes, record, None) {
                    Ok(wait) => wait,
                    Err(NotReady::Unavailable(message)) => return Err(Fault::Refused { status: 503, what: "route-unavailable", message }),
                    Err(NotReady::Refused(reason)) => return refused("route-refused", json!({"run_id": run_id, "reason": reason, "declared": routes.names()})),
                }
            }
        };
        let context_dir = context.flatten();
        // The caps of the run: what the submit names within the runtime's ceilings, else its defaults.
        // They are part of the run's settings from here on, so a run taken up again has the same ones.
        let mut settings = input.settings.clone();
        match self.inner.runtime.resolve_caps(&settings) {
            Ok(Some(caps)) => caps.put(&mut settings),
            Ok(None) => {}
            Err(why) => return Err(Fault::Invalid(why)),
        }
        // What the runtime refuses a run of the runbook for at load (a run never unbound in money).
        self.inner.runtime.check_runbook(&input.runbook_id, input.runbook_version.as_deref()).map_err(Fault::Invalid)?;

        let st = self.st();
        draining_fault(&st)?;
        let floor = match st.daemon() {
            Some(d) if d.daemon_id == daemon_id => d.confinement_floor,
            _ => return refused("no-daemon", json!({"daemon_id": daemon_id})),
        };
        let session = input.session_id.as_ref().map(|sid| (sid, st.session(sid).map(|s| s.state)));
        if let Some((sid, None)) = &session {
            return refused("no-session", json!({"session_id": sid}));
        }
        if !confinement.at_least(floor) {
            return refused("below-floor", json!({"run_id": run_id, "confinement": confinement}));
        }
        if let Some((sid, Some(SessionState::Closed))) = &session {
            return refused("session-closed", json!({"run_id": run_id, "session_id": sid}));
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

        let payload = json!({"run_id": run_id, "runbook_id": input.runbook_id, "confinement": confinement});
        let admitted = Admitted {
            run_id: run_id.clone(),
            runbook_id: input.runbook_id,
            runbook_version: input.runbook_version,
            intake: input.intake,
            context_path: input.context_path,
            context_dir,
            workspace_exclude: input.workspace_exclude,
            settings,
            confinement,
            source,
            terms,
            session_id: input.session_id,
            idempotency_key: input.idempotency_key,
            serial_key: input.serial_key,
            route,
            route_not_taken,
            delegation: None,
            wait,
        };
        self.enter(st, admitted, None)?;
        let mut out = done("submit-run", "submitted", vec![("RunSubmitted", payload)]);
        out.run_id = Some(run_id);
        Ok(out)
    }

    /// A run that passed every guard of its command, under the lock those guards were read under: it is written (`Submitted`, with
    /// its first event `runbook.submitted`, and for a delegation its second, `runbook.delegated` with `delegated` as its record) and
    /// its thread is started.
    fn enter(&self, mut st: MutexGuard<'_, State>, a: Admitted, delegated: Option<Value>) -> Result<(), Fault> {
        let run_id = a.run_id;
        let (run_dir, workspace) = (st.run_dir(&run_id), st.workspace_dir(&run_id));
        // The run's directory holds its copy of the runbook and its store: private to the daemon's user.
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&run_dir)?;
        std::fs::create_dir_all(&workspace)?;
        let run = Run {
            run_id: run_id.clone(),
            runbook_id: a.runbook_id.clone(),
            runbook_version: a.runbook_version.clone(),
            intake: a.intake.clone(),
            context_path: a.context_path,
            settings: a.settings.clone(),
            confinement: a.confinement,
            confinement_source: Some(a.source),
            terms: a.terms.clone(),
            model_route: a.route.as_ref().map(|(record, _)| record.clone()),
            route_phase: None,
            session_id: a.session_id,
            idempotency_key: a.idempotency_key,
            serial_key: a.serial_key.clone(),
            submitted_at: st.now(),
            started_at: None,
            ended_at: None,
            outcome: None,
            failure: None,
            state: RunState::Submitted,
        };
        st.commit(Fact::RunSubmitted(run))?;
        let mut record = json!({"run_id": run_id, "runbook_id": a.runbook_id, "confinement": a.confinement, "confinement_source": a.source.as_str()});
        if let Some(level) = a.terms.autonomy {
            record["autonomy"] = json!(level);
        }
        if let Some(kind) = &a.terms.decider {
            record["decider"] = json!(kind);
        }
        if let Some(key) = &a.serial_key {
            record["serial_key"] = json!(key);
        }
        if let Some((r, _)) = &a.route {
            record["model_route"] = json!(r.route);
            record["model_route_alias"] = json!(r.alias);
            record["model_route_source"] = json!(r.source);
        }
        if let Some(not_taken) = a.route_not_taken {
            record["model_route_not_taken"] = not_taken;
        }
        // what the run adds to the daemon's exclude list: the copy reads it from here when the run starts, also after a restart
        if !a.workspace_exclude.is_empty() {
            record[WORKSPACE_EXCLUDE] = json!(a.workspace_exclude);
        }
        record_event_locked(&mut st, &run_id, "runbook.submitted", record)?;
        if let Some(d) = delegated {
            record_event_locked(&mut st, &run_id, crate::delegation::DELEGATED_EVENT, d)?;
        }
        drop(st);

        // A delegation the run classifies takes the route entries of the runbook the classifier picks as one that names it
        // does (`route_of_delegation`): the route the delegation named stays the run's word. Its routes pass the readiness
        // check of a run's before the runbook's first call, and the run's record names them.
        let admit = (a.delegation.is_some() && a.runbook_id == crate::delegation::CLASSIFY_RUNBOOK).then(|| self.admit_of(&run_id, a.route.as_ref().map(|(record, _)| record)));
        let request = StartRequest {
            run_id,
            runbook: a.runbook_id,
            version: a.runbook_version,
            intake: a.intake,
            context_dir: a.context_dir,
            settings: a.settings,
            confinement: a.confinement,
            terms: a.terms,
            state_dir: run_dir,
            workspace_dir: workspace,
            routes: a.route.map(|(_, table)| table),
            model_route: a.delegation,
            admit,
            worktree: None,
        };
        self.spawn_run(request, a.wait);
        Ok(())
    }

    /// The admission of the route entries of the runbook the classifier picks for the delegation `run_id` (`crate::runtime::Admit`):
    /// the route the delegation named (its record's route when the run chose it) stays the run's word.
    fn admit_of(&self, run_id: &str, record: Option<&RunRoute>) -> crate::runtime::Admit {
        // the run holds the daemon weakly: a run that outlives it gets a refusal, and no cycle keeps the daemon alive
        let inner = Arc::downgrade(&self.inner);
        let named = record.filter(|record| record.source == RouteSource::Run).map(|record| record.route.clone());
        let run = run_id.to_owned();
        crate::runtime::Admit(Arc::new(move |runbook: &str| {
            let refused = |why: String| format!("route-refused: {why}");
            let daemon = Daemon { inner: inner.upgrade().ok_or_else(|| refused("the daemon stopped before the run took its runbook's routes".to_owned()))? };
            let word = daemon.protocol_word(runbook, None).map_err(refused)?;
            let (record, table) = route_of_delegation(&daemon.inner.policy, named.as_deref(), word.as_ref()).map_err(refused)?;
            daemon.ready_routes(&run, &record)?;
            daemon.st().commit(Fact::RunRouted { run_id: run.clone(), model_route: record }).map_err(|e| refused(format!("the run's record could not take its routes: {e}")))?;
            Ok(table)
        }))
    }

    /// The runs the journal holds as WaitingOnKey when the daemon starts: the queue of each key is the journal's order, so
    /// each is taken up again (its own thread waits for its key) in that order. A run whose context or routes the daemon
    /// refuses now fails to start, with the reason, and frees its place.
    fn requeue(&self) {
        let waiting: Vec<String> = self.st().runs().iter().filter(|r| r.state == RunState::WaitingOnKey).map(|r| r.run_id.clone()).collect();
        for run_id in waiting {
            match self.request_of(&run_id) {
                Ok(request) => self.spawn_run(request, None),
                Err(reason) => {
                    let _ = self.fail_run_start(&run_id, &reason);
                }
            }
        }
    }

    /// The start request of a run from its record, as `enter` made it at the admission: the context is resolved under the
    /// daemon's roots again, the routes against the routes it declares now (`table_of_record`), and a delegation's route
    /// is the one its event `runbook.delegated` names.
    fn request_of(&self, run_id: &str) -> Result<StartRequest, String> {
        let st = self.st();
        let r = st.run(run_id).cloned().ok_or_else(|| format!("no run {run_id}"))?;
        let delegated = st.events_after(Some(run_id), None, 0).iter().find(|e| e.kind == crate::delegation::DELEGATED_EVENT).map(|e| e.record.get("route").and_then(Value::as_str).unwrap_or_default().to_owned());
        let (state_dir, workspace_dir) = (st.run_dir(run_id), st.workspace_dir(run_id));
        drop(st);
        let context_dir = match &r.context_path {
            None => None,
            Some(p) => Some(self.context_under_roots(p).ok_or_else(|| "the run's context lies under no context root of the daemon now".to_owned())?),
        };
        let routes = match &r.model_route {
            None => None,
            Some(record) => Some(table_of_record(&self.inner.policy, record).map_err(|why| format!("route-refused: {why}"))?),
        };
        let admit = (delegated.is_some() && r.runbook_id == crate::delegation::CLASSIFY_RUNBOOK).then(|| self.admit_of(run_id, r.model_route.as_ref()));
        Ok(StartRequest {
            run_id: r.run_id,
            runbook: r.runbook_id,
            version: r.runbook_version,
            intake: r.intake,
            context_dir,
            settings: r.settings,
            confinement: r.confinement,
            terms: r.terms,
            state_dir,
            workspace_dir,
            routes,
            model_route: delegated,
            admit,
            worktree: None,
        })
    }

    /// `Delegate` (`docs/model-routes.md`, section 8): a piece of agent work on a named route, as a run. The guards run in the
    /// specification's order; the gateway of the route is asked whether it is ready after the guards that need no lock and before
    /// anything is written (a replay of an idempotency key is not asked about).
    pub fn delegate(&self, input: Delegate) -> Reply {
        let id = self.inner.daemon_id.clone();
        self.delegate_for(&id, input)
    }

    /// `Delegate` as the daemon `daemon_id` names (the HTTP handler always names its own).
    pub fn delegate_for(&self, daemon_id: &str, input: Delegate) -> Reply {
        let daemon_id = daemon_id.to_owned();
        if let Some(name) = input.settings.keys().find_map(|k| gateway_name(k)) {
            return Err(gateway_refusal("the setting", &name));
        }
        check_serial_key(input.serial_key.as_deref())?;
        if input.idempotency_key.as_deref().is_some_and(|k| !long_enough(k)) {
            return Err(Fault::Invalid(format!("idempotency_key is longer than {MAX_LABEL_CHARS} characters")));
        }
        for (member, value) in [("caller", &input.caller), ("caller_label", &input.caller_label)] {
            if value.as_deref().is_some_and(|v| !long_enough(v)) {
                return Err(Fault::Invalid(format!("{member} is longer than {MAX_LABEL_CHARS} characters")));
            }
        }
        check_workspace_exclude(&input.workspace_exclude)?;
        draining_fault(&self.st())?;
        let terms = Terms { autonomy: self.inner.policy.autonomy, decider: self.inner.policy.decider.clone() };
        check_terms(&terms, &self.inner.policy.kinds).map_err(Fault::Invalid)?;
        let confinement = self.inner.policy.default_confinement.ok_or_else(|| {
            Fault::Invalid("a delegation runs at the daemon's default confinement level and this daemon has none: start it with --default-confinement or a `confinement` in runbook.toml".to_owned())
        })?;
        let run_id = uuid::Uuid::now_v7().to_string();
        let refused = |name: &'static str, payload: Value| -> Reply { Ok(refuse("delegate", name, payload)) };
        // The path (docs/model-routes.md, section 9): a named runbook; the free run, asked for by its name or taken by a delegation
        // that names none on a daemon without a classifier route; else none yet: the run classifies the request when it starts.
        use crate::delegation::{CHANGE_RUNBOOK, CLASSIFY_RUNBOOK, FREE_RUNBOOK};
        // A delegation that changes a repository (`crate::handback`) names an entry of runbook.toml and runs the daemon's
        // protocol delegation-change at the level local, from the entry's base resolved to a commit now.
        let change = input.repository.as_deref().map(|named| self.repository_for(named, input.protocol.as_deref(), confinement));
        let (runbook_id, path) = match input.protocol.as_deref() {
            _ if change.is_some() => (CHANGE_RUNBOOK.to_owned(), Some("change")),
            Some(FREE_RUNBOOK) => (FREE_RUNBOOK.to_owned(), Some("free")),
            Some(p) => (p.to_owned(), Some("named")),
            None if self.inner.runtime.classifies() => (CLASSIFY_RUNBOOK.to_owned(), None),
            None => (FREE_RUNBOOK.to_owned(), Some("free")),
        };
        let runbook_known = match (path, input.protocol.as_deref()) {
            (Some("named"), Some(p)) => self.runbook_known(p, None),
            // the runtime of a change is checked by `repository_for`, which names why it refuses
            (Some("change"), _) => true,
            (Some(_), _) => self.inner.runtime.free_run(),
            (None, _) => true,
        };
        let context = input.workspace.as_deref().map(|p| self.context_under_roots(p));
        // The facts the specification takes from the host. The delegation's route is chosen as a run's (`route_of_delegation`): the
        // one it names, then the route entries of the runbook it names, else the daemon's `route_policy.default`; a route that is
        // not declared, or none, is refused.
        let word = match (path, input.protocol.as_deref()) {
            (Some("named"), Some(p)) if runbook_known => self.protocol_word(p, None),
            _ => Ok(None),
        };
        let route = word.and_then(|word| route_of_delegation(&self.inner.policy, input.model_route.as_deref(), word.as_ref()));

        if input.request.len() > MAX_INTAKE_BYTES {
            return refused("intake-too-large", json!({"run_id": run_id}));
        }
        let (record, table) = match route {
            Ok(route) => route,
            Err(reason) => return refused("route-refused", json!({"run_id": run_id, "reason": reason, "declared": self.inner.policy.routes.names()})),
        };
        let route = record.route.clone();
        if !runbook_known {
            return refused("no-runbook", json!({"run_id": run_id}));
        }
        if matches!(context, Some(None)) {
            return refused("context-outside-roots", json!({"run_id": run_id}));
        }
        let change = match change {
            None => None,
            Some(Ok(change)) => Some(change),
            Some(Err(reason)) => return refused("repository-refused", json!({"run_id": run_id, "reason": reason})),
        };
        let context_dir = context.flatten();
        let mut settings = input.settings.clone();
        match self.inner.runtime.resolve_caps(&settings) {
            Ok(Some(caps)) => caps.put(&mut settings),
            Ok(None) => {}
            Err(why) => return Err(Fault::Invalid(why)),
        }
        if path == Some("named") {
            self.inner.runtime.check_runbook(&runbook_id, None).map_err(Fault::Invalid)?;
        }
        let replay = input.idempotency_key.as_deref().is_some_and(|k| self.st().run_by_key(k).is_some());
        // The delegation's route takes the readiness check of a run's (`route_wait`); a replay is not asked about. A delegation
        // whose path is still open is classified by a call to the daemon's classifier route: that route is read with it.
        let wait = if replay {
            None
        } else {
            let routes = &self.inner.policy.routes;
            let classifier = if path.is_none() { routes.classifier() } else { None };
            match route_wait(routes, &record, classifier) {
                Ok(wait) => wait,
                Err(NotReady::Unavailable(message)) => return Err(Fault::Refused { status: 503, what: "route-unavailable", message }),
                Err(NotReady::Refused(reason)) => return refused("route-refused", json!({"run_id": run_id, "reason": reason, "declared": routes.names()})),
            }
        };

        let st = self.st();
        draining_fault(&st)?;
        match st.daemon() {
            Some(d) if d.daemon_id == daemon_id => {}
            _ => return refused("no-daemon", json!({"daemon_id": daemon_id})),
        }
        let session = input.session_id.as_ref().map(|sid| (sid, st.session(sid).map(|s| s.state)));
        if let Some((sid, None)) = &session {
            return refused("no-session", json!({"session_id": sid}));
        }
        if let Some((sid, Some(SessionState::Closed))) = &session {
            return refused("session-closed", json!({"run_id": run_id, "session_id": sid}));
        }
        if let Some(key) = &input.idempotency_key {
            if let Some(first) = st.run_by_key(key) {
                let first = first.run_id.clone();
                // the key may be one a submit-run used: its run is read from the view run, not from delegation-result
                let delegation = st.events_after(Some(&first), None, 0).iter().any(|e| e.kind == crate::delegation::DELEGATED_EVENT);
                let mut out = done("delegate", "already-submitted", vec![("SubmitReplayed", json!({"idempotency_key": key, "run_id": first}))]);
                out.location_view = (!delegation).then_some("run");
                out.run_id = Some(first);
                return Ok(out);
            }
        }
        let submitted = json!({"run_id": run_id, "runbook_id": runbook_id, "confinement": confinement});
        // a delegation the run classifies holds no path until it starts: the run event `runbook.delegation-resolved` names it then
        let mut event = json!({"route": route, "protocol": runbook_id});
        if let Some(path) = path {
            event["path"] = json!(path);
        }
        let mut delegated = event.clone();
        // who asked is the run's record (its event `runbook.delegated`), not a member of the published RunDelegated
        for (member, value) in [("caller", &input.caller), ("caller_label", &input.caller_label)] {
            if let Some(v) = value {
                event[member] = json!(v);
            }
        }
        delegated["run_id"] = json!(run_id);
        // the repository and the commit the run's branch starts from are the run's record, read when its run starts
        if let Some((entry, base)) = &change {
            event["repository"] = json!(entry.name);
            event["base"] = json!(base);
        }
        let admitted = Admitted {
            run_id: run_id.clone(),
            runbook_id: runbook_id.clone(),
            runbook_version: None,
            intake: input.request,
            context_path: input.workspace,
            context_dir,
            workspace_exclude: input.workspace_exclude,
            settings,
            confinement,
            source: ConfinementSource::DaemonDefault,
            terms,
            session_id: input.session_id,
            idempotency_key: input.idempotency_key,
            serial_key: input.serial_key,
            route: Some((record, table)),
            route_not_taken: None,
            delegation: Some(route.clone()),
            wait,
        };
        self.enter(st, admitted, Some(event))?;
        let mut out = done("delegate", "delegated", vec![("RunSubmitted", submitted), ("RunDelegated", delegated)]);
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
        let now = st.now();
        if let Some(until) = parse_instant(&now) {
            record_pod_time_locked(&mut st, &self.inner.policy.routes, &id, until);
        }
        st.commit(Fact::RunCancelled { run_id: id.clone(), at: now })?;
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
        let mut input = input;
        input.answered_by = client_label("answered_by", &input.answered_by).map_err(Fault::Invalid)?;
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
        // A request read from its case takes one of its options, or the free answer where it takes one;
        // anything else is refused before it is committed or handed on (`answer_offered` of the
        // specification, which the handler fills).
        let offered = st.decision(&request_id).and_then(|r| r.artifact.as_ref().map(|a| a.takes(&r.options, &input.answer))).unwrap_or(true);
        if !offered {
            return Ok(refuse("answer-decision", "unknown-option", json!({"request_id": request_id})));
        }
        let principal = self.inner.cfg.principal.clone();
        // The runtime is given the daemon's principal; the label a client typed is a label and nothing else.
        // The answer is on the journal before the run is handed it (a journal that cannot take the line
        // is an error to the client and the run has not seen the answer), and the state takes it only
        // after the run took it, all under the lock: a runtime that has failed while its run's thread has
        // not reported yet (the run is on its way to `runtime-failure`) answers `UnknownRun`, and then
        // the line is taken out of the journal again and the answer is refused, not recorded as given.
        let now = st.now();
        let mut delivered = Ok(());
        let recorded = st.commit_if(
            Fact::DecisionAnswered {
                request_id: request_id.clone(),
                answer: input.answer.clone(),
                answered_by: input.answered_by.clone(),
                principal: Some(principal.clone()),
                at: now,
            },
            || {
                delivered = self.inner.runtime.answer(&run_id, &request_id, &input.answer, &principal);
                !(matches!(delivered, Err(AnswerFailure::UnknownRun)) && lock(&self.inner.cancels).contains_key(&run_id))
            },
        )?;
        if !recorded {
            return Ok(refuse("answer-decision", "run-not-waiting", json!({"run_id": run_id})));
        }
        if delivered.is_err() {
            // The answer stands; the runtime cannot take it (its run is gone). Say so in the run's record.
            let _ = record_event_locked(&mut st, &run_id, "runbook.answer-undeliverable", json!({"request_id": request_id}));
        }
        drop(st);
        let published = json!({"request_id": request_id, "run_id": run_id, "answered_by": input.answered_by, "principal": principal});
        Ok(done("answer-decision", "answered", vec![("DecisionAnswered", published)]))
    }

    // ---- runtime-side commands: what the runtime's reports map to ----

    pub fn start_run(&self, run_id: &str) -> Reply {
        let mut st = self.st();
        match st.run(run_id).map(|r| r.state) {
            None => Ok(refuse("start-run", "no-run", json!({"run_id": run_id}))),
            Some(RunState::Submitted | RunState::WaitingForRoute) => {
                let now = st.now();
                st.commit(Fact::RunStarted { run_id: run_id.to_string(), at: now })?;
                Ok(done("start-run", "started", vec![("RunStarted", json!({"run_id": run_id}))]))
            }
            Some(_) => Ok(refuse("start-run", "wrong-state", json!({"run_id": run_id}))),
        }
    }

    pub fn fail_run_start(&self, run_id: &str, reason: &str) -> Reply {
        fail_run_start_locked(&mut self.st(), run_id, reason)
    }

    /// `WaitForRoute`, issued by the daemon itself: a route of a Submitted run could not answer at the submit, and the
    /// run waits for it. `phase` is what the gateway gave of the route's model.
    pub fn wait_for_route(&self, run_id: &str, phase: Option<&str>) -> Reply {
        let mut st = self.st();
        match st.run(run_id).map(|r| r.state) {
            None => Ok(refuse("wait-for-route", "no-run", json!({"run_id": run_id}))),
            Some(RunState::Submitted) => {
                st.commit(Fact::RunWaitingForRoute { run_id: run_id.to_string(), phase: phase.map(str::to_owned) })?;
                Ok(done("wait-for-route", "waiting-for-route", vec![("RunWaitingForRoute", json!({"run_id": run_id}))]))
            }
            Some(_) => Ok(refuse("wait-for-route", "wrong-state", json!({"run_id": run_id}))),
        }
    }

    /// `WaitOnKey`, issued by the daemon itself: a Submitted run whose serial key a live run submitted before it holds
    /// waits.
    pub fn wait_on_key(&self, run_id: &str) -> Reply {
        wait_on_key_locked(&mut self.st(), run_id)
    }

    /// `TakeKey`, issued by the daemon itself: no live run of the key was submitted before the waiting run any more.
    pub fn take_key(&self, run_id: &str) -> Reply {
        take_key_locked(&mut self.st(), run_id)
    }

    /// `EndRouteWait`, issued by the daemon itself: the run's wait for its route is over and a route of it still could
    /// not answer. The run ends `stopped: route`, with the last phase the gateway gave; the daemon writes the run event
    /// `runbook.route-wait-ended` first, while the run still takes events.
    pub fn end_route_wait(&self, run_id: &str, route: Option<&str>, phase: Option<&str>) -> Reply {
        self.end_route_wait_recorded(run_id, json!({"route": route, "phase": phase}), phase)
    }

    /// `EndRouteWait` with the record of its event `runbook.route-wait-ended`: a wake the gateway answered
    /// `spend-ceiling` ends the wait at once, and the record names the ceiling.
    fn end_route_wait_recorded(&self, run_id: &str, record: Value, phase: Option<&str>) -> Reply {
        let mut st = self.st();
        match st.run(run_id).map(|r| r.state) {
            None => Ok(refuse("end-route-wait", "no-run", json!({"run_id": run_id}))),
            Some(RunState::WaitingForRoute) => {
                record_event_locked(&mut st, run_id, "runbook.route-wait-ended", record)?;
                let now = st.now();
                st.commit(Fact::RunStoppedOnRoute { run_id: run_id.to_string(), phase: phase.map(str::to_owned), at: now })?;
                Ok(done("end-route-wait", "stopped", vec![("RunStoppedOnRoute", json!({"run_id": run_id}))]))
            }
            Some(_) => Ok(refuse("end-route-wait", "wrong-state", json!({"run_id": run_id}))),
        }
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

    /// What the submit or the delegation of `run_id` added to the daemon's exclude list: the member `workspace_exclude` of its
    /// event `runbook.submitted`, which `enter` wrote after the member passed its check.
    fn workspace_exclude_of(&self, run_id: &str) -> Vec<String> {
        let st = self.st();
        let events = st.events_after(Some(run_id), None, 0);
        let submitted = events.iter().find(|e| e.kind == "runbook.submitted");
        submitted.and_then(|e| e.record.get(WORKSPACE_EXCLUDE)).and_then(Value::as_array).map(|list| list.iter().filter_map(Value::as_str).map(str::to_owned).collect()).unwrap_or_default()
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
                let now = st.now();
                // the run's share of the pod's time of a priced managed route, up to its end
                if let Some(until) = parse_instant(&now) {
                    record_pod_time_locked(&mut st, &self.inner.policy.routes, run_id, until);
                }
                st.commit(Fact::RunEnded { run_id: run_id.to_string(), outcome: outcome.to_string(), at: now })?;
                Ok(done("end-run", "ended", vec![("RunEnded", json!({"run_id": run_id, "outcome": outcome}))]))
            }
            Some(_) => Ok(refuse("end-run", "wrong-state", json!({"run_id": run_id}))),
        }
    }

    // ---- views ----

    pub fn runs(&self) -> Vec<Value> {
        let st = self.st();
        run_rows(&st, st.runs().iter().rev())
    }

    pub fn runs_of_session(&self, session_id: &str) -> Vec<Value> {
        let st = self.st();
        run_rows(&st, st.runs().iter().rev().filter(|r| r.session_id.as_deref() == Some(session_id)))
    }

    pub fn runs_in_state(&self, state: &str) -> Result<Vec<Value>, Fault> {
        let state = parse_state(state)?;
        let st = self.st();
        Ok(run_rows(&st, st.runs().iter().rev().filter(|r| r.state == state)))
    }

    pub fn runs_of_session_in_state(&self, session_id: &str, state: &str) -> Result<Vec<Value>, Fault> {
        let state = parse_state(state)?;
        let st = self.st();
        Ok(run_rows(&st, st.runs().iter().rev().filter(|r| r.session_id.as_deref() == Some(session_id) && r.state == state)))
    }

    /// The view `RunDetail` (wire name `run`): zero or one row.
    pub fn run_detail(&self, run_id: &str) -> Vec<Value> {
        // What the run has used is the runtime's to say; asked before the state is locked.
        let used = self.inner.runtime.usage(run_id);
        let st = self.st();
        st.run(run_id)
            .map(|r| {
                let mut row = run_row(&st, r);
                let m = row.as_object_mut().unwrap();
                m.insert("intake".into(), json!(r.intake));
                let mut settings = r.settings.clone();
                if let Some(used) = used {
                    used.put(&mut settings);
                }
                m.insert("settings".into(), json!(settings));
                if let Some(c) = &r.context_path {
                    m.insert("context_path".into(), json!(c));
                }
                if let Some(roles) = r.model_route.as_ref().map(|route| &route.roles).filter(|roles| !roles.is_empty()) {
                    m.insert("model_routes".into(), json!(roles));
                }
                if let Some(phase) = &r.route_phase {
                    m.insert("route_phase".into(), json!(phase));
                }
                let events = st.events_after(Some(run_id), None, 0);
                // a delegation's path, protocol and confidence (docs/model-routes.md, section 9)
                m.extend(delegation_of(&events).unwrap_or_default());
                // what failed the run after it started, as the view `delegation-result` says it: the reason of its event
                // `runbook.runtime-failed` or `runbook.step-failed`, so that no failed run reads as one with no reason
                if r.failure.is_none() {
                    if let Some(reason) = events.iter().rev().find(|e| e.kind == RUNTIME_FAILED || e.kind == STEP_FAILED).and_then(|e| e.record["reason"].as_str()) {
                        m.insert("failure".into(), json!(reason));
                    }
                }
                if r.state == RunState::Running {
                    if let Some(step) = lock(&self.inner.steps).get(run_id) {
                        let now = st.now_millis();
                        let seconds = parse_instant(&step.started_at).map_or(0, |at| (now - at).max(0) / 1000);
                        let mut s = Map::new();
                        put(&mut s, "action", &step.action);
                        put(&mut s, "case_id", &step.case);
                        s.insert("started_at".into(), json!(step.started_at));
                        s.insert("seconds".into(), json!(seconds));
                        m.insert("step_in_flight".into(), Value::Object(s));
                    }
                }
                row
            })
            .into_iter()
            .collect()
    }

    pub fn open_decisions_of_run(&self, run_id: &str) -> Vec<Value> {
        self.st().open_decisions_of(run_id).into_iter().map(open_decision_row).collect()
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
                put_artifact(&mut m, d);
                m.insert("opened_at".into(), json!(d.opened_at));
                put(&mut m, "answer", &d.answer);
                put(&mut m, "answered_by", &d.answered_by);
                put(&mut m, "principal", &d.principal);
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

    /// The workspace of a run the daemon knows (its `work/`), or the refusal of an unknown run.
    fn workspace_of(&self, run_id: &str) -> Result<PathBuf, Fault> {
        let st = self.st();
        if st.run(run_id).is_none() {
            return Err(Fault::no_such_run(run_id));
        }
        Ok(st.workspace_dir(run_id))
    }

    /// The view `ProductsOfRun` (wire name `products`): one row, the files under the run's workspace
    /// (`engine::products::list`), read after the state is let go.
    pub fn products_of_run(&self, run_id: &str) -> Result<Vec<Value>, Fault> {
        let workspace = self.workspace_of(run_id)?;
        let l = crate::engine::products::list(&workspace)?;
        let files: Vec<Value> = l.files.iter().map(|f| json!({"path": f.path, "size": f.size, "modified_at": crate::clock::format_instant(f.modified_millis)})).collect();
        let mut m = Map::new();
        m.insert("run_id".into(), json!(run_id));
        m.insert("files".into(), json!(files));
        if let Some(cut) = l.cut {
            m.insert("cut".into(), json!(cut));
        }
        m.insert("hidden".into(), json!(l.hidden));
        m.insert("left_out".into(), json!(l.left_out));
        Ok(vec![Value::Object(m)])
    }

    /// The view `ProductOfRun` (wire name `product`): one row, one file of the run's workspace
    /// (`engine::products::read`), read after the state is let go.
    pub fn product_of_run(&self, run_id: &str, path: &str) -> Result<Vec<Value>, Fault> {
        let workspace = self.workspace_of(run_id)?;
        let r = crate::engine::products::read(&workspace, path)?;
        let mut m = Map::new();
        m.insert("run_id".into(), json!(run_id));
        m.insert("path".into(), json!(r.path));
        m.insert("size".into(), json!(r.size));
        m.insert("modified_at".into(), json!(crate::clock::format_instant(r.modified_millis)));
        m.insert("kind".into(), json!(if r.text.is_some() { "text" } else { "binary" }));
        if let Some(text) = r.text {
            m.insert("text".into(), json!(text));
        }
        m.insert("shown".into(), json!(r.shown));
        m.insert("truncated".into(), json!(r.truncated));
        m.insert("limit".into(), json!(crate::engine::products::LIMIT));
        Ok(vec![Value::Object(m)])
    }

    /// The view `DelegationResult` (wire name `delegation-result`): one row, what a delegation's run has come to. The route, path
    /// and protocol are its event `runbook.delegated`; the calls, tokens and cost the sum of its events `runbook.model-call`; the
    /// result the text of `result.md` of its workspace (`engine::products::read_within`, at most `delegation::RESULT_LIMIT` bytes)
    /// and the products the paths `products_of_run` lists, read after the state is let go. A run that no delegation created is
    /// refused 404 `not a delegation`.
    pub fn delegation_result(&self, run_id: &str) -> Result<Vec<Value>, Fault> {
        use crate::delegation::{DELEGATED_EVENT, RESULT_FILE, RESULT_LIMIT};
        let (mut m, workspace) = {
            let st = self.st();
            let run = st.run(run_id).ok_or_else(|| Fault::no_such_run(run_id))?;
            let events = st.events_after(Some(run_id), None, 0);
            let delegated = events.iter().find(|e| e.kind == DELEGATED_EVENT).ok_or_else(|| Fault::Refused {
                status: 404,
                what: "not a delegation",
                message: format!("the run {} was not created by a delegation: read it with the view run", crate::engine::products::said(run_id)),
            })?;
            let (mut calls, mut input_tokens, mut output_tokens, mut cost, mut cost_known) = (0u64, 0u64, 0u64, 0u64, true);
            for e in events.iter().filter(|e| e.kind == crate::relay::MODEL_CALL_EVENT) {
                let n = |k: &str| e.record[k].as_u64().unwrap_or(0);
                calls += 1;
                input_tokens = input_tokens.saturating_add(n("input_tokens"));
                output_tokens = output_tokens.saturating_add(n("output_tokens"));
                cost = cost.saturating_add(n("micro_usd"));
                cost_known &= e.record["cost_known"].as_bool().unwrap_or(false);
            }
            let mut m = Map::new();
            m.insert("run_id".into(), json!(run_id));
            m.insert("run_state".into(), json!(run.state));
            put(&mut m, "outcome", &run.outcome);
            // Why the run failed to start, or what failed it after it started: the reason of its event `runbook.runtime-failed`
            // (a step whose route did not serve its call names the route, the gateway's code and the setting) or
            // `runbook.step-failed` (the step whose program failed and how it ended).
            let failed_after_start = events.iter().rev().find(|e| e.kind == RUNTIME_FAILED || e.kind == STEP_FAILED).and_then(|e| e.record["reason"].as_str()).map(str::to_owned);
            put(&mut m, "failure", &run.failure.clone().or(failed_after_start));
            m.insert("route".into(), delegated.record["route"].clone());
            put(&mut m, "route_phase", &run.route_phase);
            m.insert("confidence".into(), Value::Null);
            m.extend(delegation_of(&events).unwrap_or_default());
            // a delegation that changes a repository: the repository, and once its run delivered the branch and the commit
            // it pushed, or why the hand-back pushed nothing (`crate::handback`)
            if let Some(name) = delegated.record.get("repository").filter(|v| v.is_string()) {
                m.insert("repository".into(), name.clone());
            }
            // (a run cancelled while its push ran has no event of the push's end: the branch is the one its push named)
            let refused = events.iter().rev().find(|e| e.kind == crate::handback::REFUSED_EVENT);
            let pushed = events.iter().rev().find(|e| e.kind == crate::handback::HANDED_BACK_EVENT).or_else(|| match refused {
                None => events.iter().rev().find(|e| e.kind == crate::handback::PUSHING_EVENT),
                Some(_) => None,
            });
            if let Some(e) = pushed {
                m.insert("branch".into(), e.record["branch"].clone());
                m.insert("commit".into(), e.record["commit"].clone());
            }
            if let Some(e) = refused {
                m.insert("handback_refused".into(), e.record["reason"].clone());
            }
            m.insert("calls".into(), json!(calls));
            m.insert("input_tokens".into(), json!(input_tokens));
            m.insert("output_tokens".into(), json!(output_tokens));
            m.insert("cost_micro_usd".into(), json!(cost));
            m.insert("cost_known".into(), json!(cost_known));
            (m, st.workspace_dir(run_id))
        };
        let products: Vec<String> = crate::engine::products::list(&workspace)?.files.into_iter().map(|f| f.path).collect();
        let (text, truncated) = match crate::engine::products::read_within(&workspace, RESULT_FILE, RESULT_LIMIT) {
            Ok(r) => (r.text, r.truncated),
            Err(_) => (None, false),
        };
        if let Some(text) = text {
            m.insert("result_text".into(), json!(text));
        }
        m.insert("result_truncated".into(), json!(truncated));
        m.insert("result_limit".into(), json!(RESULT_LIMIT));
        m.insert("products".into(), json!(products));
        Ok(vec![Value::Object(m)])
    }

    /// The view `MetricsOfRun` (wire name `run-metrics`): one row, what the run took, read from the run's own events in
    /// one walk (`crate::run_metrics`); an unknown run is refused 404 `no such run`.
    pub fn run_metrics(&self, run_id: &str) -> Result<Vec<Value>, Fault> {
        let st = self.st();
        let run = st.run(run_id).ok_or_else(|| Fault::no_such_run(run_id))?;
        Ok(vec![crate::run_metrics::row(run, st.events_of(run_id), st.now_millis())])
    }

    /// The view `CasesOfRun` (wire name `cases`): a row per case of the run's tree, as the runtime reads it
    /// (`Runtime::cases`), read after the state is let go. Where a case stands (`case_state`): `Ended` with an outcome;
    /// else `SpendCapReached`, `CallCapReached` or `WallCapReached` when the daemon ended the run at that cap (whatever
    /// the case's last stop: a run that ends at a cap between two steps leaves the step's `BudgetSpent` line last); else
    /// `AwaitingPerson` or `AwaitingAuthority` when its last stop says so and the run waits on a decision; else
    /// `Stuck` when its last stop is `Stuck` or `BudgetSpent` and the run ended by its tree (`Ended`, not orphaned: a wait
    /// or a cancel that ended with the run, or a run the daemon orphaned, leaves the case `Open`); else `Running` when the
    /// run's step in flight names the case; else `Open`.
    pub fn cases_of_run(&self, run_id: &str) -> Result<Vec<Value>, Fault> {
        let (state, step_case, capped, ended_by_its_tree) = {
            let st = self.st();
            let run = st.run(run_id).ok_or_else(|| Fault::no_such_run(run_id))?;
            let step = if run.state == RunState::Running { lock(&self.inner.steps).get(run_id).and_then(|s| s.case.clone()) } else { None };
            let ended = run.state == RunState::Ended;
            let capped = if ended { run.outcome.as_deref().and_then(crate::engine::Cap::of_outcome) } else { None };
            (run.state, step, capped, ended && run.outcome.as_deref() != Some("orphaned"))
        };
        let cases = self.inner.runtime.cases(run_id).map_err(|_| Fault::Refused { status: 503, what: "tree not readable", message: format!("the tree of run {} cannot be read at this moment: ask again", crate::engine::products::said(run_id)) })?;
        Ok(cases
            .into_iter()
            .map(|c| {
                let case_state = match (&c.outcome, capped, c.stop.as_deref()) {
                    (Some(_), ..) => "Ended",
                    (None, Some(cap), _) => cap.case_state(),
                    (None, None, Some(stop @ ("AwaitingPerson" | "AwaitingAuthority"))) if state == RunState::WaitingOnDecision => stop,
                    (None, None, Some("Stuck" | "BudgetSpent")) if ended_by_its_tree => "Stuck",
                    _ if step_case.as_deref() == Some(c.case_id.as_str()) => "Running",
                    _ => "Open",
                };
                let mut m = Map::new();
                m.insert("case_id".into(), json!(c.case_id));
                put(&mut m, "parent", &c.parent);
                put(&mut m, "name", &c.name);
                m.insert("protocol".into(), json!(c.protocol));
                m.insert("case_state".into(), json!(case_state));
                put(&mut m, "outcome", &c.outcome);
                Value::Object(m)
            })
            .collect())
    }

    /// The view `DaemonConfiguration` (wire name `configuration`).
    pub fn daemon_configuration(&self) -> Vec<Value> {
        self.st()
            .daemon()
            .map(|d| json!({"daemon_id": d.daemon_id, "confinement_floor": d.confinement_floor, "principal": d.principal, "started_at": d.started_at}))
            .into_iter()
            .collect()
    }

    /// The view `Routes` (wire name `routes`): the declared model routes, by name, with what their gateways said
    /// at the last read, which is at most [`crate::routes::READ_EVERY_MILLIS`] of the daemon's clock old. A row
    /// names the gateway by name and holds no address, token or price. A managed route's row also answers the figures of
    /// the day of the daemon's clock that its wake cap counts ([`wake_figures`]), read from the state when it is asked. Every
    /// row answers the route's figures of that day that its daily caps are held against (`daily_booked_micro_usd`,
    /// `daily_llm_calls`, and the caps it declares: [`crate::route_day::RouteDays::figures`]), read from the same figures
    /// the relay admits a call by.
    pub fn routes(&self) -> Vec<Value> {
        let mut rows = self.gateway_rows();
        let st = self.st();
        for row in &mut rows {
            let name = row["route"].as_str().unwrap_or_default().to_owned();
            let phase = row["phase"].as_str().map(str::to_owned);
            let mut figures = wake_figures(&st, &self.inner.policy.routes, &name, phase.as_deref());
            figures.extend(self.inner.days.figures(&name));
            if let Some(m) = row.as_object_mut().filter(|_| !figures.is_empty()) {
                // the figures go before `read_at`, the last member of a row
                let read_at = m.shift_remove("read_at");
                m.extend(figures);
                if let Some(at) = read_at {
                    m.insert("read_at".into(), at);
                }
            }
        }
        rows
    }

    /// The day's figures of the declared routes, for the relay (`crate::relay::Relay::count_days`): the one place the
    /// daily caps of a route are held and the view `routes` reads them.
    pub fn route_days(&self) -> Arc<crate::route_day::RouteDays> {
        self.inner.days.clone()
    }

    /// The rows of the view `routes` as the gateways were read last, without the day's figures.
    fn gateway_rows(&self) -> Vec<Value> {
        let now = self.st().now_millis();
        self.inner.routes.rows(now)
    }

    /// The route entries of the runbook `runbook` (and version) a submit or a delegation names (`crate::admission`), with
    /// the facts of the declared routes as the view `routes` read them last: none when it has no file. `Err` is the reason
    /// of `route-refused`, naming the runbook: a file that cannot be read, or one no run can admit (`admission::check`).
    fn protocol_word(&self, runbook: &str, version: Option<&str>) -> Result<Option<ProtocolWord>, String> {
        use crate::admission::{check, parse, Facts};
        let Some(text) = self.inner.runtime.routes_file(runbook, version) else { return Ok(None) };
        let declared = text.and_then(|t| parse(runbook, &t)).map_err(|why| format!("runbook {runbook}: {why}"))?;
        let facts: Vec<Facts> = self.gateway_rows().iter().map(Facts::of_row).collect();
        check(runbook, &declared, &facts)?;
        Ok(Some(ProtocolWord { runbook: runbook.to_owned(), declared, facts }))
    }

    /// The view `Catalog` (wire name `catalog`, `docs/model-routes.md` section 10): one row with what a delegation may name.
    /// `routes`: the rows of the view `routes` (the gateways read at most once every [`crate::routes::READ_EVERY_MILLIS`])
    /// with the prices and the wait of the daemon's file. `runbooks`: the rows of the view `runbooks` with what the runtime
    /// says of each (`Runtime::runbook_facts`) and the admission of its route entries (`crate::admission`) against those
    /// routes and the default route. `delegation`: the members of `Delegate` as the OpenAPI document gives them, the paths,
    /// the default and classifier routes, the level and the most bytes of a request. No address, token or path.
    pub fn catalog(&self) -> Vec<Value> {
        use crate::admission::{admit, parse, Facts};
        let table = self.inner.routes.table().clone();
        let rows = self.routes();
        let facts: Vec<Facts> = rows.iter().map(Facts::of_row).collect();
        let routes: Vec<Value> = rows
            .into_iter()
            .map(|mut row| {
                let declared = row["route"].as_str().and_then(|r| table.route(r)).cloned();
                if let (Some(route), Some(m)) = (declared, row.as_object_mut()) {
                    let read_at = m.shift_remove("read_at");
                    let numbers = [("price_in_micro_usd_per_mtok", route.prices.in_micro_usd_per_mtok), ("price_out_micro_usd_per_mtok", route.prices.out_micro_usd_per_mtok), ("start_wait_seconds", route.start_wait_seconds)];
                    for (key, value) in numbers {
                        if let Some(n) = value {
                            m.insert(key.into(), json!(n));
                        }
                    }
                    if let Some(at) = read_at {
                        m.insert("read_at".into(), at);
                    }
                }
                row
            })
            .collect();
        let default = table.default_route();
        let runbooks: Vec<Value> = self
            .runbooks()
            .into_iter()
            .map(|row| {
                let id = row["runbook_id"].as_str().unwrap_or_default().to_owned();
                let f = self.inner.runtime.runbook_facts(&id);
                let mut m = Map::new();
                m.insert("runbook_id".into(), json!(id));
                if let Some(v) = row.get("runbook_version") {
                    m.insert("runbook_version".into(), v.clone());
                }
                put(&mut m, "title", &f.title);
                put(&mut m, "description", &f.description);
                m.insert("inputs".into(), json!(f.inputs));
                let (entries, allowed, refusal) = match f.routes_file.map(|text| text.and_then(|t| parse(&id, &t))) {
                    None => (Vec::new(), None, None),
                    Some(Err(why)) => (Vec::new(), None, Some(why)),
                    Some(Ok(declared)) => {
                        let a = admit(&declared, &facts, default);
                        let refusal = (!a.reasons.is_empty()).then(|| a.reasons.join("; "));
                        (a.entries, a.routes_allowed, refusal)
                    }
                };
                m.insert("route_entries".into(), json!(entries));
                if let Some(list) = allowed {
                    m.insert("routes_allowed".into(), json!(list));
                }
                m.insert("admitted".into(), json!(refusal.is_none()));
                put(&mut m, "refusal", &refusal);
                Value::Object(m)
            })
            .collect();
        let schema = crate::mcp::schema::command_input("delegate", &crate::mcp::tools::DELEGATE_FILLED, &crate::mcp::tools::SUBMIT_DEFAULTED);
        let required: Vec<&str> = schema["required"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        let members: Vec<Value> = schema["properties"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(name, s)| {
                let mut m = Map::new();
                m.insert("name".into(), json!(name));
                m.insert("required".into(), json!(required.contains(&name.as_str())));
                if let Some(d) = s["description"].as_str() {
                    m.insert("summary".into(), json!(d));
                }
                if let Some(n) = s["maxLength"].as_u64() {
                    m.insert("max_length".into(), json!(n));
                }
                Value::Object(m)
            })
            .collect();
        let mut paths = Vec::new();
        if !runbooks.is_empty() {
            paths.push("named");
        }
        if self.inner.runtime.free_run() {
            paths.push("free");
        }
        // a daemon whose runbook.toml lists repositories takes a delegation that changes one (`crate::handback`)
        if self.inner.runtime.free_run() && !self.inner.policy.repositories.is_empty() {
            paths.push("change");
        }
        // a daemon with a classifier route classifies a delegation that names no protocol (section 9): it runs the runbook
        // the classifier picked, or builds one, whose agent node is the free run's
        if self.inner.runtime.classifies() {
            if !runbooks.is_empty() {
                paths.push("classified");
            }
            if self.inner.runtime.free_run() {
                paths.push("generated");
            }
        }
        let mut delegation = Map::new();
        delegation.insert("members".into(), json!(members));
        delegation.insert("paths".into(), json!(paths));
        put(&mut delegation, "default_route", &default.map(str::to_owned));
        put(&mut delegation, "classifier_route", &table.classifier().map(str::to_owned));
        if let Some(level) = self.inner.policy.default_confinement {
            delegation.insert("confinement".into(), json!(level));
        }
        delegation.insert("request_limit".into(), json!(MAX_INTAKE_BYTES));
        vec![json!({"routes": routes, "runbooks": runbooks, "delegation": delegation})]
    }

    /// The view `Runbooks` (wire name `runbooks`): the runtime's own runbooks and the entries of
    /// `--runbook-dir`, each name once (the runtime's row wins), by name. A name is listed only when
    /// `runbook_known` accepts it, and a row holds a name, a version and the runtime, never a path.
    pub fn runbooks(&self) -> Vec<Value> {
        let kind = self.inner.runtime.kind();
        let mut by_name: BTreeMap<String, Option<String>> = BTreeMap::new();
        if let Some(dir) = &self.inner.cfg.runbook_dir {
            for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let Ok(file) = entry.file_name().into_string() else { continue };
                let name = match file.strip_suffix(".protocol.yaml") {
                    Some(stem) if entry.path().is_file() => stem.to_string(),
                    _ if entry.path().is_dir() => file,
                    _ => continue,
                };
                if self.runbook_known(&name, None) {
                    by_name.insert(name, None);
                }
            }
        }
        for info in self.inner.runtime.runbooks() {
            by_name.insert(info.name, info.version);
        }
        by_name
            .into_iter()
            .map(|(name, version)| {
                let mut m = Map::new();
                m.insert("runbook_id".into(), json!(name));
                put(&mut m, "runbook_version", &version);
                m.insert("runtime".into(), json!(kind));
                Value::Object(m)
            })
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

    /// The entry a delegation's `repository` names and its base as a commit, read at the submit; `Err` is the reason of
    /// `repository-refused`: not an entry (or a path in place of a name), a protocol beside it, a level that is not
    /// `local` (the first slice serves `local` only), or a base that names no commit of the checkout.
    fn repository_for(&self, named: &str, protocol: Option<&str>, confinement: Confinement) -> Result<(crate::handback::Repository, String), String> {
        if let Some(why) = crate::handback::refusal(&self.inner.policy.repositories, named) {
            return Err(why);
        }
        if protocol.is_some_and(|p| p != crate::delegation::CHANGE_RUNBOOK) {
            return Err(format!("protocol: a delegation that names a repository runs the daemon's protocol {}; name no other", crate::delegation::CHANGE_RUNBOOK));
        }
        if confinement != Confinement::Local {
            return Err(format!("repository: a delegation that changes a repository runs at the level local only, and this daemon's default level is {}", confinement.as_str()));
        }
        if !self.inner.runtime.free_run() {
            return Err("repository: this daemon's runtime has no agent node of the daemon's own (the loom runtime has)".to_owned());
        }
        let entry = self.inner.policy.repositories.get(named).cloned().ok_or("repository: not an entry of runbook.toml")?;
        let base = crate::handback::resolve_base(&entry)?;
        Ok((entry, base))
    }

    /// The repository entry and the base commit of a run that changes a repository, read from its event `runbook.delegated`;
    /// none for any other run. `Err`: the entry is no longer in the daemon's file (the run's failure to start).
    fn change_of_run(&self, run_id: &str) -> Option<Result<(crate::handback::Repository, String), String>> {
        let st = self.st();
        let events = st.events_after(Some(run_id), None, 0);
        let record = &events.iter().find(|e| e.kind == crate::delegation::DELEGATED_EVENT)?.record;
        let (name, base) = (record["repository"].as_str()?, record["base"].as_str()?);
        Some(self.inner.policy.repositories.get(name).cloned().map(|entry| (entry, base.to_owned())).ok_or_else(|| format!("repository: {name} is no longer an entry of the daemon's runbook.toml")))
    }

    /// The hand-back of a run that delivered: its change committed and pushed (`crate::handback::hand_back`), the lease on its
    /// tree ended, and a tree whose change was pushed finished. The run event `runbook.handed-back` (repository, branch,
    /// commit, base, whether the tree was finished) or `runbook.hand-back-refused` (repository, reason) records it. Right
    /// before the push, while the run is still live and not cancelled, `runbook.pushing` (repository, branch, commit) is
    /// written under the state's lock, so a cancel either comes first and nothing is pushed, or comes after that event,
    /// which names the branch the push makes.
    fn hand_back(&self, run_id: &str, entry: &crate::handback::Repository, tree: &crate::handback::Tree, run_dir: &Path, cancel: &AtomicBool) {
        let before_push = |h: &crate::handback::Handed| {
            !cancel.load(Ordering::SeqCst)
                && self
                    .record_run_event(run_id, crate::handback::PUSHING_EVENT, json!({"repository": entry.name, "branch": h.branch, "commit": h.commit}))
                    .is_ok_and(|o| !o.is_refusal())
        };
        let handed = crate::handback::hand_back(entry, run_id, tree, run_dir, &before_push);
        crate::handback::release(run_id, tree);
        let _ = match handed {
            Ok(h) => {
                let finished = crate::handback::finish(entry, run_id);
                self.record_run_event(run_id, crate::handback::HANDED_BACK_EVENT, json!({"repository": entry.name, "branch": h.branch, "commit": h.commit, "base": tree.base, "tree_finished": finished}))
            }
            Err(reason) => self.record_run_event(run_id, crate::handback::REFUSED_EVENT, json!({"repository": entry.name, "reason": reason})),
        };
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

    /// Holds a run whose routes could not all answer at the submit (`docs/model-routes.md`, section 3): the run is
    /// WaitingForRoute, its routes are read again at most once every 5 s of the daemon's clock, and no call is sent to
    /// them. A stopped managed route whose wake is on is woken once first (`wake_route`). True when every route can
    /// answer: the run starts. False when the wait is over (the run ends `stopped: route` with the last phase), a wake
    /// was answered `spend-ceiling` (it ends so at once), the run was cancelled or the daemon stops.
    fn wait_for_routes(&self, run_id: &str, wait: &RouteWait, cancel: &AtomicBool) -> bool {
        let wait_seconds = wait.wait_millis / 1000;
        let record = json!({"route": wait.route, "phase": wait.phase, "wait_seconds": wait_seconds});
        if self.record_run_event(run_id, "runbook.waiting-for-route", record).is_err() {
            return false;
        }
        match self.wait_for_route(run_id, wait.phase.as_deref()) {
            Ok(o) if !o.is_refusal() => {}
            _ => return false,
        }
        let ceiling = |name: &str, phase: &Option<String>| {
            let record = json!({"route": name, "phase": phase, "ceiling": crate::routes::SPEND_CEILING});
            let _ = self.end_route_wait_recorded(run_id, record, phase.as_deref());
            false
        };
        // the wakes this run joined: route, phase and the run that sent the wake
        let mut joined: Vec<(String, Option<String>, String)> = Vec::new();
        // the wakes the gateway did not serve: route, phase and when to send it again
        let mut again: Vec<(String, Option<String>, i64)> = Vec::new();
        for (name, phase) in &wait.wake {
            if cancel.load(Ordering::SeqCst) || self.is_stopping() {
                return false;
            }
            match self.wake_route(run_id, name, false) {
                Woke::Sent => {}
                Woke::Failed => again.push((name.clone(), phase.clone(), self.st().now_millis().saturating_add(WAKE_AGAIN_AFTER_MILLIS))),
                Woke::Ceiling => return ceiling(name, phase),
                Woke::Joined(run) => joined.push((name.clone(), phase.clone(), run)),
            }
        }
        let start = self.st().now_millis();
        let deadline = start.saturating_add(wait.wait_millis);
        let (mut route, mut phase, mut read_at) = (wait.route.clone(), wait.phase.clone(), start);
        loop {
            if cancel.load(Ordering::SeqCst) || self.is_stopping() {
                return false;
            }
            // a joined wake the gateway answered `spend-ceiling` (before the join or since) ends this run as it ended the
            // run that sent it
            if let Some((name, phase, _)) = joined.iter().find(|(name, _, run)| answered_ceiling(&self.st(), run, name)) {
                return ceiling(name, phase);
            }
            let now = self.st().now_millis();
            if now >= read_at.saturating_add(ROUTE_READ_EVERY_MILLIS) {
                read_at = now;
                match read_routes(&self.inner.policy.routes, &wait.routes) {
                    Ok(None) => return !cancel.load(Ordering::SeqCst),
                    Ok(Some((r, p))) => {
                        route = r;
                        if p.is_some() {
                            phase = p;
                        }
                    }
                    // a gateway that does not answer or no longer lists the alias while the run waits: it waits on
                    Err(_) => {}
                }
            }
            if now >= deadline {
                let _ = self.end_route_wait(run_id, Some(&route), phase.as_deref());
                return false;
            }
            // a wake the gateway did not serve is sent again after its pause, before the wait is over, while the gateway
            // still gives the model as stopped (a model another wake is starting is not woken again)
            for (name, phase, next) in &mut again {
                if now < *next {
                    continue;
                }
                let until = std::time::Instant::now() + crate::routes::READ_BOUND;
                let stopped = matches!(self.inner.policy.routes.readiness(name, until), Some(crate::routes::Readiness::NotCallable { phase: Some(p) }) if p == "stopped");
                if !stopped {
                    *next = now.saturating_add(WAKE_AGAIN_AFTER_MILLIS);
                    continue;
                }
                match self.wake_route(run_id, name, true) {
                    Woke::Failed => *next = self.st().now_millis().saturating_add(WAKE_AGAIN_AFTER_MILLIS),
                    Woke::Ceiling => return ceiling(name, phase),
                    Woke::Sent | Woke::Joined(_) => *next = i64::MAX,
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    /// The readiness check of the routes of `record` for the running run `run_id` (a delegation the run classified, whose
    /// runbook's route entries gave it routes the submit did not read; `docs/model-routes.md`, section 3): every route can
    /// answer, or the run waits as a submitted run does, reading them at most once every 5 s of the daemon's clock up to the
    /// least `start_wait_seconds` of those that cannot (the run event `runbook.waiting-for-route`). `Err` is the run's
    /// failure: `route-unavailable: ...` for a gateway that does not answer or is not ready and for a wait that ran out,
    /// `route-refused: ...` for an alias the gateway does not list; a cancel or the daemon's stop ends the wait too.
    fn ready_routes(&self, run_id: &str, record: &RunRoute) -> Result<(), String> {
        let routes = &self.inner.policy.routes;
        let wait = match route_wait(routes, record, None) {
            Ok(None) => return Ok(()),
            Ok(Some(wait)) => wait,
            Err(NotReady::Unavailable(message)) => return Err(format!("route-unavailable: {message}")),
            Err(NotReady::Refused(reason)) => return Err(format!("route-refused: {reason}")),
        };
        let wait_seconds = wait.wait_millis / 1000;
        let _ = self.record_run_event(run_id, "runbook.waiting-for-route", json!({"route": wait.route, "phase": wait.phase, "wait_seconds": wait_seconds}));
        let cancel = lock(&self.inner.cancels).get(run_id).cloned();
        let start = self.st().now_millis();
        let deadline = start.saturating_add(wait.wait_millis);
        let (mut route, mut phase, mut read_at) = (wait.route.clone(), wait.phase.clone(), start);
        loop {
            if cancel.as_ref().is_some_and(|c| c.load(Ordering::SeqCst)) || self.is_stopping() {
                return Err("the run was cancelled while it waited for its routes".to_owned());
            }
            let now = self.st().now_millis();
            if now >= read_at.saturating_add(ROUTE_READ_EVERY_MILLIS) {
                read_at = now;
                match read_routes(routes, &wait.routes) {
                    Ok(None) => return Ok(()),
                    Ok(Some((r, p))) => {
                        route = r;
                        if p.is_some() {
                            phase = p;
                        }
                    }
                    // a gateway that does not answer or no longer lists the alias while the run waits: it waits on
                    Err(_) => {}
                }
            }
            if now >= deadline {
                let phase = phase.as_deref().unwrap_or("unknown");
                return Err(format!("route-unavailable: route {route} could not answer within its start_wait_seconds of {wait_seconds} s (phase {phase}); no call was sent to it"));
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    /// Wakes the managed route `name` once for the run (`docs/model-routes.md`, section 3 point 4). A run within the
    /// route's `start_wait_seconds` of a wake sent for the route joins that wake: nothing is sent or booked, and the event
    /// names the run and the instant of that wake (`joined`, `joined_at`). Otherwise the wake is booked
    /// against the route's cap for the day of the daemon's clock before it is sent, under the lock of the state, in the
    /// run event `runbook.route-wake`; a wake the rest of the day's cap does not cover is not sent and the event says
    /// "wake cap reached". The gateway's answer is the event `runbook.route-wake-answered`. [`Woke::Ceiling`] when the
    /// gateway answered `spend-ceiling`: the run stops; [`Woke::Joined`] names the run whose wake this run joined, whose
    /// answer is this run's too. A wake the gateway did not take (not a 2xx) books nothing (`wakes_booked`);
    /// [`Woke::Failed`] when it answered a 5xx other than `spend-ceiling` or nothing in time: the run's wait sends it
    /// again (`again`: a try of the run's own after a failed one, which joins no wake).
    fn wake_route(&self, run_id: &str, name: &str, again: bool) -> Woke {
        let routes = &self.inner.policy.routes;
        let Some(route) = routes.route(name) else { return Woke::Sent };
        let (Some(cap), Some(cost)) = (route.wake.daily_cap_micro_usd, route.wake.cost_micro_usd) else { return Woke::Sent };
        let wait_millis = i64::try_from(route.start_wait_seconds.unwrap_or(0)).unwrap_or(i64::MAX / 2).saturating_mul(1000);
        {
            let mut st = self.st();
            // a wake sent for the route within its wait is still starting the model: the run joins it and books nothing
            if let Some((run, at)) = wake_sent_within(&st, name, st.now_millis(), wait_millis).filter(|_| !again) {
                let record = json!({"route": name, "sent": false, "reason": WAKE_JOINED, "joined": run, "joined_at": at});
                let _ = record_event_locked(&mut st, run_id, ROUTE_WAKE, record);
                return Woke::Joined(run);
            }
            let day = wake_day(&st);
            // the day's cap counts the pod's time of a priced route too, its runs that still live up to now
            let (wakes, pod) = day_booked(&st, routes, name, &day, st.now_millis());
            let booked = wakes.saturating_add(pod.unwrap_or(0));
            let fits = booked.checked_add(cost).is_some_and(|total| total <= cap);
            let mut record = json!({"route": name, "sent": fits, "day": day, "cost_micro_usd": cost, "booked_micro_usd": if fits { booked + cost } else { booked }, "cap_micro_usd": cap});
            if let Some(pod) = pod {
                record["pod_micro_usd"] = json!(pod);
            }
            if !fits {
                record["reason"] = json!(WAKE_CAP_REACHED);
            }
            // a booking that cannot be written sends no wake
            if record_event_locked(&mut st, run_id, ROUTE_WAKE, record).is_err() || !fits {
                return Woke::Sent;
            }
        }
        let answer = routes.wake(name, std::time::Instant::now() + crate::routes::READ_BOUND).unwrap_or_default();
        let mut record = json!({"route": name, "status": answer.status});
        if let Some(code) = &answer.code {
            record["code"] = json!(code);
        }
        {
            // a run closed while its wake was out takes no event: the answer is the daemon's line, so the booking sees it
            let mut st = self.st();
            if run_lives(&st, run_id) {
                let _ = record_event_locked(&mut st, run_id, ROUTE_WAKE_ANSWERED, record);
            } else {
                let at = st.now();
                let _ = st.commit(Fact::RouteWakeAnswered(crate::state::WakeAnswer { run_id: run_id.to_owned(), record, at }));
            }
        }
        if answer.code.as_deref() == Some(crate::routes::SPEND_CEILING) {
            Woke::Ceiling
        } else if answer.status.is_none_or(|s| s >= 500) {
            Woke::Failed
        } else {
            Woke::Sent
        }
    }

    fn spawn_run(&self, request: StartRequest, wait: Option<RouteWait>) {
        let flag = Arc::new(AtomicBool::new(false));
        lock(&self.inner.cancels).insert(request.run_id.clone(), flag.clone());
        let d = self.clone();
        let name = format!("run-{}", request.run_id);
        let handle = std::thread::Builder::new().name(name).spawn(move || d.run_thread(request, wait, &flag)).expect("spawn a thread for the run");
        let mut threads = lock(&self.inner.threads);
        threads.retain(|h| !h.is_finished());
        threads.push(handle);
    }

    /// A run the runtime failed on while it waited on decisions can only end from Running: its open
    /// requests are withdrawn (as a cancel does) and the run is Running again.
    fn leave_wait(&self, run_id: &str) {
        let mut st = self.st();
        if st.run(run_id).map(|r| r.state) != Some(RunState::WaitingOnDecision) {
            return;
        }
        let open: Vec<String> = st.open_decisions_of(run_id).iter().map(|d| d.request_id.clone()).collect();
        for request_id in open {
            let _ = withdraw_decision_locked(&mut st, &request_id);
        }
        drop(st);
        let _ = self.resume_run(run_id);
    }

    /// Holds a run whose serial key a live run submitted before it holds (`State::ahead_on_key`): the run is WaitingOnKey
    /// (`WaitOnKey`) and looks again every [`KEY_LOOK_EVERY`] until no such run is live, then takes the key (`TakeKey`).
    /// The order is the journal's, so the runs of a key start in submit order, also after a restart. A cancel or the
    /// daemon's stop ends the wait and leaves the run's state as it is; both are read again under the state's lock, so
    /// a run takes no key after the daemon's stop began. A run that goes on is taken up (`State::take_up`) under that
    /// lock: it holds its key until its run thread returns.
    fn wait_for_key(&self, run_id: &str, cancel: &AtomicBool) -> Keyed {
        loop {
            {
                let mut st = self.st();
                if cancel.load(Ordering::SeqCst) || self.is_stopping() {
                    return Keyed::Left;
                }
                let Some(run) = st.run(run_id) else { return Keyed::Left };
                if run.serial_key.is_none() {
                    st.take_up(run_id);
                    return Keyed::Free;
                }
                let state = run.state;
                let ahead = st.ahead_on_key(run_id).is_some();
                match (state, ahead) {
                    (RunState::Submitted, false) => {
                        st.take_up(run_id);
                        return Keyed::Free;
                    }
                    (RunState::WaitingOnKey, false) => {
                        return match take_key_locked(&mut st, run_id) {
                            Ok(o) if !o.is_refusal() => {
                                st.take_up(run_id);
                                Keyed::Took
                            }
                            _ => Keyed::Left,
                        };
                    }
                    (RunState::Submitted, true) => {
                        if !matches!(wait_on_key_locked(&mut st, run_id), Ok(o) if !o.is_refusal()) {
                            return Keyed::Left;
                        }
                    }
                    (RunState::WaitingOnKey, true) => {}
                    // cancelled (or moved by anything else) while it waited
                    _ => return Keyed::Left,
                }
            }
            std::thread::sleep(KEY_LOOK_EVERY);
        }
    }

    /// The wait for the routes of a run that took its serial key after a wait: they are read again now (`route_wait`), as a
    /// submit reads them; a delegation the run classifies reads the classifier route with them. `Err` is the run's failure to
    /// start: `route-unavailable: ...` or `route-refused: ...`.
    fn routes_after_key(&self, request: &StartRequest) -> Result<Option<RouteWait>, String> {
        let Some(record) = self.st().run(&request.run_id).and_then(|r| r.model_route.clone()) else { return Ok(None) };
        let routes = &self.inner.policy.routes;
        let classifier = if request.model_route.is_some() && request.runbook == crate::delegation::CLASSIFY_RUNBOOK { routes.classifier() } else { None };
        route_wait(routes, &record, classifier).map_err(|e| match e {
            NotReady::Unavailable(message) => format!("route-unavailable: {message}"),
            NotReady::Refused(reason) => format!("route-refused: {reason}"),
        })
    }

    fn run_thread(&self, request: StartRequest, wait: Option<RouteWait>, cancel: &AtomicBool) {
        let run_id = request.run_id.clone();
        let keyed = self.wait_for_key(&run_id, cancel);
        // the run holds its serial key until this thread returns: the runtime has let it go then
        let _taken_up = (!matches!(keyed, Keyed::Left)).then(|| TakenUp { daemon: self.clone(), run_id: run_id.clone() });
        let wait = match keyed {
            Keyed::Free => wait,
            Keyed::Took => match self.routes_after_key(&request) {
                Ok(wait) => wait,
                Err(reason) => {
                    if !self.is_stopping() {
                        let _ = self.fail_run_start(&run_id, &reason);
                    }
                    lock(&self.inner.cancels).remove(&run_id);
                    return;
                }
            },
            Keyed::Left => {
                lock(&self.inner.cancels).remove(&run_id);
                return;
            }
        };
        if let Some(wait) = &wait {
            if !self.wait_for_routes(&run_id, wait, cancel) {
                lock(&self.inner.cancels).remove(&run_id);
                return;
            }
        }
        if let (Some(from), false) = (&request.context_dir, cancel.load(Ordering::SeqCst)) {
            // `.git`, the daemon's exclude list and the run's own are left out, within the daemon's bounds (`crate::context_copy`)
            let extra = self.workspace_exclude_of(&run_id);
            match crate::context_copy::copy(from, &request.workspace_dir, &self.inner.policy.copy, &extra) {
                Ok(copied) => {
                    let _ = self.record_run_event(&run_id, crate::context_copy::COPIED_EVENT, copied.record());
                }
                Err(e) => {
                    if !self.is_stopping() {
                        let _ = self.fail_run_start(&run_id, &e.reason());
                    }
                    lock(&self.inner.cancels).remove(&run_id);
                    return;
                }
            }
        }
        // A delegation answers only a result.md its run wrote: one that came with the workspace is set aside.
        let delegation = self.st().events_after(Some(&run_id), None, 0).iter().any(|e| e.kind == crate::delegation::DELEGATED_EVENT);
        if delegation {
            if let Err(e) = crate::delegation::set_aside(&request.workspace_dir) {
                if !self.is_stopping() {
                    let _ = self.fail_run_start(&run_id, &format!("the result.md of the workspace could not be set aside: {}", e.kind()));
                }
                lock(&self.inner.cancels).remove(&run_id);
                return;
            }
        }
        // A delegation that changes a repository: the daemon makes the run's worktree now, before the run starts, at the base
        // its submit resolved (`crate::handback`). A tree that cannot be made fails the start.
        let mut request = request;
        let change = match self.change_of_run(&run_id) {
            Some(Ok((entry, base))) if !cancel.load(Ordering::SeqCst) => match crate::handback::make(&entry, &run_id, &base, &request.state_dir) {
                Ok(tree) => Some((entry, tree)),
                Err(why) => {
                    if !self.is_stopping() {
                        let _ = self.fail_run_start(&run_id, &format!("repository: {why}"));
                    }
                    lock(&self.inner.cancels).remove(&run_id);
                    return;
                }
            },
            Some(Err(why)) => {
                if !self.is_stopping() {
                    let _ = self.fail_run_start(&run_id, &why);
                }
                lock(&self.inner.cancels).remove(&run_id);
                return;
            }
            _ => None,
        };
        request.worktree = change.as_ref().map(|(_, tree)| tree.path.clone());
        let run_dir = request.state_dir.clone();
        let sink = RunSink { daemon: self.clone(), run_id: run_id.clone(), started: AtomicBool::new(false) };
        let result = if cancel.load(Ordering::SeqCst) { Ok(Finish::Cancelled) } else { self.inner.runtime.start(request, &sink, cancel) };
        // The hand-back (`crate::handback`): after the outcome delivered the daemon commits and pushes the run's change before
        // the run ends, and records what came of it; a run that ends any other way pushes nothing and keeps its tree. The
        // run's lease on its tree ends in every case.
        if let (Some((entry, tree)), false) = (&change, self.is_stopping()) {
            if matches!(&result, Ok(Finish::Outcome(o)) if o == "delivered") {
                self.hand_back(&run_id, entry, tree, &run_dir, cancel);
            } else {
                crate::handback::release(&run_id, tree);
            }
        }
        if !self.is_stopping() {
            // A refusal below means the run is Cancelled already (a report after a cancel).
            match result {
                Ok(Finish::Outcome(outcome)) => {
                    // A run stopped while it waited on decisions (a cap, also one reached while the
                    // last answer was taken, with nothing left open): what it asked is withdrawn and
                    // it ends from Running.
                    self.leave_wait(&run_id);
                    let _ = self.end_run(&run_id, &outcome);
                }
                // A step failed and nothing else could be done: why is recorded, then the run ends with the outcome that
                // names the step and its exit.
                Ok(Finish::StepFailed { outcome, record }) => {
                    let _ = self.record_run_event(&run_id, STEP_FAILED, record);
                    self.leave_wait(&run_id);
                    let _ = self.end_run(&run_id, &outcome);
                }
                // CancelRun moved the run already.
                Ok(Finish::Cancelled) => {}
                Err(f) if sink.started.load(Ordering::SeqCst) => {
                    // a runtime-failure always says why: a runtime that gave no text is said to have given none
                    let reason = if f.reason.trim().is_empty() { "the runtime ended the run and gave no reason".to_owned() } else { f.reason };
                    let _ = self.record_run_event(&run_id, RUNTIME_FAILED, json!({"reason": reason}));
                    self.leave_wait(&run_id);
                    let _ = self.end_run(&run_id, "runtime-failure");
                }
                Err(f) => {
                    let _ = self.fail_run_start(&run_id, &f.reason);
                }
            }
        }
        lock(&self.inner.steps).remove(&run_id);
        lock(&self.inner.cancels).remove(&run_id);
    }
}

/// The start and the end of the programs of one run, as the runtime tells them
/// (`EventSink::steps`): the start is recorded as the run event `runbook.action-started` before the
/// program starts and the step is kept as the run's step in flight until the program ends.
struct RunSteps {
    daemon: Daemon,
    run_id: String,
}

impl StartTarget for RunSteps {
    fn started(&self, start: &Started) {
        if self.daemon.is_stopping() {
            return;
        }
        let mut st = self.daemon.st();
        let mut record = Map::new();
        put(&mut record, "case", &start.case);
        put(&mut record, "action", &start.action);
        record.insert("role".into(), json!(start.role));
        if let Some(n) = start.try_number {
            record.insert("try".into(), json!(n));
        }
        // A refusal means the run is closed (a report after a cancel): nothing is in flight then.
        if !matches!(record_event_locked_outcome(&mut st, &self.run_id, ACTION_STARTED, Value::Object(record)), Ok(o) if !o.is_refusal()) {
            return;
        }
        let Some(started_at) = st.last_event_of(&self.run_id).map(|e| e.at.clone()) else { return };
        lock(&self.daemon.inner.steps).insert(self.run_id.clone(), Step { serial: start.serial, case: start.case.clone(), action: start.action.clone(), started_at });
    }

    fn ended(&self, start: &Started) {
        let mut steps = lock(&self.daemon.inner.steps);
        if steps.get(&self.run_id).is_some_and(|s| s.serial == start.serial) {
            steps.remove(&self.run_id);
        }
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
    fn steps(&self) -> Option<Arc<dyn StartTarget>> {
        Some(Arc::new(RunSteps { daemon: self.daemon.clone(), run_id: self.run_id.clone() }))
    }

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

    fn decision_answered(&self, request_id: &str, answer: &str, by: &str, principal: &str) -> bool {
        if self.daemon.is_stopping() || !decider_label(by) || !decider_label(principal) {
            return false;
        }
        let mut st = self.daemon.st();
        match st.decision(request_id) {
            // An answer that names no option of a request read from its case is not taken from a decider either:
            // the request stays open for a person.
            Some(d) if d.run_id == self.run_id && d.state == DecisionState::Open && d.artifact.as_ref().is_none_or(|a| a.takes(&d.options, answer)) => {}
            _ => return false,
        }
        let now = st.now();
        st.commit(Fact::DecisionAnswered { request_id: request_id.to_owned(), answer: answer.to_owned(), answered_by: by.to_owned(), principal: Some(principal.to_owned()), at: now }).is_ok()
    }

    fn resumed(&self) {
        if !self.daemon.is_stopping() {
            let _ = self.daemon.resume_run(&self.run_id);
        }
    }
}

// ---- commands that run with the state already locked ----

/// The refusal of a submit while the daemon drains: a fault of the daemon's own, no outcome of the
/// specification, with the count of the runs it waits for.
fn draining_fault(st: &State) -> Result<(), Fault> {
    if st.draining() {
        return Err(Fault::Draining { live_runs: st.live_run_ids().len() });
    }
    Ok(())
}

fn fail_run_start_locked(st: &mut State, run_id: &str, reason: &str) -> Reply {
    match st.run(run_id).map(|r| r.state) {
        None => Ok(refuse("fail-run-start", "no-run", json!({"run_id": run_id}))),
        Some(RunState::Submitted | RunState::WaitingOnKey | RunState::WaitingForRoute) => {
            record_event_locked(st, run_id, "runbook.start-failed", json!({"reason": reason}))?;
            let now = st.now();
            st.commit(Fact::RunFailedToStart { run_id: run_id.to_string(), reason: reason.to_string(), at: now })?;
            Ok(done("fail-run-start", "failed", vec![("RunFailedToStart", json!({"run_id": run_id, "reason": reason}))]))
        }
        Some(_) => Ok(refuse("fail-run-start", "wrong-state", json!({"run_id": run_id}))),
    }
}

/// `WaitOnKey`: a Submitted run waits behind the live runs of its serial key submitted before it. The run event
/// `runbook.waiting-on-key` names the key and the run ahead of it now.
fn wait_on_key_locked(st: &mut State, run_id: &str) -> Reply {
    match st.run(run_id).map(|r| r.state) {
        None => Ok(refuse("wait-on-key", "no-run", json!({"run_id": run_id}))),
        Some(RunState::Submitted) => {
            let key = st.run(run_id).and_then(|r| r.serial_key.clone());
            let behind = st.ahead_on_key(run_id).map(|r| r.run_id.clone());
            record_event_locked(st, run_id, KEY_WAIT_EVENT, json!({"serial_key": key, "behind": behind}))?;
            st.commit(Fact::RunWaitingOnKey { run_id: run_id.to_string() })?;
            Ok(done("wait-on-key", "waiting-on-key", vec![("RunWaitingOnKey", json!({"run_id": run_id}))]))
        }
        Some(_) => Ok(refuse("wait-on-key", "wrong-state", json!({"run_id": run_id}))),
    }
}

/// `TakeKey`: a run that waited on its serial key takes it and is Submitted again. The run event `runbook.key-taken`
/// names the key.
fn take_key_locked(st: &mut State, run_id: &str) -> Reply {
    match st.run(run_id).map(|r| r.state) {
        None => Ok(refuse("take-key", "no-run", json!({"run_id": run_id}))),
        Some(RunState::WaitingOnKey) => {
            let key = st.run(run_id).and_then(|r| r.serial_key.clone());
            record_event_locked(st, run_id, KEY_TAKEN_EVENT, json!({"serial_key": key}))?;
            st.commit(Fact::RunTookKey { run_id: run_id.to_string() })?;
            Ok(done("take-key", "took-key", vec![("RunTookKey", json!({"run_id": run_id}))]))
        }
        Some(_) => Ok(refuse("take-key", "wrong-state", json!({"run_id": run_id}))),
    }
}

fn end_orphaned_run_locked(st: &mut State, routes: &crate::routes::RouteTable, run_id: &str, reason: &str) -> Reply {
    match st.run(run_id).map(|r| r.state) {
        None => Ok(refuse("end-orphaned-run", "no-run", json!({"run_id": run_id}))),
        Some(state @ (RunState::Running | RunState::WaitingOnDecision)) => {
            // The run's stream says so while the run is still live (a closed run takes no event).
            let record = json!({"run_id": run_id, "state": state.as_str(), "reason": reason});
            record_event_locked(st, run_id, RUN_ORPHANED, record)?;
            let now = st.now();
            // its share of the pod's time of a priced managed route runs to this mark (`pod_uses`)
            if let Some(until) = parse_instant(&now) {
                record_pod_time_locked(st, routes, run_id, until);
            }
            st.commit(Fact::RunEnded { run_id: run_id.to_string(), outcome: ORPHANED.to_string(), at: now })?;
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
    let now = st.now();
    st.commit(Fact::DecisionRequested(DecisionRequest {
        request_id: request.request_id,
        run_id: run_id.to_string(),
        case_id: request.case_id,
        action: request.action,
        capability: request.capability,
        question: request.question,
        options: request.options,
        artifact: request.artifact,
        opened_at: now,
        state: DecisionState::Open,
        answer: None,
        answered_by: None,
        principal: None,
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
    let now = st.now();
    st.commit(Fact::EventRecorded(RunEvent {
        event_id: event_id.clone(),
        run_id: run_id.to_string(),
        session_id,
        sequence,
        position,
        kind: kind.to_string(),
        record,
        at: now,
    }))?;
    Ok(done(
        "record-run-event",
        "recorded",
        vec![("RunEventRecorded", json!({"event_id": event_id, "run_id": run_id, "sequence": sequence, "position": position}))],
    ))
}

// ---- rows ----

/// The row of an open decision request (view `open-decisions`): what it asks, of which step, and, for a
/// request read from its case, the recommendation and the grounds beside the options.
pub fn open_decision_row(d: &DecisionRequest) -> Value {
    let mut m = Map::new();
    m.insert("request_id".into(), json!(d.request_id));
    m.insert("run_id".into(), json!(d.run_id));
    m.insert("case_id".into(), json!(d.case_id));
    m.insert("action".into(), json!(d.action));
    m.insert("capability".into(), json!(d.capability));
    m.insert("question".into(), json!(d.question));
    m.insert("options".into(), json!(d.options));
    put_artifact(&mut m, d);
    m.insert("opened_at".into(), json!(d.opened_at));
    Value::Object(m)
}

/// The member `artifact` of a request read from its case; a request that read nothing has none.
fn put_artifact(m: &mut Map<String, Value>, d: &DecisionRequest) {
    if let Some(a) = &d.artifact {
        m.insert("artifact".into(), json!(a));
    }
}

fn put(m: &mut Map<String, Value>, key: &str, value: &Option<String>) {
    if let Some(v) = value {
        m.insert(key.to_string(), json!(v));
    }
}

fn parse_state(text: &str) -> Result<RunState, Fault> {
    RunState::parse(text).ok_or_else(|| Fault::Invalid(format!("state {text:?} is not one of {}", RunState::ALL.map(RunState::as_str).join(", "))))
}

fn run_row(st: &State, r: &Run) -> Value {
    let mut m = Map::new();
    m.insert("run_id".into(), json!(r.run_id));
    m.insert("runbook_id".into(), json!(r.runbook_id));
    put(&mut m, "runbook_version", &r.runbook_version);
    m.insert("state".into(), json!(r.state));
    m.insert("confinement".into(), json!(r.confinement));
    if let Some(source) = r.confinement_source {
        m.insert("confinement_source".into(), json!(source));
    }
    if let Some(level) = r.terms.autonomy {
        m.insert("autonomy".into(), json!(level));
    }
    put(&mut m, "decider", &r.terms.decider);
    if let Some(route) = &r.model_route {
        m.insert("model_route".into(), json!(route.route));
        m.insert("model_route_alias".into(), json!(route.alias));
        m.insert("model_route_source".into(), json!(route.source));
    }
    put(&mut m, "session_id", &r.session_id);
    put(&mut m, "serial_key", &r.serial_key);
    if let Some(wait) = key_wait(st, r) {
        m.insert("key_wait".into(), json!(wait));
    }
    m.insert("submitted_at".into(), json!(r.submitted_at));
    put(&mut m, "started_at", &r.started_at);
    put(&mut m, "ended_at", &r.ended_at);
    put(&mut m, "outcome", &r.outcome);
    put(&mut m, "failure", &r.failure);
    Value::Object(m)
}

/// What a delegation's run says of its path, from its events: `path` and `protocol` of `runbook.delegated` (none while the run
/// classifies), then `path`, `protocol`, `confidence`, `classifier_route` and `protocol_digest` of `runbook.delegation-resolved`.
/// None for a run no delegation created.
fn delegation_of(events: &[&RunEvent]) -> Option<Map<String, Value>> {
    let delegated = events.iter().find(|e| e.kind == crate::delegation::DELEGATED_EVENT)?;
    let mut m = Map::new();
    if let Some(path) = delegated.record.get("path").filter(|v| !v.is_null()) {
        m.insert("path".into(), path.clone());
        m.insert("protocol".into(), delegated.record["protocol"].clone());
    }
    if let Some(resolved) = events.iter().rev().find(|e| e.kind == crate::delegation::RESOLVED_EVENT) {
        for key in ["path", "protocol", "confidence", "classifier_route", "protocol_digest"] {
            if let Some(v) = resolved.record.get(key).filter(|v| !v.is_null()) {
                m.insert(key.into(), v.clone());
            }
        }
    }
    Some(m)
}

fn run_rows<'a>(st: &State, runs: impl Iterator<Item = &'a Run>) -> Vec<Value> {
    runs.map(|r| run_row(st, r)).collect()
}

/// What a run that waits on its serial key says: `waiting on key <k> behind run <id>`, the run ahead of it now
/// (`State::ahead_on_key`). None for a run in any other state.
fn key_wait(st: &State, r: &Run) -> Option<String> {
    if r.state != RunState::WaitingOnKey {
        return None;
    }
    let key = r.serial_key.as_deref()?;
    let ahead = st.ahead_on_key(&r.run_id)?;
    Some(format!("waiting on key {key} behind run {}", ahead.run_id))
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

/// The member of a submit and a delegation, and of the record of `runbook.submitted`, that adds to the daemon's exclude list.
const WORKSPACE_EXCLUDE: &str = "workspace_exclude";

/// A submit's or a delegation's `workspace_exclude` (`crate::context_copy::check_list`): refused 400 by the member and the
/// entry's place, never by its value, before anything is written.
fn check_workspace_exclude(list: &[String]) -> Result<(), Fault> {
    crate::context_copy::check_list(list).map_err(|why| Fault::Invalid(format!("{WORKSPACE_EXCLUDE}: {why}")))
}
