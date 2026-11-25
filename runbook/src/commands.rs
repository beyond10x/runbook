//! The one command layer. HTTP, and later MCP and the CLI, call only this: one function per client
//! command and per view of `ess/domains/daemon.yaml`, and one per runtime-side command the runtime's
//! reports map to. Every outcome and refusal has the specification's name, and the guards run in
//! the specification's order.
//!
//! The daemon mints run ids (UUID version 7) and fills `daemon_id`, the time and the facts it reads
//! from the host (`runbook_known`, `context_inside_roots`); a client never chooses them.

use crate::clock::{parse_instant, Clock, SystemClock};
use crate::runtime::{AnswerFailure, Autonomy, Confinement, ConfinementSource, DecisionRequest as RuntimeRequest, EventSink, Finish, Recovery, Runtime, StartRequest, StartTarget, Started, Terms};
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
/// The reason of `runbook.run-orphaned` when the runtime holds no run of the id at the next start.
pub const ORPHANED_AT_START: &str = "the daemon started again and the runtime holds no live run of this id";
/// The reason of `runbook.run-orphaned` when the daemon ends while the run lives (a second stop signal,
/// or the drain bound): the run does not wait for the next start to be orphaned.
pub const ORPHANED_AT_STOP: &str = "the daemon was stopped while the run was live";
/// The longest client label and idempotency key, in characters.
pub const MAX_LABEL_CHARS: usize = 200;
/// The kind of the run event the daemon records when a program of a run is about to start.
pub const ACTION_STARTED: &str = "runbook.action-started";

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
    /// token files read; empty: no route. The view `routes` lists them.
    pub routes: Arc<crate::routes::RouteTable>,
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
    #[serde(default, deserialize_with = "settings_of_strings")]
    pub settings: BTreeMap<String, String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

/// A run its command admitted: what `Daemon::enter` writes and starts.
struct Admitted {
    run_id: String,
    runbook_id: String,
    runbook_version: Option<String>,
    intake: String,
    context_path: Option<String>,
    context_dir: Option<PathBuf>,
    settings: BTreeMap<String, String>,
    confinement: Confinement,
    source: ConfinementSource,
    terms: Terms,
    session_id: Option<String>,
    idempotency_key: Option<String>,
    model_route: Option<String>,
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
        let state = State::open_with_clock(&cfg.state_dir, clock)?;
        let routes = crate::routes::Listing::new(policy.routes.clone());
        let d = Daemon {
            inner: Arc::new(Inner {
                daemon_id: uuid::Uuid::now_v7().to_string(),
                cfg,
                policy,
                routes,
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
    /// event that says the daemon was stopped while it ran, and its Open decision requests are withdrawn.
    /// No thread is joined: `stop` does that. The number of runs that were live and are ended now.
    pub fn orphan_live_runs(&self) -> usize {
        self.inner.stopping.store(true, Ordering::SeqCst);
        let live = self.live_runs();
        for run_id in &live {
            let _ = self.end_orphaned_run_because(run_id, ORPHANED_AT_STOP);
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
        let out = end_orphaned_run_locked(&mut st, run_id, reason)?;
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
        if input.idempotency_key.as_deref().is_some_and(|k| !long_enough(k)) {
            return Err(Fault::Invalid(format!("idempotency_key is longer than {MAX_LABEL_CHARS} characters")));
        }
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
            settings,
            confinement,
            source,
            terms,
            session_id: input.session_id,
            idempotency_key: input.idempotency_key,
            model_route: None,
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
            session_id: a.session_id,
            idempotency_key: a.idempotency_key,
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
        record_event_locked(&mut st, &run_id, "runbook.submitted", record)?;
        if let Some(d) = delegated {
            record_event_locked(&mut st, &run_id, crate::delegation::DELEGATED_EVENT, d)?;
        }
        drop(st);

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
            model_route: a.model_route,
        };
        self.spawn_run(request);
        Ok(())
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
        if input.idempotency_key.as_deref().is_some_and(|k| !long_enough(k)) {
            return Err(Fault::Invalid(format!("idempotency_key is longer than {MAX_LABEL_CHARS} characters")));
        }
        draining_fault(&self.st())?;
        let terms = Terms { autonomy: self.inner.policy.autonomy, decider: self.inner.policy.decider.clone() };
        check_terms(&terms, &self.inner.policy.kinds).map_err(Fault::Invalid)?;
        let confinement = self.inner.policy.default_confinement.ok_or_else(|| {
            Fault::Invalid("a delegation runs at the daemon's default confinement level and this daemon has none: start it with --default-confinement or a `confinement` in runbook.toml".to_owned())
        })?;
        let run_id = uuid::Uuid::now_v7().to_string();
        let refused = |name: &'static str, payload: Value| -> Reply { Ok(refuse("delegate", name, payload)) };
        // The facts the specification takes from the host. A delegation that names no route takes the daemon's default route; one
        // without either is refused as one that names an undeclared route.
        let routes = self.inner.runtime.routes();
        let named = input.model_route.clone().or_else(|| self.inner.policy.routes.default_route().map(str::to_owned));
        let route = named.as_deref().filter(|r| routes.iter().any(|d| d == r));
        let (runbook_id, path) = match &input.protocol {
            Some(p) => (p.clone(), "named"),
            None => (crate::delegation::FREE_RUNBOOK.to_owned(), "free"),
        };
        let runbook_known = match &input.protocol {
            Some(p) => self.runbook_known(p, None),
            None => self.inner.runtime.free_run(),
        };
        let context = input.workspace.as_deref().map(|p| self.context_under_roots(p));

        if input.request.len() > MAX_INTAKE_BYTES {
            return refused("intake-too-large", json!({"run_id": run_id}));
        }
        let Some(route) = route.map(str::to_owned) else {
            let mut payload = json!({"run_id": run_id, "routes": routes});
            if let Some(named) = &named {
                payload["route"] = json!(shown(named));
            }
            return refused("route-refused", payload);
        };
        if !runbook_known {
            return refused("no-runbook", json!({"run_id": run_id}));
        }
        if matches!(context, Some(None)) {
            return refused("context-outside-roots", json!({"run_id": run_id}));
        }
        let context_dir = context.flatten();
        let mut settings = input.settings.clone();
        match self.inner.runtime.resolve_caps(&settings) {
            Ok(Some(caps)) => caps.put(&mut settings),
            Ok(None) => {}
            Err(why) => return Err(Fault::Invalid(why)),
        }
        if input.protocol.is_some() {
            self.inner.runtime.check_runbook(&runbook_id, None).map_err(Fault::Invalid)?;
        }
        let replay = input.idempotency_key.as_deref().is_some_and(|k| self.st().run_by_key(k).is_some());
        if !replay {
            if let Some(why) = self.inner.runtime.route_unready(&route) {
                return Err(Fault::Refused { status: 503, what: "route-unavailable", message: why });
            }
        }

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
                let mut out = done("delegate", "already-submitted", vec![("SubmitReplayed", json!({"idempotency_key": key, "run_id": first}))]);
                out.run_id = Some(first);
                return Ok(out);
            }
        }
        let submitted = json!({"run_id": run_id, "runbook_id": runbook_id, "confinement": confinement});
        let delegated = json!({"run_id": run_id, "route": route, "path": path, "protocol": runbook_id});
        let admitted = Admitted {
            run_id: run_id.clone(),
            runbook_id: runbook_id.clone(),
            runbook_version: None,
            intake: input.request,
            context_path: input.workspace,
            context_dir,
            settings,
            confinement,
            source: ConfinementSource::DaemonDefault,
            terms,
            session_id: input.session_id,
            idempotency_key: input.idempotency_key,
            model_route: Some(route.clone()),
        };
        self.enter(st, admitted, Some(json!({"route": route, "path": path, "protocol": runbook_id})))?;
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
            Some(RunState::Submitted) => {
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
                let now = st.now();
                st.commit(Fact::RunEnded { run_id: run_id.to_string(), outcome: outcome.to_string(), at: now })?;
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
        // What the run has used is the runtime's to say; asked before the state is locked.
        let used = self.inner.runtime.usage(run_id);
        let st = self.st();
        st.run(run_id)
            .map(|r| {
                let mut row = run_row(r);
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
            put(&mut m, "failure", &run.failure);
            for key in ["route", "path", "protocol"] {
                m.insert(key.into(), delegated.record[key].clone());
            }
            m.insert("confidence".into(), Value::Null);
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

    /// The view `CasesOfRun` (wire name `cases`): a row per case of the run's tree, as the runtime reads it
    /// (`Runtime::cases`), read after the state is let go. Where a case stands (`case_state`): `Ended` with an outcome;
    /// else `AwaitingPerson` or `AwaitingAuthority` when its last stop says so and the run waits on a decision; else
    /// `Stuck` when its last stop is `Stuck` or `BudgetSpent` and the run ended by its tree (`Ended`, not orphaned: a wait
    /// or a cancel that ended with the run, or a run the daemon orphaned, leaves the case `Open`); else `Running` when the
    /// run's step in flight names the case; else `Open`.
    pub fn cases_of_run(&self, run_id: &str) -> Result<Vec<Value>, Fault> {
        let (state, step_case, ended_by_its_tree) = {
            let st = self.st();
            let run = st.run(run_id).ok_or_else(|| Fault::no_such_run(run_id))?;
            let step = if run.state == RunState::Running { lock(&self.inner.steps).get(run_id).and_then(|s| s.case.clone()) } else { None };
            (run.state, step, run.state == RunState::Ended && run.outcome.as_deref() != Some("orphaned"))
        };
        let cases = self.inner.runtime.cases(run_id).map_err(|_| Fault::Refused { status: 503, what: "tree not readable", message: format!("the tree of run {} cannot be read at this moment: ask again", crate::engine::products::said(run_id)) })?;
        Ok(cases
            .into_iter()
            .map(|c| {
                let case_state = match (&c.outcome, c.stop.as_deref()) {
                    (Some(_), _) => "Ended",
                    (None, Some(stop @ ("AwaitingPerson" | "AwaitingAuthority"))) if state == RunState::WaitingOnDecision => stop,
                    (None, Some("Stuck" | "BudgetSpent")) if ended_by_its_tree => "Stuck",
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
    /// names the gateway by name and holds no address, token or price.
    pub fn routes(&self) -> Vec<Value> {
        let now = self.st().now_millis();
        self.inner.routes.rows(now)
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
                    // A run stopped while it waited on decisions (a cap, also one reached while the
                    // last answer was taken, with nothing left open): what it asked is withdrawn and
                    // it ends from Running.
                    self.leave_wait(&run_id);
                    let _ = self.end_run(&run_id, &outcome);
                }
                // CancelRun moved the run already.
                Ok(Finish::Cancelled) => {}
                Err(f) if sink.started.load(Ordering::SeqCst) => {
                    let _ = self.record_run_event(&run_id, "runbook.runtime-failed", json!({"reason": f.reason}));
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
        Some(RunState::Submitted) => {
            record_event_locked(st, run_id, "runbook.start-failed", json!({"reason": reason}))?;
            let now = st.now();
            st.commit(Fact::RunFailedToStart { run_id: run_id.to_string(), reason: reason.to_string(), at: now })?;
            Ok(done("fail-run-start", "failed", vec![("RunFailedToStart", json!({"run_id": run_id, "reason": reason}))]))
        }
        Some(_) => Ok(refuse("fail-run-start", "wrong-state", json!({"run_id": run_id}))),
    }
}

fn end_orphaned_run_locked(st: &mut State, run_id: &str, reason: &str) -> Reply {
    match st.run(run_id).map(|r| r.state) {
        None => Ok(refuse("end-orphaned-run", "no-run", json!({"run_id": run_id}))),
        Some(state @ (RunState::Running | RunState::WaitingOnDecision)) => {
            // The run's stream says so while the run is still live (a closed run takes no event).
            let record = json!({"run_id": run_id, "state": state.as_str(), "reason": reason});
            record_event_locked(st, run_id, "runbook.run-orphaned", record)?;
            let now = st.now();
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

fn run_row(r: &Run) -> Value {
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
