//! The view `run-metrics` (`MetricsOfRun` of `ess/domains/daemon.yaml`): what a run took, read from the run's own
//! events in one walk.
//!
//! A step is one action of one case in one role (`program` or `check`), over all its tries. Its starts are the daemon's
//! events `runbook.action-started`; the end of a try is Loom's line `ActionFinished` (a program) or `HandoffChecked` (a
//! check) of the same action, recorded as a run event, and it ends the newest step of that action and role. A relay call
//! (`runbook.model-call`) is the newest step's whose name (its action, else its case) the call's `step` names; the calls
//! of `runbook.cost-booked` (a run on no route) are the newest program step's. A number the events do not hold is left
//! out of the row, never written 0.
//!
//! The walk is linear in the run's events and keeps at most [`STEP_LIMIT`] step rows: a step past it is counted (eight
//! bytes of its key's hash) and its calls stay in the totals.

use crate::clock::parse_instant;
use crate::state::{Run, RunEvent, RunState};
use serde_json::{json, Map, Value};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

/// The most steps a row lists.
pub const STEP_LIMIT: usize = 500;

/// The model calls of a step or of the run.
#[derive(Default)]
struct Calls {
    model_calls: u64,
    input: Option<u64>,
    output: Option<u64>,
    without_tokens: u64,
    cost: u64,
    unknown: u64,
    booked: u64,
}

fn add(sum: &mut Option<u64>, n: Option<u64>) {
    if let Some(n) = n {
        *sum = Some(sum.unwrap_or(0).saturating_add(n));
    }
}

impl Calls {
    /// One call the relay booked (`runbook.model-call`).
    fn relayed(&mut self, r: &Value) {
        let (input, output) = (r["input_tokens"].as_u64(), r["output_tokens"].as_u64());
        let micro_usd = r["micro_usd"].as_u64().unwrap_or(0);
        self.model_calls += 1;
        add(&mut self.input, input);
        add(&mut self.output, output);
        if input.is_none() || output.is_none() {
            self.without_tokens += 1;
        }
        self.booked = self.booked.saturating_add(micro_usd);
        if r["cost_known"].as_bool() == Some(true) {
            self.cost = self.cost.saturating_add(micro_usd);
        } else {
            self.unknown += 1;
        }
    }

    /// The calls of a step booked from its programs' cost records (`runbook.cost-booked`): the event holds no tokens and
    /// does not say whether the cost was known.
    fn booked(&mut self, r: &Value) {
        let calls = r["calls"].as_u64().unwrap_or(0);
        self.model_calls = self.model_calls.saturating_add(calls);
        self.without_tokens = self.without_tokens.saturating_add(calls);
        self.unknown = self.unknown.saturating_add(calls);
        self.booked = self.booked.saturating_add(r["micro_usd"].as_u64().unwrap_or(0));
    }

    fn value(&self) -> Value {
        let mut m = Map::new();
        m.insert("model_calls".into(), json!(self.model_calls));
        // no call: no token; calls none of which reported any: unknown
        for (key, sum) in [("input_tokens", self.input), ("output_tokens", self.output)] {
            if let Some(n) = sum.or((self.model_calls == 0).then_some(0)) {
                m.insert(key.into(), json!(n));
            }
        }
        m.insert("calls_without_tokens".into(), json!(self.without_tokens));
        m.insert("cost_micro_usd".into(), json!(self.cost));
        m.insert("unknown_cost_calls".into(), json!(self.unknown));
        m.insert("booked_micro_usd".into(), json!(self.booked));
        Value::Object(m)
    }
}

/// Where a step stands, by its role, case and action.
type Key = (&'static str, Option<String>, Option<String>);

struct Step {
    role: &'static str,
    case: Option<String>,
    action: Option<String>,
    tries: u64,
    started_at: String,
    /// The start of the last try while it has no end.
    open: Option<String>,
    ended_at: Option<String>,
    exit: Option<i64>,
    /// The milliseconds of the tries that ended; none once a try has no end or an instant does not read.
    ran_ms: Option<i64>,
    calls: Calls,
}

fn between(from: &str, to: &str) -> Option<i64> {
    Some(parse_instant(to)? - parse_instant(from)?)
}

fn role_of(record: &Value) -> Option<&'static str> {
    match record["role"].as_str() {
        Some("program") => Some("program"),
        Some("check") => Some("check"),
        _ => None,
    }
}

fn text(record: &Value, key: &str) -> Option<String> {
    record[key].as_str().map(str::to_owned)
}

fn hashed(key: &Key) -> u64 {
    let mut h = DefaultHasher::new();
    key.hash(&mut h);
    h.finish()
}

#[derive(Default)]
struct Walk {
    steps: Vec<Step>,
    index: HashMap<Key, usize>,
    left_out: HashSet<u64>,
    /// The newest listed program step by its name (its action, else its case): the step a relay call names.
    by_name: HashMap<String, usize>,
    /// The newest listed step by its role and action: the step Loom's end line of that action ends.
    by_action: HashMap<(&'static str, String), usize>,
    /// The newest step that started, when it is listed.
    newest: Option<usize>,
    /// The newest program step that started, when it is listed: the step of `runbook.cost-booked`.
    newest_program: Option<usize>,
    totals: Calls,
    outside: u64,
}

impl Walk {
    fn started(&mut self, e: &RunEvent) {
        let Some(role) = role_of(&e.record) else { return };
        let (case, action) = (text(&e.record, "case"), text(&e.record, "action"));
        let name = action.clone().or_else(|| case.clone());
        let key: Key = (role, case.clone(), action.clone());
        let at = e.at.clone();
        let i = match self.index.get(&key) {
            Some(i) => *i,
            None if self.steps.len() < STEP_LIMIT => {
                self.steps.push(Step { role, case, action: action.clone(), tries: 0, started_at: at.clone(), open: None, ended_at: None, exit: None, ran_ms: Some(0), calls: Calls::default() });
                self.index.insert(key, self.steps.len() - 1);
                self.steps.len() - 1
            }
            None => {
                // past the bound: counted, and no call or end of it goes to an older step of the same name
                self.left_out.insert(hashed(&key));
                if role == "program" {
                    if let Some(n) = &name {
                        self.by_name.remove(n);
                    }
                    self.newest_program = None;
                }
                if let Some(a) = action {
                    self.by_action.remove(&(role, a));
                }
                self.newest = None;
                return;
            }
        };
        let s = &mut self.steps[i];
        if s.open.is_some() {
            // the try before has no end: the time the step ran is not known
            s.ran_ms = None;
        }
        s.tries += 1;
        s.open = Some(at);
        s.ended_at = None;
        s.exit = None;
        if role == "program" {
            if let Some(n) = name {
                self.by_name.insert(n, i);
            }
            self.newest_program = Some(i);
        }
        if let Some(a) = &self.steps[i].action {
            self.by_action.insert((role, a.clone()), i);
        }
        self.newest = Some(i);
    }

    fn ended(&mut self, role: &'static str, e: &RunEvent) {
        let Some(action) = text(&e.record, "action") else { return };
        let Some(&i) = self.by_action.get(&(role, action)) else { return };
        let s = &mut self.steps[i];
        let Some(from) = s.open.take() else { return };
        s.ran_ms = s.ran_ms.zip(between(&from, &e.at)).map(|(a, b)| a.saturating_add(b));
        s.ended_at = Some(e.at.clone());
        s.exit = e.record["exit"].as_i64();
    }

    fn relayed(&mut self, e: &RunEvent) {
        self.totals.relayed(&e.record);
        match e.record["step"].as_str().and_then(|n| self.by_name.get(n)) {
            Some(&i) => self.steps[i].calls.relayed(&e.record),
            None => self.outside += 1,
        }
    }

    fn booked(&mut self, e: &RunEvent) {
        self.totals.booked(&e.record);
        match self.newest_program {
            Some(i) => self.steps[i].calls.booked(&e.record),
            None => self.outside = self.outside.saturating_add(e.record["calls"].as_u64().unwrap_or(0)),
        }
    }
}

/// The row of the view for `run`, from its events in position order, at `now` (milliseconds of the daemon's clock).
pub fn row<'a>(run: &Run, events: impl Iterator<Item = &'a RunEvent>, now: i64) -> Value {
    let mut w = Walk::default();
    for e in events {
        match e.kind.as_str() {
            crate::commands::ACTION_STARTED => w.started(e),
            "ActionFinished" => w.ended("program", e),
            "HandoffChecked" => w.ended("check", e),
            crate::relay::MODEL_CALL_EVENT => w.relayed(e),
            "runbook.cost-booked" => w.booked(e),
            _ => {}
        }
    }
    let running = run.state == RunState::Running;
    let steps: Vec<Value> = w
        .steps
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let mut m = Map::new();
            if let Some(c) = &s.case {
                m.insert("case_id".into(), json!(c));
            }
            if let Some(a) = &s.action {
                m.insert("action".into(), json!(a));
            }
            m.insert("role".into(), json!(s.role));
            m.insert("tries".into(), json!(s.tries));
            let state = match &s.open {
                None => "ended",
                Some(_) if running && w.newest == Some(i) => "running",
                Some(_) => "unfinished",
            };
            m.insert("step_state".into(), json!(state));
            m.insert("started_at".into(), json!(s.started_at));
            if let Some(at) = &s.ended_at {
                m.insert("ended_at".into(), json!(at));
            }
            if let Some(exit) = s.exit {
                m.insert("exit".into(), json!(exit));
            }
            match (&s.open, s.ran_ms) {
                (None, Some(ms)) => {
                    m.insert("duration_ms".into(), json!(ms.max(0)));
                }
                (Some(from), _) if state == "running" => {
                    if let Some(ms) = parse_instant(from).map(|f| now - f) {
                        m.insert("running_ms".into(), json!(ms.max(0)));
                    }
                }
                _ => {}
            }
            m.insert("calls".into(), w.steps[i].calls.value());
            Value::Object(m)
        })
        .collect();
    let mut m = Map::new();
    m.insert("run_id".into(), json!(run.run_id));
    m.insert("run_state".into(), json!(run.state));
    if let Some(o) = &run.outcome {
        m.insert("outcome".into(), json!(o));
    }
    m.insert("submitted_at".into(), json!(run.submitted_at));
    if let Some(at) = &run.started_at {
        m.insert("started_at".into(), json!(at));
    }
    if let Some(at) = &run.ended_at {
        m.insert("ended_at".into(), json!(at));
    }
    let started = run.started_at.as_deref().and_then(parse_instant);
    match (started, &run.ended_at) {
        (Some(from), Some(to)) => {
            if let Some(to) = parse_instant(to) {
                m.insert("duration_ms".into(), json!((to - from).max(0)));
            }
        }
        (Some(from), None) if run.state.is_live() => {
            m.insert("running_ms".into(), json!((now - from).max(0)));
        }
        _ => {}
    }
    m.insert("steps".into(), json!(steps));
    m.insert("steps_left_out".into(), json!(w.left_out.len()));
    m.insert("step_limit".into(), json!(STEP_LIMIT));
    m.insert("totals".into(), w.totals.value());
    m.insert("calls_outside_steps".into(), json!(w.outside));
    Value::Object(m)
}
