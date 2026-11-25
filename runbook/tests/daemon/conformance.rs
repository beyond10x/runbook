//! The conformance suite of the ESS system `sdc`, run against the real daemon.
//!
//! The suite is `ess/generated/conformance/authored.json`: the authored scenarios, compiled by
//! `ess verify conform author` and committed (`task check:ess-suite` compares it byte for byte,
//! `task ess-suite:authored` writes it); this test only reads it. The synthesized suite
//! (`suite.json`, `task ess-suite:record`, ess 0.97.3) is committed beside it with a digest of its
//! sources (`task check:ess-suite:synthesized`); the gate below does not read it, see the story
//! `daemon-synthesized-suite-has-no-refusal`. `CONFORMANCE_SUITE=<file> CONFORMANCE_ONLY=sdc.daemon`
//! runs any suite document, the synthesized one included, and prints its counts without the gate;
//! `CONFORMANCE_REPORT=<file>` writes the verdicts. `ess verify conform run --target process` could drive the same suite through an
//! adapter process, but it needs an `ess` on the path and a second program; this module is the runner
//! instead, in the crate's one test binary, and reads the suite document step by step.
//!
//! The target:
//! - the five client commands are `POST /daemon/commands/<name>` and every view is
//!   `GET /daemon/views/<name>` over a real socket;
//! - the runtime-side commands (StartRun, FailRunStart, WaitOnDecision, RequestDecision,
//!   WithdrawDecision, ResumeRun, RecordRunEvent, EndRun) are not on the network (the specification
//!   says so): they are the calls of the command layer that a runtime's reports map to, made on the
//!   same daemon by a runtime double (`Manual`) that takes a run up and then reports nothing on its own;
//! - StartDaemon is the start of the process: the daemon is started with the floor the scenario's first
//!   StartDaemon names, and the step is witnessed through the configuration row, never by a flag. The
//!   daemon states its default level as the operator would (`--default-confinement`): the floor, because a
//!   daemon has no built-in level and StartDaemon has no member for one, so the scenario of a submit without
//!   a level (the handler fills the daemon's default) is asked of a daemon that states one;
//! - facts the specification takes from the host (`runbook_known`, `context_inside_roots`) are made true
//!   or false in the host: the runtime double knows the runbook or not, the context is a directory under
//!   the context root or a path outside it; `route_refused` is the refusal of the route the step names in
//!   `model_route`, which the daemon (it declares no route) refuses;
//! - time: the daemon reads one clock (`runbook::clock`); the runner builds it with a `SetClock` and moves it to
//!   the `at` of each command step before the step runs, so the instants the daemon records are the ones the
//!   scenario names and a claim of an exact instant is read like any other (compared as an instant, so
//!   `...:00Z` and `...:00.000Z` are equal). The clock is set only here: `Daemon::start_with_clock` is called
//!   by test code, never by `serve`;
//! - ids the daemon mints (run ids, the daemon id) are bound to the literal the scenario used for them
//!   the first time an event of the answer names them.
//!
//! What it cannot ask is `skipped`, with the reason; it is never `passed`:
//! - an actor the daemon does not know (the daemon authenticates no actor, so a command is asked as the
//!   actor the specification grants it to, and a scenario that asks as another is skipped);
//! - a step of a kind this runner does not read.
//!
//! The gate: `GATE` below holds the committed numbers and `KNOWN_FAILURES` the failures by name.

use super::http::send;
use runbook::api::{self, routes};
use runbook::clock::{parse_instant, SetClock};
use runbook::commands::{Config, Daemon, Outcome, Policy, SubmitRun};
use runbook::runtime::{AnswerFailure, Confinement, DecisionRequest as RtRequest, EventSink, Finish, Recovery, RunbookInfo, Runtime, RuntimeFailure, StartRequest};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The committed numbers of the daemon's scenarios: a run may not execute fewer, pass fewer or skip
/// more. Raise `executed` and `passed` (and lower `skipped`) in the commit that makes them move.
struct Gate {
    total: usize,
    executed: usize,
    passed: usize,
    skipped: usize,
}

/// The principal of the daemon the scenarios run against: the `principal` of their `StartDaemon` steps.
const CONFORMANCE_PRINCIPAL: &str = "agent:conformance";

const GATE: Gate = Gate { total: 17, executed: 17, passed: 17, skipped: 0 };

/// Scenarios that fail today, by name, with the reason: a different failure cannot hide behind them.
const KNOWN_FAILURES: &[(&str, &str)] = &[];

#[derive(Clone, Debug, PartialEq)]
enum Verdict {
    Passed,
    Failed(String),
    Skipped(String),
}

enum Stop {
    Fail(String),
    Skip(String),
}

type Res<T> = Result<T, Stop>;

fn fail<T>(m: impl Into<String>) -> Res<T> {
    Err(Stop::Fail(m.into()))
}

fn skip<T>(m: impl Into<String>) -> Res<T> {
    Err(Stop::Skip(m.into()))
}

// ---- the runtime double ----

/// A runtime that knows the runbooks the scenario made known and takes a run up without reporting:
/// the scenario's steps are the runtime's reports.
struct Manual {
    known: Mutex<BTreeSet<String>>,
}

impl Runtime for Manual {
    fn recover(&self, _run_id: &str) -> Recovery {
        Recovery::Gone
    }

    fn knows_runbook(&self, runbook: &str, _version: Option<&str>) -> bool {
        self.known.lock().unwrap().contains(runbook)
    }

    fn runbooks(&self) -> Vec<RunbookInfo> {
        self.known.lock().unwrap().iter().map(|name| RunbookInfo { name: name.clone(), version: None }).collect()
    }

    fn kind(&self) -> &'static str {
        "stub"
    }

    fn start(&self, _request: StartRequest, _sink: &dyn EventSink, cancel: &AtomicBool) -> Result<Finish, RuntimeFailure> {
        while !cancel.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(Finish::Cancelled)
    }

    fn answer(&self, _run_id: &str, _request_id: &str, _answer: &str, _principal: &str) -> Result<(), AnswerFailure> {
        Ok(())
    }

    fn cancel(&self, _run_id: &str) {}
}

// ---- one daemon on a socket ----

struct Live {
    daemon: Daemon,
    addr: SocketAddr,
    root: PathBuf,
    rt: Option<tokio::runtime::Runtime>,
}

impl Drop for Live {
    fn drop(&mut self) {
        self.daemon.stop();
        if let Some(rt) = self.rt.take() {
            rt.shutdown_background();
        }
    }
}

struct Answer {
    outcome: String,
    error: Option<String>,
    events: Vec<(String, Value)>,
}

fn answer_of_body(body: &Value) -> Option<Answer> {
    let outcome = body.get("outcome")?.as_str()?.to_string();
    let error = body.get("error").and_then(Value::as_str).map(String::from);
    let events = body
        .get("published")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(|e| (e["event"].as_str().unwrap_or_default().to_string(), e["payload"].clone())).collect())
        .unwrap_or_default();
    Some(Answer { outcome, error, events })
}

fn answer_of_outcome(o: &Outcome) -> Answer {
    answer_of_body(&o.body()).expect("an outcome has a name")
}

fn kebab(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn view_wire(view: &str) -> Option<&'static str> {
    Some(match view {
        "Sessions" => "sessions",
        "DaemonConfiguration" => "configuration",
        "Runs" => "runs",
        "RunsOfSession" => "runs-of-session",
        "RunsInState" => "runs-in-state",
        "RunsOfSessionInState" => "runs-of-session-in-state",
        "RunDetail" => "run",
        "OpenDecisionsOfRun" => "open-decisions",
        "DecisionsOfRun" => "decisions",
        "EventsAfter" => "events",
        "EventsOfSessionAfter" => "session-events",
        "EventsOfDaemonAfter" => "daemon-events",
        _ => return None,
    })
}

fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The actors the specification grants a command to, as the daemon can be asked: the daemon
/// authenticates no actor, so a command is asked as an actor it is granted to and no other.
fn granted_to(command: &str) -> &'static [&'static str] {
    match command {
        "OpenSession" | "CloseSession" | "SubmitRun" | "CancelRun" | "AnswerDecision" => &["sdc.daemon.Client"],
        "StartDaemon" => &["sdc.daemon.Operator"],
        "EndOrphanedRun" | "WaitForRoute" | "EndRouteWait" => &["sdc.daemon.DaemonProcess"],
        "WithdrawDecision" => &["sdc.daemon.Runtime"],
        _ => &["sdc.daemon.Runtime"],
    }
}

/// Equal as the suite reads values: an absent member and `null` are one value.
fn same(want: &Value, got: &Value) -> bool {
    match (want, got) {
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
        (Value::String(a), Value::String(b)) => a == b || matches!((parse_instant(a), parse_instant(b)), (Some(x), Some(y)) if x == y),
        (Value::Array(a), Value::Array(b)) => a.len() == b.len() && a.iter().zip(b).all(|(x, y)| same(x, y)),
        (Value::Object(a), Value::Object(b)) => {
            a.keys().chain(b.keys()).collect::<BTreeSet<_>>().iter().all(|k| same(a.get(*k).unwrap_or(&Value::Null), b.get(*k).unwrap_or(&Value::Null)))
        }
        _ => want == got,
    }
}

/// A whole number the suite wrote as `1` or as `1.0`.
fn uint(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| v.as_f64().filter(|f| f.fract() == 0.0 && *f >= 0.0).map(|f| f as u64))
}

/// A view parameter as the query string spells it: text as it is, a whole number without `.0`.
fn param_text(v: &Value) -> String {
    match (v.as_str(), uint(v)) {
        (Some(s), _) => s.to_string(),
        (None, Some(n)) => n.to_string(),
        _ => v.to_string(),
    }
}

fn text(m: &Map<String, Value>, key: &str) -> Res<String> {
    match m.get(key).and_then(Value::as_str) {
        Some(s) => Ok(s.to_string()),
        None => skip(format!("the input has no text `{key}`: {:?}", m.get(key))),
    }
}

// ---- one scenario ----

struct Scn {
    dir: PathBuf,
    floor: Confinement,
    runtime: Arc<Manual>,
    clock: Arc<SetClock>,
    live: Option<Live>,
    started_by_step: bool,
    binds: BTreeMap<String, String>,
    captured: BTreeMap<String, Value>,
    events: Vec<(String, Value)>,
    last: Option<Answer>,
    rows: BTreeMap<String, Vec<Value>>,
    snaps: BTreeMap<String, (Map<String, Value>, Option<Value>)>,
    run_literal: Option<String>,
}

impl Scn {
    fn new(dir: PathBuf, floor: Confinement) -> Scn {
        Scn {
            dir,
            floor,
            runtime: Arc::new(Manual { known: Mutex::new(BTreeSet::new()) }),
            clock: Arc::new(SetClock::at_millis(0)),
            live: None,
            started_by_step: false,
            binds: BTreeMap::new(),
            captured: BTreeMap::new(),
            events: Vec::new(),
            last: None,
            rows: BTreeMap::new(),
            snaps: BTreeMap::new(),
            run_literal: None,
        }
    }

    fn live(&mut self) -> Res<&Live> {
        if self.live.is_none() {
            let root = self.dir.join("ctx");
            std::fs::create_dir_all(&root).map_err(|e| Stop::Fail(e.to_string()))?;
            let root = std::fs::canonicalize(root).map_err(|e| Stop::Fail(e.to_string()))?;
            let cfg = Config { state_dir: self.dir.join("state"), confinement_floor: self.floor, context_roots: vec![root.clone()], runbook_dir: None, principal: CONFORMANCE_PRINCIPAL.into() };
            let daemon = Daemon::start_full(cfg, Policy { default_confinement: Some(self.floor), ..Policy::default() }, self.runtime.clone(), self.clock.clone()).map_err(|e| Stop::Fail(format!("the daemon did not start: {e}")))?;
            let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().map_err(|e| Stop::Fail(e.to_string()))?;
            let listener = rt.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).map_err(|e| Stop::Fail(e.to_string()))?;
            let addr = listener.local_addr().map_err(|e| Stop::Fail(e.to_string()))?;
            let served = daemon.clone();
            rt.spawn(async move {
                let _ = api::serve(listener, served, None, std::future::pending()).await;
            });
            self.live = Some(Live { daemon, addr, root, rt: Some(rt) });
        }
        Ok(self.live.as_ref().unwrap())
    }

    fn http(&mut self, method: &str, path: &str, body: Option<&Value>) -> Res<(u16, Value)> {
        let addr = self.live()?.addr;
        let bytes = body.map(|b| b.to_string());
        let headers: &[(&str, &str)] = if bytes.is_some() { &[("Content-Type", "application/json")] } else { &[] };
        let r = send(addr, method, path, headers, bytes.as_deref().map(str::as_bytes));
        let json = serde_json::from_slice(&r.body).unwrap_or(Value::Null);
        Ok((r.status, json))
    }

    // values

    fn resolve(&self, v: &Value) -> Res<Value> {
        match v.get("kind").and_then(Value::as_str) {
            Some("literal") => Ok(self.bound(&v["value"])),
            Some("instance") => {
                let name = v["instance"].as_str().unwrap_or_default();
                match self.captured.get(name) {
                    Some(x) => Ok(x.clone()),
                    None => fail(format!("the instance {name} was never captured")),
                }
            }
            Some("observed") => {
                let (event, field) = (v["event"].as_str().unwrap_or_default(), v["field"].as_str().unwrap_or_default());
                match self.events.iter().rev().find(|(e, _)| e == event) {
                    Some((_, p)) => Ok(p.get(field).cloned().unwrap_or(Value::Null)),
                    None => fail(format!("no {event} was published, so {field} cannot be observed")),
                }
            }
            other => skip(format!("a value of kind {}", other.unwrap_or("none"))),
        }
    }

    /// The literal a scenario used for an id the daemon minted, replaced by the minted id.
    fn bound(&self, v: &Value) -> Value {
        match v {
            Value::String(s) => self.binds.get(s).map_or_else(|| v.clone(), |r| Value::String(r.clone())),
            Value::Array(a) => Value::Array(a.iter().map(|x| self.bound(x)).collect()),
            Value::Object(m) => Value::Object(m.iter().map(|(k, x)| (k.clone(), self.bound(x))).collect()),
            other => other.clone(),
        }
    }

    fn values(&self, map: &Value) -> Res<Map<String, Value>> {
        let mut out = Map::new();
        for (k, v) in map.as_object().into_iter().flatten() {
            out.insert(k.clone(), self.resolve(v)?);
        }
        Ok(out)
    }

    fn publish(&mut self, a: Answer) {
        self.events.extend(a.events.iter().cloned());
        self.last = Some(a);
    }

    // commands

    fn execute(&mut self, step: &Value) -> Res<()> {
        let qualified = step["command"].as_str().unwrap_or_default();
        let Some(command) = qualified.strip_prefix("sdc.daemon.") else {
            return skip(format!("{qualified} is not a command of the daemon"));
        };
        let actor = step["actor"].as_str().unwrap_or_default();
        if !granted_to(command).contains(&actor) {
            return skip(format!("{command} asked as {actor}: the daemon authenticates no actor, it can only be asked as {}", granted_to(command).join(" or ")));
        }
        let input = self.values(&step["input"])?;
        // The daemon stamps the instant its clock stands at: a step that names one (`at`) is run at it.
        if let Some(at) = input.get("at").and_then(Value::as_str) {
            if !self.clock.set(at) {
                return fail(format!("the step names `at` {at:?}, which is no RFC 3339 instant in UTC"));
            }
        }
        let answer = self.command(command, &input)?;
        self.publish(answer);
        Ok(())
    }

    fn post(&mut self, command: &str, body: Map<String, Value>) -> Res<Answer> {
        let (status, json) = self.http("POST", &routes::command_path(&kebab(command)), Some(&Value::Object(body)))?;
        match answer_of_body(&json) {
            Some(a) => Ok(a),
            None => fail(format!("{command}: status {status} with no outcome: {json}")),
        }
    }

    fn library(&self, command: &str, reply: runbook::commands::Reply) -> Res<Answer> {
        match reply {
            Ok(o) => Ok(answer_of_outcome(&o)),
            Err(f) => fail(format!("{command}: the command layer refused the input as a fault: {f:?}")),
        }
    }

    fn command(&mut self, command: &str, input: &Map<String, Value>) -> Res<Answer> {
        let pick = |keys: &[&str]| -> Map<String, Value> { keys.iter().filter_map(|k| input.get(*k).filter(|v| !v.is_null()).map(|v| (k.to_string(), v.clone()))).collect() };
        match command {
            "OpenSession" => self.post(command, pick(&["session_id", "client"])),
            "CloseSession" => self.post(command, pick(&["session_id"])),
            "CancelRun" => self.post(command, pick(&["run_id"])),
            "AnswerDecision" => self.post(command, pick(&["run_id", "request_id", "answer", "answered_by"])),
            "SubmitRun" => self.submit(input),
            "StartDaemon" => self.start_daemon(input),
            "StartRun" => {
                let run = text(input, "run_id")?;
                let r = self.live()?.daemon.start_run(&run);
                self.library(command, r)
            }
            "FailRunStart" => {
                let (run, reason) = (text(input, "run_id")?, text(input, "reason")?);
                let r = self.live()?.daemon.fail_run_start(&run, &reason);
                self.library(command, r)
            }
            "WaitOnDecision" => {
                let run = text(input, "run_id")?;
                let r = self.live()?.daemon.wait_on_decision(&run);
                self.library(command, r)
            }
            "ResumeRun" => {
                let run = text(input, "run_id")?;
                let r = self.live()?.daemon.resume_run(&run);
                self.library(command, r)
            }
            "WithdrawDecision" => {
                let request = text(input, "request_id")?;
                let r = self.live()?.daemon.withdraw_decision(&request);
                self.library(command, r)
            }
            "EndRun" => {
                let (run, outcome) = (text(input, "run_id")?, text(input, "outcome")?);
                let r = self.live()?.daemon.end_run(&run, &outcome);
                self.library(command, r)
            }
            "EndOrphanedRun" => {
                let run = text(input, "run_id")?;
                let r = self.live()?.daemon.end_orphaned_run(&run);
                self.library(command, r)
            }
            "WaitForRoute" => {
                let run = text(input, "run_id")?;
                let phase = input.get("phase").and_then(Value::as_str).map(str::to_owned);
                let r = self.live()?.daemon.wait_for_route(&run, phase.as_deref());
                self.library(command, r)
            }
            "EndRouteWait" => {
                let run = text(input, "run_id")?;
                let phase = input.get("phase").and_then(Value::as_str).map(str::to_owned);
                let r = self.live()?.daemon.end_route_wait(&run, None, phase.as_deref());
                self.library(command, r)
            }
            "RequestDecision" => {
                let run = text(input, "run_id")?;
                let request = RtRequest {
                    request_id: text(input, "request_id")?,
                    case_id: text(input, "case_id")?,
                    action: text(input, "action")?,
                    capability: text(input, "capability")?,
                    question: text(input, "question")?,
                    options: input.get("options").and_then(Value::as_array).cloned().unwrap_or_default(),
                    artifact: input.get("artifact").filter(|v| !v.is_null()).and_then(|v| serde_json::from_value(v.clone()).ok()),
                };
                let r = self.live()?.daemon.request_decision(&run, request);
                self.library(command, r)
            }
            "RecordRunEvent" => {
                let run = text(input, "run_id")?;
                let (Some(sequence), Some(position)) = (input.get("sequence").and_then(uint), input.get("position").and_then(uint).or_else(|| self.next_position())) else {
                    return skip("RecordRunEvent without a sequence");
                };
                let record = input.get("record").cloned().unwrap_or(Value::Null);
                let r = self.live()?.daemon.record_run_event_numbered(&run, sequence, position, &text(input, "kind")?, record);
                self.library(command, r)
            }
            other => skip(format!("the command {other} has no route")),
        }
    }

    /// The next position of the daemon: the handler fills it, so a scenario that names none gets it from the events read.
    fn next_position(&self) -> Option<u64> {
        let live = self.live.as_ref()?;
        Some(live.daemon.events_of_daemon_after(0).iter().filter_map(|e| e["position"].as_u64()).max().unwrap_or(0) + 1)
    }

    fn start_daemon(&mut self, input: &Map<String, Value>) -> Res<Answer> {
        let floor_name = text(input, "confinement_floor")?;
        let Some(floor) = Confinement::parse(&floor_name) else { return skip(format!("the floor {floor_name} is not a level of the daemon")) };
        let given = text(input, "daemon_id")?;
        if self.started_by_step {
            let real = self.live()?.daemon.daemon_id().to_string();
            let r = self.live()?.daemon.start_daemon(&real, floor);
            return self.library("StartDaemon", r);
        }
        self.started_by_step = true;
        let rows = {
            let live = self.live()?;
            live.daemon.daemon_configuration()
        };
        let Some(row) = rows.first() else { return fail("the daemon holds no configuration row after its start") };
        if row["confinement_floor"].as_str() != Some(floor.as_str()) {
            return fail(format!("the daemon was started with the floor {floor_name} and holds {}", row["confinement_floor"]));
        }
        let principal = text(input, "principal")?;
        if row["principal"].as_str() != Some(principal.as_str()) {
            return fail(format!("the daemon was started with the principal {principal} and holds {}", row["principal"]));
        }
        let id = row["daemon_id"].as_str().unwrap_or_default().to_string();
        self.binds.insert(given, id.clone());
        Ok(Answer { outcome: "started".into(), error: None, events: vec![("sdc.daemon.DaemonStarted".into(), json!({"daemon_id": id, "confinement_floor": floor_name}))] })
    }

    fn submit(&mut self, input: &Map<String, Value>) -> Res<Answer> {
        let runbook = text(input, "runbook_id")?;
        let known = input.get("runbook_known").and_then(Value::as_bool).unwrap_or(true);
        {
            let mut k = self.runtime.known.lock().unwrap();
            if known {
                k.insert(runbook.clone());
            } else {
                k.remove(&runbook);
            }
        }
        let mut body = Map::new();
        for k in ["runbook_id", "runbook_version", "intake", "settings", "confinement", "session_id", "idempotency_key", "model_route", "model_routes"] {
            if let Some(v) = input.get(k).filter(|v| !v.is_null()) {
                body.insert(k.to_string(), v.clone());
            }
        }
        let inside = input.get("context_inside_roots").and_then(Value::as_bool).unwrap_or(true);
        if !inside {
            body.insert("context_path".into(), json!("/nonexistent-outside-the-roots/context"));
        } else if let Some(name) = input.get("context_path").and_then(Value::as_str) {
            let root = self.live()?.root.clone();
            let dir = root.join(name.trim_matches('/').replace('/', "_"));
            std::fs::create_dir_all(&dir).map_err(|e| Stop::Fail(e.to_string()))?;
            self.binds.insert(name.to_string(), dir.to_string_lossy().into_owned());
            body.insert("context_path".into(), json!(dir.to_string_lossy()));
        }
        let wanted = input.get("daemon_id").and_then(Value::as_str).map(String::from);
        let real = self.live()?.daemon.daemon_id().to_string();
        let answer = match wanted {
            Some(d) if d != real => {
                let parsed: SubmitRun = serde_json::from_value(Value::Object(body)).map_err(|e| Stop::Fail(format!("the submit does not parse: {e}")))?;
                let r = self.live()?.daemon.submit_run_for(&d, parsed);
                self.library("SubmitRun", r)?
            }
            _ => self.post("SubmitRun", body)?,
        };
        if let (Some(literal), Some(run)) = (self.run_literal.clone(), answer.events.iter().find(|(e, _)| e == "sdc.daemon.RunSubmitted").and_then(|(_, p)| p["run_id"].as_str())) {
            self.binds.insert(literal, run.to_string());
        }
        Ok(answer)
    }

    // observations

    fn query(&mut self, view: &str, params: &Map<String, Value>) -> Res<Vec<Value>> {
        let short = view.strip_prefix("sdc.daemon.").unwrap_or(view);
        let Some(wire) = view_wire(short) else { return skip(format!("the view {view} is not a view of the daemon")) };
        let query: Vec<String> = params
            .iter()
            .map(|(k, v)| format!("{k}={}", encode(&param_text(v))))
            .collect();
        let mut path = routes::view_path(wire);
        if !query.is_empty() {
            path = format!("{path}?{}", query.join("&"));
        }
        let (status, body) = self.http("GET", &path, None)?;
        if status != 200 {
            return fail(format!("{view}: status {status}: {body}"));
        }
        Ok(body["rows"].as_array().cloned().unwrap_or_default())
    }

    fn claims(&mut self, fields: &Value) -> Res<Map<String, Value>> {
        let mut out = Map::new();
        for (k, v) in fields.as_object().into_iter().flatten() {
            let wrapped = v.get("kind").and_then(Value::as_str).is_some_and(|k| matches!(k, "literal" | "instance" | "observed"));
            let value = if wrapped { self.resolve(v)? } else { self.bound(v) };
            out.insert(k.clone(), value);
        }
        Ok(out)
    }

    fn matches(row: &Value, claims: &Map<String, Value>) -> bool {
        claims.iter().all(|(k, want)| same(want, row.get(k).unwrap_or(&Value::Null)))
    }

    fn expect_view(&mut self, step: &Value) -> Res<()> {
        let view = step["view"].as_str().unwrap_or_default().to_string();
        if !self.rows.contains_key(&view) {
            let rows = self.query(&view, &Map::new())?;
            self.rows.insert(view.clone(), rows);
        }
        let ex = &step["expectation"];
        match ex["expect"].as_str() {
            Some("contains") => {
                let claims = self.claims(&ex["fields"])?;
                let rows = &self.rows[&view];
                if !rows.iter().any(|r| Self::matches(r, &claims)) {
                    return fail(format!("{view} holds no row with {}; rows: {}", Value::Object(claims), Value::Array(rows.clone())));
                }
            }
            Some("excludes") => {
                let claims = self.claims(&ex["fields"])?;
                if let Some(r) = self.rows[&view].iter().find(|r| Self::matches(r, &claims)) {
                    return fail(format!("{view} holds a row it should not: {r}"));
                }
            }
            Some("counts") => {
                let n = self.rows[&view].len() as u64;
                if let Some(min) = ex["at_least"].as_u64() {
                    if n < min {
                        return fail(format!("{view} holds {n} rows, at least {min} expected"));
                    }
                }
                if let Some(max) = ex["at_most"].as_u64() {
                    if n > max {
                        return fail(format!("{view} holds {n} rows, at most {max} expected"));
                    }
                }
                if let Some(exact) = ex["exactly"].as_u64() {
                    if n != exact {
                        return fail(format!("{view} holds {n} rows, {exact} expected"));
                    }
                }
            }
            Some("at") => {
                // The first or the last row in the order the expectation names (`position asc`).
                let place = ex["position"]["row"].as_str().unwrap_or_default();
                if !matches!(place, "first" | "last") {
                    return skip(format!("a view expectation `at` the row {place:?}"));
                }
                let claims = self.claims(&ex["fields"])?;
                let mut rows = self.rows[&view].clone();
                for key in ex["order_by"].as_array().into_iter().flatten().filter_map(Value::as_str).rev() {
                    let mut parts = key.split_whitespace();
                    let (field, descending) = (parts.next().unwrap_or_default().to_string(), parts.next() == Some("desc"));
                    rows.sort_by(|a, b| {
                        let order = match (a[&field].as_u64(), b[&field].as_u64()) {
                            (Some(x), Some(y)) => x.cmp(&y),
                            _ => a[&field].to_string().cmp(&b[&field].to_string()),
                        };
                        if descending { order.reverse() } else { order }
                    });
                }
                let row = if place == "first" { rows.first() } else { rows.last() };
                match row {
                    Some(r) if Self::matches(r, &claims) => {}
                    Some(r) => return fail(format!("{view}: the {place} row is {r}, expected {}", Value::Object(claims))),
                    None => return fail(format!("{view} holds no row, the {place} one expected with {}", Value::Object(claims))),
                }
            }
            other => return skip(format!("a view expectation `{}`", other.unwrap_or("none"))),
        }
        Ok(())
    }

    fn event_claims(&mut self, step: &Value) -> Res<()> {
        let event = step["event"].as_str().unwrap_or_default().to_string();
        let claims = self.claims(&step["payload"])?;
        let Some(last) = &self.last else { return fail("an event is claimed before any command") };
        let Some((_, payload)) = last.events.iter().find(|(e, _)| *e == event) else {
            return fail(format!("{event} was not published; published: {:?}", last.events.iter().map(|(e, _)| e).collect::<Vec<_>>()));
        };
        for (k, want) in &claims {
            let got = payload.get(k).unwrap_or(&Value::Null);
            if !same(want, got) {
                return fail(format!("{event}.{k}: expected {want}, published {got}"));
            }
        }
        Ok(())
    }

    fn snapshot(&mut self, step: &Value) -> Res<()> {
        let view = step["view"].as_str().unwrap_or_default().to_string();
        let key = self.values(&step["subject"])?;
        let rows = self.query(&view, &Map::new())?;
        let row = rows.into_iter().find(|r| Self::matches(r, &key));
        self.snaps.insert(view, (key, row));
        Ok(())
    }

    fn unchanged(&mut self, step: &Value) -> Res<()> {
        let view = step["view"].as_str().unwrap_or_default().to_string();
        let Some((key, before)) = self.snaps.get(&view).cloned() else { return fail(format!("no snapshot of {view}")) };
        let rows = self.query(&view, &Map::new())?;
        let after = rows.into_iter().find(|r| Self::matches(r, &key));
        match (before, after) {
            (None, None) => Ok(()),
            (Some(b), Some(a)) if same(&b, &a) => Ok(()),
            (b, a) => fail(format!("{view}: the subject changed: before {b:?}, after {a:?}")),
        }
    }

    fn step(&mut self, step: &Value) -> Res<()> {
        match step["step"].as_str().unwrap_or_default() {
            "execute_command" => self.execute(step),
            "expect_outcome" => {
                let want = step["outcome"]["outcome"].as_str().unwrap_or_default();
                let Some(last) = &self.last else { return fail("an outcome is claimed before any command") };
                if last.outcome == want {
                    Ok(())
                } else {
                    fail(format!("{} answered {} and not {want}{}", step["outcome"]["command"], last.outcome, last.error.as_ref().map(|e| format!(" ({e})")).unwrap_or_default()))
                }
            }
            "expect_no_error" => match self.last.as_ref().and_then(|l| l.error.clone()) {
                Some(e) => fail(format!("an error {e} was answered")),
                None => Ok(()),
            },
            "expect_error" => {
                let want = step["error"].as_str().unwrap_or_default();
                match self.last.as_ref().and_then(|l| l.error.clone()) {
                    Some(e) if e == want => Ok(()),
                    other => fail(format!("the error {want} was claimed, answered {other:?}")),
                }
            }
            "expect_event" | "expect_event_values" => self.event_claims(step),
            "expect_no_event" => {
                let event = step["event"].as_str().unwrap_or_default();
                match self.last.as_ref().and_then(|l| l.events.iter().find(|(e, _)| e == event)) {
                    Some(_) => fail(format!("{event} was published and must not be")),
                    None => Ok(()),
                }
            }
            "expect_no_events" => match self.last.as_ref().map(|l| l.events.len()) {
                Some(0) | None => Ok(()),
                Some(n) => fail(format!("{n} events were published and none may be")),
            },
            "capture_instance" => {
                let (name, event, field) = (step["instance"].as_str().unwrap_or_default(), step["event"].as_str().unwrap_or_default(), step["field"].as_str().unwrap_or_default());
                let found = self.last.as_ref().and_then(|l| l.events.iter().find(|(e, _)| e == event)).and_then(|(_, p)| p.get(field).cloned());
                match found {
                    Some(v) => {
                        self.captured.insert(name.to_string(), v);
                        Ok(())
                    }
                    None => fail(format!("{event} published no {field} to capture")),
                }
            }
            "query_view" => {
                let view = step["view"].as_str().unwrap_or_default().to_string();
                let params = self.values(&step["params"])?;
                let rows = self.query(&view, &params)?;
                self.rows.insert(view, rows);
                Ok(())
            }
            "expect_view" => self.expect_view(step),
            "snapshot_subject" | "snapshot_complete_subject" => self.snapshot(step),
            "expect_subject_unchanged" | "expect_complete_subject_unchanged" => self.unchanged(step),
            "expect_not_granted" => skip("the daemon authenticates no actor: a refusal for an actor without the grant cannot be asked"),
            other => skip(format!("a step of kind {other}")),
        }
    }

    fn run(&mut self, steps: &[Value]) -> Verdict {
        for s in steps {
            if s["step"] == "execute_command" && s["command"] == "sdc.daemon.SubmitRun" {
                self.run_literal = s["input"]["run_id"]["value"].as_str().filter(|_| s["input"]["run_id"]["kind"] == "literal").map(String::from);
            }
            match self.step(s) {
                Ok(()) => {}
                Err(Stop::Fail(m)) => return Verdict::Failed(m),
                Err(Stop::Skip(m)) => return Verdict::Skipped(m),
            }
        }
        Verdict::Passed
    }
}

// ---- the suite ----

fn suite_path() -> PathBuf {
    if let Ok(p) = std::env::var("CONFORMANCE_SUITE") {
        return PathBuf::from(p);
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ess/generated/conformance/authored.json")
}

fn load_suite() -> Value {
    let raw = std::fs::read_to_string(suite_path()).unwrap_or_else(|e| panic!("{}: {e}", suite_path().display()));
    serde_json::from_str(&raw).expect("the committed suite is JSON")
}

/// The floor the scenario's first StartDaemon names, else the daemon's default.
fn floor_of(steps: &[Value]) -> Confinement {
    steps
        .iter()
        .find(|s| s["step"] == "execute_command" && s["command"] == "sdc.daemon.StartDaemon")
        .and_then(|s| s["input"]["confinement_floor"]["value"].as_str())
        .and_then(Confinement::parse)
        .unwrap_or(Confinement::Linux)
}

struct Report {
    verdicts: Vec<(String, Verdict, Duration)>,
    out_of_domain: usize,
}

fn run_suite(tag: &str, only: Option<&str>) -> Report {
    let suite = load_suite();
    let base = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("conformance-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let mut verdicts = Vec::new();
    let mut out_of_domain = 0;
    for (n, (id, scenario)) in suite["scenarios"].as_object().expect("scenarios").iter().enumerate() {
        if !id.starts_with("sdc.daemon") {
            out_of_domain += 1;
            continue;
        }
        if only.is_some_and(|o| !id.contains(o)) {
            continue;
        }
        let steps = scenario["steps"].as_array().cloned().unwrap_or_default();
        let t = Instant::now();
        let dir = base.join(n.to_string());
        let verdict = {
            let mut scn = Scn::new(dir.clone(), floor_of(&steps));
            scn.run(&steps)
        };
        let _ = std::fs::remove_dir_all(&dir);
        verdicts.push((id.clone(), verdict, t.elapsed()));
    }
    let _ = std::fs::remove_dir_all(&base);
    Report { verdicts, out_of_domain }
}

impl Report {
    fn count(&self, f: impl Fn(&Verdict) -> bool) -> usize {
        self.verdicts.iter().filter(|(_, v, _)| f(v)).count()
    }

    fn passed(&self) -> usize {
        self.count(|v| *v == Verdict::Passed)
    }

    fn failed(&self) -> usize {
        self.count(|v| matches!(v, Verdict::Failed(_)))
    }

    fn skipped(&self) -> usize {
        self.count(|v| matches!(v, Verdict::Skipped(_)))
    }

    fn json(&self) -> Value {
        let mut scenarios = Map::new();
        for (id, v, t) in &self.verdicts {
            let (status, reason) = match v {
                Verdict::Passed => ("passed", String::new()),
                Verdict::Failed(m) => ("failed", m.clone()),
                Verdict::Skipped(m) => ("skipped", m.clone()),
            };
            scenarios.insert(id.clone(), json!({"status": status, "reason": reason, "millis": t.as_millis() as u64}));
        }
        json!({
            "total": self.verdicts.len(),
            "executed": self.passed() + self.failed(),
            "passed": self.passed(),
            "failed": self.failed(),
            "skipped": self.skipped(),
            "out_of_domain": self.out_of_domain,
            "scenarios": scenarios,
        })
    }
}

#[test]
fn the_committed_suite_holds_its_counts_against_the_daemon() {
    let only = std::env::var("CONFORMANCE_ONLY").ok();
    let t = Instant::now();
    let report = run_suite("gate", only.as_deref());
    let elapsed = t.elapsed();
    let summary = report.json();
    if let Ok(path) = std::env::var("CONFORMANCE_REPORT") {
        std::fs::write(path, serde_json::to_string_pretty(&summary).unwrap()).unwrap();
    }
    let mut slowest: Vec<_> = report.verdicts.iter().map(|(id, _, t)| (t.as_millis(), id.clone())).collect();
    slowest.sort_by(|a, b| b.cmp(a));
    println!(
        "conformance of sdc.daemon: total {} executed {} passed {} failed {} skipped {} (out of domain: {}) in {:.1}s; slowest: {:?}",
        report.verdicts.len(),
        report.passed() + report.failed(),
        report.passed(),
        report.failed(),
        report.skipped(),
        report.out_of_domain,
        elapsed.as_secs_f64(),
        slowest.iter().take(3).collect::<Vec<_>>()
    );
    for (id, v, _) in &report.verdicts {
        match v {
            Verdict::Failed(m) => println!("failed  {id}: {m}"),
            Verdict::Skipped(m) => println!("skipped {id}: {m}"),
            Verdict::Passed => {}
        }
    }
    if only.is_some() {
        return;
    }
    assert!(slowest.first().is_none_or(|(ms, _)| *ms < 60_000), "a scenario took over 60 s");

    let failed: BTreeSet<&str> = report.verdicts.iter().filter(|(_, v, _)| matches!(v, Verdict::Failed(_))).map(|(id, _, _)| id.as_str()).collect();
    let known: BTreeSet<&str> = KNOWN_FAILURES.iter().map(|(id, _)| *id).collect();
    assert_eq!(failed, known, "the failing scenarios are not the known ones: a different failure cannot hide behind a known one, and a fixed one leaves the list");
    assert!(report.verdicts.len() >= GATE.total, "the suite has fewer scenarios than the committed {}", GATE.total);
    assert!(report.passed() + report.failed() >= GATE.executed, "executed fell below the committed {}", GATE.executed);
    assert!(report.passed() >= GATE.passed, "passed fell below the committed {}", GATE.passed);
    assert_eq!(report.skipped(), GATE.skipped, "skipped differs from the committed {}", GATE.skipped);
}
