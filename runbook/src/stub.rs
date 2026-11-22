//! The stub runtime: a scripted run described by data. The table of scripts is filled by the caller
//! (the tests, or `runbook serve` for the presets) and chosen by the runbook name of the submit.

use crate::runtime::{AnswerFailure, DecisionRequest, EventSink, Finish, Recovery, Runtime, RuntimeFailure, StartRequest};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// One decision request of an `Ask` step.
#[derive(Clone, Debug)]
pub struct Ask {
    pub request_id: String,
    pub case_id: String,
    pub action: String,
    pub capability: String,
    pub question: String,
    pub options: Vec<Value>,
}

/// `{run}` and `{intake}` in a text of an ask are the run's id and intake; every other text stays.
fn fill(text: &str, request: &StartRequest) -> String {
    text.replace("{run}", &request.run_id).replace("{intake}", &request.intake)
}

fn fill_value(v: &Value, request: &StartRequest) -> Value {
    match v {
        Value::String(s) => Value::String(fill(s, request)),
        Value::Array(a) => Value::Array(a.iter().map(|x| fill_value(x, request)).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), fill_value(x, request))).collect()),
        other => other.clone(),
    }
}

impl Ask {
    /// The ask of one run: the request id must be unique per daemon, so a script that runs twice names it with `{run}`.
    fn filled(&self, request: &StartRequest) -> Ask {
        Ask {
            request_id: fill(&self.request_id, request),
            case_id: self.case_id.clone(),
            action: self.action.clone(),
            capability: self.capability.clone(),
            question: fill(&self.question, request),
            options: self.options.iter().map(|o| fill_value(o, request)).collect(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Step {
    /// Emit a run-log record with a kind and a body.
    Emit { kind: String, body: Value },
    /// Ask these decisions and wait until every one is answered.
    Ask(Vec<Ask>),
    /// Sleep this many milliseconds (the cancel flag is looked at while it runs).
    Sleep(u64),
    /// End with this outcome.
    End(String),
    /// Fail to start (when it is the first step), or fail after the start (later).
    FailToStart(String),
}

#[derive(Clone, Debug)]
pub struct Script {
    pub version: Option<String>,
    pub steps: Vec<Step>,
}

impl Script {
    pub fn new(steps: Vec<Step>) -> Script {
        Script { version: None, steps }
    }

    pub fn version(mut self, version: &str) -> Script {
        self.version = Some(version.to_string());
        self
    }
}

/// The outcome a script that has no `End` step ends with.
pub const DEFAULT_OUTCOME: &str = "done";

#[derive(Default)]
pub struct StubRuntime {
    table: Mutex<HashMap<String, Script>>,
    answers: Mutex<HashMap<String, Sender<String>>>,
}

const POLL: Duration = Duration::from_millis(2);

impl StubRuntime {
    pub fn new() -> StubRuntime {
        StubRuntime::default()
    }

    /// Put a script in the table under a runbook name.
    pub fn script(&self, runbook: &str, script: Script) {
        self.table.lock().unwrap().insert(runbook.to_string(), script);
    }

    /// The script `runbook serve` gives a preset: one record, a short sleep, the outcome `done`.
    pub fn demo_script() -> Script {
        Script::new(vec![
            Step::Emit { kind: "stub.note".into(), body: serde_json::json!({"note": "scripted run of the stub runtime"}) },
            Step::Sleep(20),
            Step::End(DEFAULT_OUTCOME.into()),
        ])
    }

    /// The script `runbook serve` gives the preset `stub-decision`: two decision requests at once (the
    /// question is the run's intake), a wait until both are answered, a record and the outcome
    /// `approved`. It is the stub's way to hold a run in `WaitingOnDecision` for a client to answer.
    pub fn decision_script() -> Script {
        let option = |letter: &str, does: &str| serde_json::json!({"letter": letter, "does": does, "cost": "none", "consequence": {"admits": [], "blocks": []}});
        let ask = |n: u8, question: &str| Ask {
            request_id: format!("{{run}}-q{n}"),
            case_id: "case-1".into(),
            action: "stub.approve".into(),
            capability: "approve".into(),
            question: question.into(),
            options: vec![option("A", "yes: {intake}"), option("B", "no: {intake}")],
        };
        Script::new(vec![
            Step::Emit { kind: "stub.note".into(), body: serde_json::json!({"note": "scripted run of the stub runtime, waiting on two decisions"}) },
            Step::Ask(vec![ask(1, "{intake}"), ask(2, "Second look: {intake}")]),
            Step::Emit { kind: "stub.answered".into(), body: serde_json::json!({"note": "both decisions were answered"}) },
            Step::End("approved".into()),
        ])
    }

    fn sleep(&self, ms: u64, cancel: &AtomicBool) -> bool {
        let until = Instant::now() + Duration::from_millis(ms);
        while Instant::now() < until {
            if cancel.load(Ordering::SeqCst) {
                return false;
            }
            std::thread::sleep(POLL.min(until.saturating_duration_since(Instant::now())));
        }
        !cancel.load(Ordering::SeqCst)
    }

    fn ask(&self, request: &StartRequest, asks: &[Ask], sink: &dyn EventSink, cancel: &AtomicBool) -> bool {
        let run_id = request.run_id.as_str();
        let (tx, rx) = channel::<String>();
        self.answers.lock().unwrap().insert(run_id.to_string(), tx);
        let asks: Vec<Ask> = asks.iter().map(|a| a.filled(request)).collect();
        let mut pending: HashSet<String> = asks.iter().map(|a| a.request_id.clone()).collect();
        for a in asks {
            sink.decision_requested(DecisionRequest {
                request_id: a.request_id,
                case_id: a.case_id,
                action: a.action,
                capability: a.capability,
                question: a.question,
                options: a.options,
            });
        }
        let mut go_on = true;
        while !pending.is_empty() {
            if cancel.load(Ordering::SeqCst) {
                go_on = false;
                break;
            }
            match rx.recv_timeout(POLL) {
                Ok(id) => {
                    pending.remove(&id);
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        self.answers.lock().unwrap().remove(run_id);
        if go_on {
            sink.resumed();
        }
        go_on
    }
}

impl Runtime for StubRuntime {
    /// A scripted run lives on one thread of the process: after a restart none is left.
    fn recover(&self, _run_id: &str) -> Recovery {
        Recovery::Gone
    }

    fn knows_runbook(&self, runbook: &str, version: Option<&str>) -> bool {
        match self.table.lock().unwrap().get(runbook) {
            None => false,
            Some(s) => match (version, &s.version) {
                (None, _) => true,
                (Some(v), Some(have)) => v == have,
                (Some(_), None) => true,
            },
        }
    }

    fn start(&self, request: StartRequest, sink: &dyn EventSink, cancel: &AtomicBool) -> Result<Finish, RuntimeFailure> {
        let script = self.table.lock().unwrap().get(&request.runbook).cloned();
        let Some(script) = script else {
            return Err(RuntimeFailure { reason: format!("the stub has no script for runbook {}", request.runbook) });
        };
        if let Some(Step::FailToStart(reason)) = script.steps.first() {
            return Err(RuntimeFailure { reason: reason.clone() });
        }
        sink.started();
        for step in &script.steps {
            if cancel.load(Ordering::SeqCst) {
                return Ok(Finish::Cancelled);
            }
            match step {
                Step::Emit { kind, body } => sink.record(kind, body.clone()),
                Step::Sleep(ms) => {
                    if !self.sleep(*ms, cancel) {
                        return Ok(Finish::Cancelled);
                    }
                }
                Step::Ask(asks) => {
                    if !self.ask(&request, asks, sink, cancel) {
                        return Ok(Finish::Cancelled);
                    }
                }
                Step::End(outcome) => return Ok(Finish::Outcome(outcome.clone())),
                Step::FailToStart(reason) => return Err(RuntimeFailure { reason: reason.clone() }),
            }
        }
        Ok(Finish::Outcome(DEFAULT_OUTCOME.to_string()))
    }

    fn answer(&self, run_id: &str, request_id: &str, _answer: &str, _principal: &str) -> Result<(), AnswerFailure> {
        match self.answers.lock().unwrap().get(run_id) {
            Some(tx) => tx.send(request_id.to_string()).map_err(|_| AnswerFailure::UnknownRun),
            None => Err(AnswerFailure::UnknownRun),
        }
    }

    fn cancel(&self, _run_id: &str) {}
}
