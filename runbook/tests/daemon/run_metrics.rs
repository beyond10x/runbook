//! Story `the-daemon-serves-a-runs-metrics`: the view `run-metrics` answers what a run took, read from the run's own
//! events: its state, outcome, start, end, the milliseconds it ran, a row per step (one action of one case in one role,
//! over its tries) and the totals of its model calls. A number the events do not hold is absent, never 0.
//!
//! The first cases build the daemon in process on a runtime double that plays a script of reports, with a set clock, so
//! every instant and every number is exact. The last ones run `runbook serve` with the Loom runtime on a stand-in gateway
//! on loopback: a step's program calls the relay once, and the relay books the call. No model is called.

use super::commands::config;
use super::default_confinement::rows;
use super::delegate::{daemon_at, until_ended, ROUTE};
use super::gateway::{claude_cli, model_table};
use super::http::send;
use super::openapi;
use super::relay::gateway_listing;
use super::state::scratch;
use runbook::api::routes::{command_answer, view_answer};
use runbook::clock::SetClock;
use runbook::commands::{Daemon, Policy};
use runbook::runtime::{AnswerFailure, Confinement, EventSink, Finish, Recovery, RunbookInfo, Runtime, RuntimeFailure, StartRequest, Started};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const OK: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nx-llm-gateway-call: gw-call-9\r\nContent-Length: 56\r\nConnection: close\r\n\r\n{\"id\":\"m\",\"usage\":{\"input_tokens\":11,\"output_tokens\":5}}";

/// What a script does with the run: the sink, the run's cancel flag and the clock of the daemon.
type Play = dyn Fn(&dyn EventSink, &AtomicBool, &SetClock) -> Finish + Send + Sync;

/// A runtime that knows every runbook and plays `play` for each run it starts.
struct Scripted {
    clock: Arc<SetClock>,
    play: Box<Play>,
}

impl Runtime for Scripted {
    fn recover(&self, _run_id: &str) -> Recovery {
        Recovery::Gone
    }
    fn knows_runbook(&self, _runbook: &str, _version: Option<&str>) -> bool {
        true
    }
    fn runbooks(&self) -> Vec<RunbookInfo> {
        Vec::new()
    }
    fn kind(&self) -> &'static str {
        "stub"
    }
    fn start(&self, _request: StartRequest, sink: &dyn EventSink, cancel: &AtomicBool) -> Result<Finish, RuntimeFailure> {
        Ok((self.play)(sink, cancel, &self.clock))
    }
    fn answer(&self, _run_id: &str, _request_id: &str, _answer: &str, _principal: &str) -> Result<(), AnswerFailure> {
        Ok(())
    }
    fn cancel(&self, _run_id: &str) {}
}

/// The daemon in process on [`Scripted`], its clock at 10:00:00 of the test's day.
fn daemon(tag: &str, play: Box<Play>) -> (Daemon, Arc<SetClock>) {
    let dir = scratch(tag);
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    let clock = Arc::new(SetClock::at("2026-11-26T10:00:00Z").unwrap());
    let rt = Arc::new(Scripted { clock: clock.clone(), play });
    let policy = Policy { default_confinement: Some(Confinement::Linux), ..Policy::default() };
    (Daemon::start_full(config(&dir), policy, rt, clock.clone()).unwrap(), clock)
}

fn submit(d: &Daemon) -> String {
    let a = command_answer("submit-run", d, json!({"runbook_id": "scripted", "intake": "tea"}).to_string().as_bytes());
    assert_eq!(a.status.as_u16(), 202, "{}", a.body);
    a.body["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned()
}

/// The answer of the view `run-metrics` for `run`: the status and the body.
fn metrics(d: &Daemon, run: &str) -> (u16, Value) {
    let a = view_answer("run-metrics", d, &HashMap::from([("run_id".to_owned(), run.to_owned())]));
    (a.status.as_u16(), a.body)
}

/// The one row of the view for `run`, checked against the row schema of the OpenAPI document.
fn row(d: &Daemon, run: &str) -> Value {
    let (status, body) = metrics(d, run);
    assert_eq!(status, 200, "{body}");
    let rows = body["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "{body}");
    openapi::check_row("MetricsOfRun", &rows[0]);
    rows[0].clone()
}

fn state_of(d: &Daemon, run: &str) -> Value {
    d.run_detail(run)[0]["state"].clone()
}

fn until(what: &str, mut done: impl FnMut() -> bool) {
    let limit = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(Instant::now() < limit, "gave up waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn start(sink: &dyn EventSink, serial: u64, role: &'static str, case: &str, action: &str, try_number: Option<u64>) -> Started {
    let s = Started { serial, role, case: Some(case.to_owned()), action: Some(action.to_owned()), try_number };
    sink.steps().expect("the daemon takes the starts of programs").started(&s);
    s
}

fn end(sink: &dyn EventSink, s: &Started) {
    sink.steps().unwrap().ended(s);
}

/// Loom's line of an action's end as the engine reports it (`b10x_loom_runlog::encode`): its kind is the event's kind.
fn finished(sink: &dyn EventSink, kind: &str, seq: i64, action: &str, exit: i64) {
    sink.record(kind, json!({"seq": seq, "at": "2026-11-26T09:00:00.000Z", "kind": kind, "action": action, "exit": exit, "evidence": [], "revisions": []}));
}

/// A call the relay booked (`runbook.model-call`): the step's name, the tokens the answer reported, whether its cost is
/// known and what it booked.
fn call(sink: &dyn EventSink, step: Option<&str>, tokens: Option<(u64, u64)>, cost_known: bool, micro_usd: u64) {
    let (input, output) = tokens.map_or((Value::Null, Value::Null), |(i, o)| (json!(i), json!(o)));
    sink.record("runbook.model-call", json!({"what": "model call", "run": "r", "step": step, "role": "default", "route": "a", "alias": "alias-a", "source": "run", "wire": "messages", "gateway_call": null, "input_tokens": input, "output_tokens": output, "cost_known": cost_known, "micro_usd": micro_usd}));
}

fn at(clock: &SetClock, time: &str) {
    assert!(clock.set(&format!("2026-11-26T{time}Z")), "{time}");
}

/// A run of three steps: `draft.write` of the case `root` in two tries (the first fails with exit 1) with three relay
/// calls, one of unknown cost and one without tokens; the check of `draft.write`; `review.read` of the case `review-1`,
/// a step of a run on no route whose two calls are one `runbook.cost-booked`; and one call of no step (the decider's).
fn three_steps(sink: &dyn EventSink, _cancel: &AtomicBool, clock: &SetClock) -> Finish {
    at(clock, "10:00:01");
    sink.started();
    at(clock, "10:00:02");
    let s = start(sink, 1, "program", "root", "draft.write", Some(1));
    at(clock, "10:00:03.500");
    call(sink, Some("draft.write"), Some((100, 20)), true, 150);
    call(sink, Some("draft.write"), None, false, 7000);
    finished(sink, "ActionFinished", 4, "draft.write", 1);
    end(sink, &s);
    at(clock, "10:00:05");
    let s = start(sink, 2, "program", "root", "draft.write", Some(2));
    at(clock, "10:00:06.250");
    call(sink, Some("draft.write"), Some((40, 10)), true, 60);
    finished(sink, "ActionFinished", 6, "draft.write", 0);
    end(sink, &s);
    at(clock, "10:00:07");
    let s = start(sink, 3, "check", "root", "draft.write", None);
    at(clock, "10:00:07.100");
    finished(sink, "HandoffChecked", 7, "draft.write", 0);
    end(sink, &s);
    at(clock, "10:00:08");
    let s = start(sink, 4, "program", "review-1", "review.read", Some(1));
    at(clock, "10:00:09");
    sink.record("runbook.cost-booked", json!({"calls": 2, "micro_usd": 500, "spent_micro_usd": 7710}));
    finished(sink, "ActionFinished", 2, "review.read", 0);
    end(sink, &s);
    call(sink, None, Some((5, 1)), true, 3);
    at(clock, "10:00:10");
    Finish::Outcome("answered".into())
}

fn calls(model_calls: u64, input: Option<u64>, output: Option<u64>, without_tokens: u64, cost: u64, unknown: u64, booked: u64) -> Value {
    let mut c = json!({"model_calls": model_calls, "calls_without_tokens": without_tokens, "cost_micro_usd": cost, "unknown_cost_calls": unknown, "booked_micro_usd": booked});
    if let Some(i) = input {
        c["input_tokens"] = json!(i);
    }
    if let Some(o) = output {
        c["output_tokens"] = json!(o);
    }
    c
}

#[test]
fn run_metrics_of_an_ended_run_give_each_step_its_tries_times_exit_calls_tokens_and_cost_and_the_run_its_totals() {
    let (d, _clock) = daemon("metrics-ended", Box::new(three_steps));
    let run = submit(&d);
    until("the run ends", || state_of(&d, &run) == "Ended");
    let r = row(&d, &run);
    let want = json!({
        "run_id": run,
        "run_state": "Ended",
        "outcome": "answered",
        "submitted_at": "2026-11-26T10:00:00.000Z",
        "started_at": "2026-11-26T10:00:01.000Z",
        "ended_at": "2026-11-26T10:00:10.000Z",
        "duration_ms": 9000,
        "steps": [
            {"case_id": "root", "action": "draft.write", "role": "program", "tries": 2, "step_state": "ended",
             "started_at": "2026-11-26T10:00:02.000Z", "ended_at": "2026-11-26T10:00:06.250Z", "exit": 0, "duration_ms": 2750,
             "calls": calls(3, Some(140), Some(30), 1, 210, 1, 7210)},
            {"case_id": "root", "action": "draft.write", "role": "check", "tries": 1, "step_state": "ended",
             "started_at": "2026-11-26T10:00:07.000Z", "ended_at": "2026-11-26T10:00:07.100Z", "exit": 0, "duration_ms": 100,
             "calls": calls(0, Some(0), Some(0), 0, 0, 0, 0)},
            {"case_id": "review-1", "action": "review.read", "role": "program", "tries": 1, "step_state": "ended",
             "started_at": "2026-11-26T10:00:08.000Z", "ended_at": "2026-11-26T10:00:09.000Z", "exit": 0, "duration_ms": 1000,
             "calls": calls(2, None, None, 2, 0, 2, 500)},
        ],
        "steps_left_out": 0,
        "step_limit": 500,
        "totals": calls(6, Some(145), Some(31), 3, 213, 3, 7713),
        "calls_outside_steps": 1,
    });
    assert_eq!(r, want);
    d.stop();
}

#[test]
fn run_metrics_of_a_live_run_say_how_long_it_and_its_step_have_run_and_a_cancel_leaves_the_step_unfinished() {
    let play: Box<Play> = Box::new(|sink, cancel, clock| {
        at(clock, "10:00:01");
        sink.started();
        at(clock, "10:00:02");
        start(sink, 1, "program", "root", "slow.step", Some(1));
        while !cancel.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(2));
        }
        Finish::Cancelled
    });
    let (d, clock) = daemon("metrics-live", play);
    let run = submit(&d);
    until("the step starts", || row(&d, &run)["steps"].as_array().is_some_and(|s| !s.is_empty()));
    at(&clock, "10:00:04.500");
    let r = row(&d, &run);
    assert_eq!((r["run_state"].clone(), r["running_ms"].clone(), r.get("duration_ms"), r.get("ended_at")), (json!("Running"), json!(3500), None, None), "{r}");
    let s = &r["steps"][0];
    assert_eq!((s["step_state"].clone(), s["running_ms"].clone(), s.get("duration_ms"), s.get("ended_at"), s.get("exit")), (json!("running"), json!(2500), None, None, None), "{s}");

    at(&clock, "10:00:05");
    let a = command_answer("cancel-run", &d, json!({"run_id": run}).to_string().as_bytes());
    assert_eq!(a.status.as_u16(), 202, "{}", a.body);
    until("the run is cancelled", || state_of(&d, &run) == "Cancelled");
    let r = row(&d, &run);
    assert_eq!((r["run_state"].clone(), r["ended_at"].clone(), r["duration_ms"].clone(), r.get("running_ms")), (json!("Cancelled"), json!("2026-11-26T10:00:05.000Z"), json!(4000), None), "{r}");
    let s = &r["steps"][0];
    assert_eq!((s["step_state"].clone(), s["tries"].clone(), s.get("running_ms"), s.get("duration_ms"), s.get("ended_at")), (json!("unfinished"), json!(1), None, None, None), "{s}");
    d.stop();
}

/// The bound is the row's own `step_limit`: a run of `step_limit` + 100 steps lists the first `step_limit`, counts the
/// rest, and keeps their calls in the totals; a call of a step left out goes to no listed step of the same action.
#[test]
fn run_metrics_list_at_most_step_limit_steps_and_count_the_rest_with_their_calls_in_the_totals() {
    const STEPS: u64 = 600;
    let play: Box<Play> = Box::new(|sink, _cancel, clock| {
        at(clock, "10:00:01");
        sink.started();
        for i in 0..STEPS {
            let s = start(sink, i + 1, "program", &format!("c-{i}"), "a.write", Some(1));
            call(sink, Some("a.write"), Some((1, 1)), true, 2);
            finished(sink, "ActionFinished", 1, "a.write", 0);
            end(sink, &s);
        }
        Finish::Outcome("done".into())
    });
    let (d, _clock) = daemon("metrics-bound", play);
    let run = submit(&d);
    until("the run ends", || state_of(&d, &run) == "Ended");
    let t = Instant::now();
    let r = row(&d, &run);
    let took = t.elapsed();
    let limit = r["step_limit"].as_u64().unwrap();
    assert!(limit < STEPS, "the case needs more steps than the bound: {limit}");
    let steps = r["steps"].as_array().unwrap();
    assert_eq!((steps.len() as u64, r["steps_left_out"].as_u64()), (limit, Some(STEPS - limit)));
    assert_eq!((steps[0]["case_id"].clone(), steps[limit as usize - 1]["case_id"].clone()), (json!("c-0"), json!(format!("c-{}", limit - 1))));
    assert!(steps.iter().all(|s| s["calls"]["model_calls"] == 1), "a call of a step left out went to a listed one");
    assert_eq!((r["totals"]["model_calls"].as_u64(), r["totals"]["cost_micro_usd"].as_u64(), r["calls_outside_steps"].as_u64()), (Some(STEPS), Some(2 * STEPS), Some(STEPS - limit)));
    assert!(took < Duration::from_secs(1), "the read of {STEPS} steps took {took:?}");
    d.stop();
}

#[test]
fn run_metrics_refuse_an_unknown_run_404_and_a_query_without_a_run_400() {
    let (d, _clock) = daemon("metrics-unknown", Box::new(|_sink, _cancel, _clock| Finish::Outcome("x".into())));
    let (status, body) = metrics(&d, "no-such-run-0001");
    assert_eq!((status, body["refused"].clone()), (404, json!("no such run")), "{body}");
    assert!(body["message"].as_str().unwrap_or_default().contains("no-such-run-0001"), "{body}");
    let a = view_answer("run-metrics", &d, &HashMap::new());
    assert_eq!(a.status.as_u16(), 400, "{}", a.body);
    d.stop();
}

/// The route `default` with prices: 11 input and 5 output tokens cost 11 + 10 micro USD.
const PRICED: &str = "[routes.default]\ngateway = \"default\"\nmodel = \"a-model\"\nkind = \"hosted\"\nprice_in_micro_usd_per_mtok = 1000000\nprice_out_micro_usd_per_mtok = 2000000\n";

/// A delegation that runs `answer-demo` (one program, `answer.write`, that calls the relay once) on the route `default`
/// of `runbook serve`, to its end; the daemon, the run and its metrics row as `GET /daemon/views/run-metrics` answers it.
fn relayed(tag: &str, route: &str) -> (super::default_confinement::Live, Value) {
    let gw = gateway_listing(OK, &["a-model"]);
    let (_h, l, _) = daemon_at(tag, &gw.base_url, &format!("{}{route}", model_table(Some(&claude_cli()))));
    let body = json!({"request": "red or blue", "model_route": "default", "protocol": "answer-demo"});
    let r = send(l.addr, "POST", "/daemon/commands/delegate", &[("Content-Type", "application/json")], Some(body.to_string().as_bytes()));
    assert_eq!(r.status, 202, "{}", r.text());
    let run = r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned();
    let end = until_ended(&l, &run);
    assert_eq!(end["outcome"], "answered", "{end}");
    let rows = rows(&l, &format!("/daemon/views/run-metrics?run_id={run}"));
    assert_eq!(rows.len(), 1, "{rows:?}");
    openapi::check_row("MetricsOfRun", &rows[0]);
    (l, rows[0].clone())
}

#[test]
fn run_metrics_of_a_run_on_a_priced_route_give_its_step_the_relayed_call_with_its_tokens_and_known_cost() {
    let (_l, r) = relayed("metrics-priced", PRICED);
    assert_eq!((r["run_state"].clone(), r["outcome"].clone()), (json!("Ended"), json!("answered")), "{r}");
    assert!(r["duration_ms"].is_u64() && r["started_at"].is_string() && r["ended_at"].is_string(), "{r}");
    let steps = r["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 1, "{r}");
    let s = &steps[0];
    assert_eq!((s["action"].clone(), s["role"].clone(), s["tries"].clone(), s["step_state"].clone(), s["exit"].clone()), (json!("answer.write"), json!("program"), json!(1), json!("ended"), json!(0)), "{s}");
    assert!(s["case_id"].is_string() && s["duration_ms"].is_u64(), "{s}");
    assert_eq!(s["calls"], calls(1, Some(11), Some(5), 0, 21, 0, 21), "{s}");
    assert_eq!((r["totals"].clone(), r["calls_outside_steps"].clone()), (calls(1, Some(11), Some(5), 0, 21, 0, 21), json!(0)), "{r}");
}

#[test]
fn run_metrics_of_a_run_on_a_route_without_prices_count_the_call_of_unknown_cost_apart_and_add_nothing_to_the_known_cost() {
    let (_l, r) = relayed("metrics-unpriced", ROUTE);
    let c = &r["steps"][0]["calls"];
    assert_eq!((c["model_calls"].clone(), c["input_tokens"].clone(), c["output_tokens"].clone(), c["cost_micro_usd"].clone(), c["unknown_cost_calls"].clone()), (json!(1), json!(11), json!(5), json!(0), json!(1)), "{r}");
    assert!(c["booked_micro_usd"].as_u64().is_some_and(|b| b > 0), "the reservation it booked is not said: {r}");
    assert_eq!(r["totals"], *c, "{r}");
}
