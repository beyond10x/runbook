//! Adversary cases of story `the-routes-view-answers-the-days-wake-figures`: the day's wake figures of a route row of the
//! view `routes` (and the catalog, and the MCP tool `list_routes`) read against the story's Outcome, its rule of
//! `wake_needed` and its Acceptance.
//!
//! In process, on the daemon's test clock, with the stub runtime and the stand-in gateway of `route_wake.rs` on a
//! loopback port of the test's own: no model call, nothing leaves loopback.

use super::route_wake::{
    answer, daemon_in, daemon_with, figures, figures_of, priced, records, route_row, submit, submit_held, until_records, until_state, until_wakes, wake_answer_lines, wake_on, want,
    Gateway, WakeSaid, FIGURES,
};
use runbook::caps::{Caps, Usage};
use runbook::clock::SetClock;
use runbook::commands::{CancelRun, Daemon, Delegate, Policy};
use runbook::routes::{RouteTable, Tables};
use runbook::runtime::{AnswerFailure, CaseRow, EventSink, Finish, ProgramSteps, Recovery, RunbookFacts, RunbookInfo, Runtime, RuntimeFailure, StartRequest};
use runbook::runtime::Confinement;
use runbook::stub::{Script, Step, StubRuntime};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// The answer `spend-ceiling` of the gateway to a wake.
fn ceiling_body() -> Value {
    json!({"error": {"code": "spend-ceiling", "message": "a pod start would pass the daily cap"}})
}

/// Story Outcome: `wake_needed` is "a run waits on the route and the daemon sends no wake for it", true only when "no wake
/// of the route is out". A managed route whose `start_wait_seconds` (2) is shorter than the time its gateway takes to
/// answer a wake (3 s here; the daemon waits up to 5 s for that answer): while the wake the run sent is still out, the
/// row answers `wake_needed: true`, though the daemon did send a wake for the waiting run and it is out right now.
#[test]
fn adversary_route_figures_a_wake_still_out_past_a_short_wait_does_not_make_wake_needed_true() {
    let gw = Gateway::start_with("stopped", vec![WakeSaid::Held(202, json!({"model": "qwen", "phase": "starting"}))], (202, json!({})));
    let dir = super::state::scratch("adv-fig-short-wait");
    let clock = Arc::new(SetClock::at("2026-11-26T10:00:00Z").unwrap());
    let mut wake = wake_on(500, 100);
    wake["start_wait_seconds"] = json!(2);
    let d = daemon_in(&dir, &gw, wake, clock.clone());
    let run = submit(&d, "qwen");
    until_wakes(&gw, 1);
    // the gateway has not answered yet: the wake is out, 3 s of the daemon's clock after it was sent
    clock.set("2026-11-26T10:00:03Z");
    assert_eq!(records(&d, &run, "runbook.route-wake-answered"), Vec::<Value>::new(), "the wake was answered already");
    let row = route_row(&d, "qwen");
    let state = super::route_wake::detail(&d, &run)["state"].clone();
    gw.let_go(1);
    assert_eq!(state, "WaitingForRoute");
    assert_eq!(row["phase"], "stopped", "{row}");
    assert_eq!(row["wake_booked_micro_usd"], 100, "the wake out is booked, so the view itself counts it as out: {row}");
    assert_eq!(row["wake_needed"], false, "a wake of the route is out and the row answers wake_needed true: {row}");
}

/// A runtime that classifies a delegation that names no protocol (`Runtime::classifies`), and is the stub otherwise: the
/// delegation's run reads the daemon's classifier route with its own (`route_wait`, `also`).
struct Classifying(Arc<StubRuntime>);

impl Runtime for Classifying {
    fn recover(&self, run_id: &str) -> Recovery {
        self.0.recover(run_id)
    }
    fn knows_runbook(&self, runbook: &str, version: Option<&str>) -> bool {
        self.0.knows_runbook(runbook, version)
    }
    fn runbooks(&self) -> Vec<RunbookInfo> {
        self.0.runbooks()
    }
    fn kind(&self) -> &'static str {
        self.0.kind()
    }
    fn runbook_facts(&self, runbook: &str) -> RunbookFacts {
        self.0.runbook_facts(runbook)
    }
    fn routes_file(&self, runbook: &str, version: Option<&str>) -> Option<Result<String, String>> {
        self.0.routes_file(runbook, version)
    }
    fn start(&self, request: StartRequest, sink: &dyn EventSink, cancel: &AtomicBool) -> Result<Finish, RuntimeFailure> {
        self.0.start(request, sink, cancel)
    }
    fn answer(&self, run_id: &str, request_id: &str, answer: &str, principal: &str) -> Result<(), AnswerFailure> {
        self.0.answer(run_id, request_id, answer, principal)
    }
    fn cancel(&self, run_id: &str) {
        self.0.cancel(run_id)
    }
    fn resolve_caps(&self, settings: &BTreeMap<String, String>) -> Result<Option<Caps>, String> {
        self.0.resolve_caps(settings)
    }
    fn usage(&self, run_id: &str) -> Option<Usage> {
        self.0.usage(run_id)
    }
    fn program_steps(&self, runbook: &str, version: Option<&str>) -> ProgramSteps {
        self.0.program_steps(runbook, version)
    }
    fn serves_routes(&self) -> bool {
        self.0.serves_routes()
    }
    fn check_runbook(&self, runbook: &str, version: Option<&str>) -> Result<(), String> {
        self.0.check_runbook(runbook, version)
    }
    fn cases(&self, run_id: &str) -> Result<Vec<CaseRow>, String> {
        self.0.cases(run_id)
    }
    fn free_run(&self) -> bool {
        self.0.free_run()
    }
    fn classifies(&self) -> bool {
        true
    }
}

/// A daemon on `gw` as `route_wake::daemon_in` builds it, with `qwen` (managed, the wake keys `wake`) as the classifier
/// route of `[route_policy]`, the decider kind `model` as its default, and a runtime that classifies.
fn classifying_daemon(dir: &Path, gw: &Gateway, wake: Value, clock: Arc<SetClock>) -> Daemon {
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    std::fs::create_dir_all(dir.join("runbooks")).unwrap();
    let token_file = dir.join("gateway-token");
    std::fs::write(&token_file, "gw-adv-figures-7d1e40b2\n").unwrap();
    let gateways = json!({"local": {"base_url": gw.base_url, "token_file": token_file.display().to_string()}});
    let mut qwen = json!({"gateway": "local", "model": "qwen", "kind": "managed", "start_wait_seconds": 600});
    qwen.as_object_mut().unwrap().extend(wake.as_object().cloned().unwrap_or_default());
    let routes = json!({"qwen": qwen, "claude": {"gateway": "local", "model": "claude", "kind": "hosted"}});
    let route_policy = json!({"classifier": "qwen", "default": "claude"});
    let mut table = RouteTable::declare(&Tables { gateways: Some(&gateways), routes: Some(&routes), route_policy: Some(&route_policy), ..Tables::default() }).unwrap();
    table.read_tokens().unwrap();
    let stub = Arc::new(StubRuntime::new());
    stub.script("quick", Script::new(vec![Step::End("done".into())]));
    let policy = Policy { default_confinement: Some(Confinement::Linux), decider: Some("model".into()), kinds: vec!["model".into()], routes: Arc::new(table), ..Policy::default() };
    Daemon::start_full(super::commands::config(dir), policy, Arc::new(Classifying(stub)), clock).unwrap()
}

/// Story Outcome: `wake_needed` is "a run waits on the route and the daemon sends no wake for it". A delegation that names
/// no protocol waits on the daemon's classifier route (`route_wait` with `also`): the daemon tries to wake it and the day's
/// cap refuses the wake ("wake cap reached"), so the delegation waits on the stopped classifier route with no wake. A plain
/// run in the same place makes the row answer `wake_needed: true` (`wake_needed_is_true_while_...` in `route_wake.rs`); the
/// delegation does not, because `wake_figures` counts only runs whose own route or role is the route (`takes_route`).
#[test]
fn adversary_route_figures_a_delegation_waiting_on_the_stopped_classifier_route_with_no_wake_makes_wake_needed_true() {
    let gw = Gateway::start("stopped", (202, json!({"model": "qwen", "phase": "starting"})));
    let dir = super::state::scratch("adv-fig-classifier");
    let clock = Arc::new(SetClock::at("2026-11-26T10:00:00Z").unwrap());
    let d = classifying_daemon(&dir, &gw, wake_on(100, 100), clock.clone());
    // a run on qwen books the day's whole cap with its wake
    let first = submit(&d, "qwen");
    until_records(&d, &first, "runbook.route-wake-answered");
    clock.set("2026-11-26T10:10:01Z");
    until_state(&d, &first, "Ended");

    let o = d.delegate(Delegate { request: "sort the green from the black".into(), model_route: Some("claude".into()), ..Delegate::default() }).unwrap();
    assert!(!o.is_refusal(), "{o:?}");
    let run = o.run_id.clone().unwrap_or_else(|| panic!("no run: {o:?}"));
    let wake = until_records(&d, &run, "runbook.route-wake");
    assert_eq!((wake[0]["route"].clone(), wake[0]["sent"].clone(), wake[0]["reason"].clone()), (json!("qwen"), json!(false), json!("wake cap reached")), "{wake:?}");
    assert_eq!(super::route_wake::detail(&d, &run)["state"], "WaitingForRoute");
    let row = route_row(&d, "qwen");
    assert_eq!(row["phase"], "stopped", "{row}");
    assert_eq!(row["wake_needed"], true, "a delegation waits on the stopped classifier route and the daemon sends no wake for it: {row}");
    gw.no_call();
}

/// A wake the gateway answered `4xx` (not `spend-ceiling`) is neither booked nor joinable, and the run that sent it sends
/// no other: the row answers `wake_needed: true` (the story's rule: "It is so ... after a `4xx` answer"). Pins the rule.
#[test]
fn adversary_route_figures_a_wake_answered_4xx_makes_wake_needed_true_and_books_nothing() {
    let gw = Gateway::start("stopped", (409, json!({"error": {"code": "conflict", "message": "the pod is being moved"}})));
    let dir = super::state::scratch("adv-fig-4xx");
    let clock = Arc::new(SetClock::at("2026-11-26T10:00:00Z").unwrap());
    let d = daemon_in(&dir, &gw, wake_on(500, 100), clock);
    let run = submit(&d, "qwen");
    assert_eq!(until_records(&d, &run, "runbook.route-wake-answered"), vec![json!({"route": "qwen", "status": 409, "code": "conflict"})]);
    assert_eq!(super::route_wake::detail(&d, &run)["state"], "WaitingForRoute");
    assert_eq!(figures(&d, "qwen"), want(0, 500, 100, Some("2026-11-26T10:00:00.000Z"), true, None));
    assert_eq!(gw.wakes(), 1);
    gw.no_call();
}

/// A run that joined a wake (taken, 202) waits past that wake's wait while its own wait goes on: no wake of the route may
/// be joined any more and the run sends none, so the row answers `wake_needed: true`; within the joined wake's wait it is
/// false. Pins the story's "when the wake a run joined is past its wait while the run still waits".
#[test]
fn adversary_route_figures_a_run_past_the_wait_of_the_wake_it_joined_makes_wake_needed_true() {
    let gw = Gateway::start("stopped", (202, json!({"model": "qwen", "phase": "starting"})));
    let dir = super::state::scratch("adv-fig-joined");
    let clock = Arc::new(SetClock::at("2026-11-26T10:00:00Z").unwrap());
    let mut wake = wake_on(500, 100);
    wake["start_wait_seconds"] = json!(60);
    let d = daemon_in(&dir, &gw, wake, clock.clone());
    let first = submit(&d, "qwen");
    until_records(&d, &first, "runbook.route-wake-answered");
    clock.set("2026-11-26T10:00:50Z");
    let second = submit(&d, "qwen");
    assert_eq!(until_records(&d, &second, "runbook.route-wake")[0]["reason"], "joined a wake sent within the wait");
    assert_eq!(route_row(&d, "qwen")["wake_needed"], false, "the joined wake is within its wait");
    // the first run's wait is over at 10:01:00; the second waits until 10:01:50
    clock.set("2026-11-26T10:01:05Z");
    until_state(&d, &first, "Ended");
    assert_eq!(super::route_wake::detail(&d, &second)["state"], "WaitingForRoute");
    let row = route_row(&d, "qwen");
    assert_eq!((row["phase"].clone(), row["wake_needed"].clone(), row["wake_booked_micro_usd"].clone()), (json!("stopped"), json!(true), json!(100)), "{row}");
    assert_eq!(gw.wakes(), 1);
    gw.no_call();
}

/// The brief's "route.wake-answered journal lines of closed runs (ceiling)": a run is cancelled while its wake is out and
/// the gateway then answers `spend-ceiling`. The answer is the daemon's journal line, and `ceiling_refused_at` is that
/// line's `at`; the view, the catalog and the MCP tool answer the same; a daemon started again on the same state answers
/// the same; the next day it is absent. Pins the line path and the replay of the index.
#[test]
fn adversary_route_figures_a_ceiling_answer_after_its_run_was_cancelled_sets_ceiling_refused_at_and_survives_a_restart() {
    let gw = Gateway::start_with("stopped", vec![WakeSaid::Held(503, ceiling_body())], (202, json!({})));
    let dir = super::state::scratch("adv-fig-ceiling-line");
    let clock = Arc::new(SetClock::at("2026-11-26T10:00:00Z").unwrap());
    let d = daemon_in(&dir, &gw, wake_on(500, 100), clock.clone());
    let first = submit(&d, "qwen");
    until_wakes(&gw, 1);
    assert_eq!(d.cancel_run(CancelRun { run_id: first.clone() }).unwrap().name, "cancelled");
    until_state(&d, &first, "Cancelled");
    clock.set("2026-11-26T10:00:02Z");
    gw.let_go(1);
    let lines = wake_answer_lines(&dir.join("state"));
    assert_eq!(lines.len(), 1, "{lines:?}");
    let at = lines[0]["at"].as_str().unwrap_or_else(|| panic!("the line has no at: {}", lines[0])).to_owned();
    assert_eq!(at, "2026-11-26T10:00:02.000Z");
    assert!(records(&d, &first, "runbook.route-wake-answered").is_empty());

    let expected = want(0, 500, 100, Some("2026-11-26T10:00:00.000Z"), false, Some(&at));
    assert_eq!(figures(&d, "qwen"), expected);
    let catalog = d.catalog();
    let in_catalog = catalog[0]["routes"].as_array().unwrap().iter().find(|r| r["route"] == "qwen").cloned().unwrap();
    assert_eq!(figures_of(&in_catalog), expected, "{in_catalog}");
    let tool = runbook::mcp::tools::call(&runbook::mcp::Local(d.clone()), "list_routes", &json!({})).unwrap();
    let listed = tool.body["rows"].as_array().unwrap().iter().find(|r| r["route"] == "qwen").cloned().unwrap();
    assert_eq!(figures_of(&listed), expected, "{listed}");
    super::openapi::check_row("Routes", &route_row(&d, "qwen"));

    d.stop();
    drop(d);
    let d = daemon_in(&dir, &gw, wake_on(500, 100), clock.clone());
    assert_eq!(figures(&d, "qwen"), expected, "a daemon started again answers other figures");
    clock.set("2026-11-27T00:00:00Z");
    assert_eq!(figures(&d, "qwen"), want(0, 500, 100, Some("2026-11-26T10:00:00.000Z"), false, None));
    gw.no_call();
}

/// Pod time across midnight (UTC of the daemon's clock): a run on the ready pod from 23:30 that still lives at 00:15
/// books 15 minutes on the new day (0.75 USD at 3.00 USD per hour), and the same once it ended at 00:15; the next wake's
/// record holds that figure as its `pod_micro_usd`, as the view answered it. The wakes of the day before are not counted.
#[test]
fn adversary_route_figures_pod_time_across_midnight_counts_only_the_new_days_part() {
    let gw = Gateway::start("ready", (202, json!({"model": "qwen", "phase": "starting"})));
    let dir = super::state::scratch("adv-fig-midnight");
    let clock = Arc::new(SetClock::at("2026-11-26T23:30:00Z").unwrap());
    let d = daemon_in(&dir, &gw, priced(10_000_000, 500_000), clock.clone());
    let run = submit_held(&d, "qwen");
    clock.set("2026-11-26T23:45:00Z");
    assert_eq!(figures(&d, "qwen")["pod_booked_micro_usd"], 750_000);
    clock.set("2026-11-27T00:15:00Z");
    let f = figures(&d, "qwen");
    assert_eq!((f["wake_booked_micro_usd"].clone(), f["pod_booked_micro_usd"].clone()), (json!(0), json!(750_000)), "{f}");
    assert_eq!(answer(&d, &run)["outcome"], "done");
    let before = figures(&d, "qwen");
    assert_eq!(before["pod_booked_micro_usd"], 750_000, "{before}");

    gw.set_phase("stopped");
    clock.set("2026-11-27T00:15:20Z");
    super::route_wake::until_phase(&d, "qwen", "stopped");
    let before = figures(&d, "qwen");
    let next = submit(&d, "qwen");
    let wake = until_records(&d, &next, "runbook.route-wake");
    assert_eq!((wake[0]["day"].clone(), wake[0]["pod_micro_usd"].clone(), wake[0]["booked_micro_usd"].clone()), (json!("2026-11-27"), before["pod_booked_micro_usd"].clone(), json!(750_000 + 500_000)), "{wake:?} {before}");
    gw.no_call();
}

/// A managed route that declares only `price_per_hour_micro_usd` answers `pod_booked_micro_usd` and none of the wake
/// figures (the story: "The wake figures answer for a managed route that declares `wake_daily_cap_micro_usd`"). Pins it.
#[test]
fn adversary_route_figures_a_priced_route_without_a_wake_cap_answers_the_pod_figure_only() {
    let gw = Gateway::start("ready", (202, json!({})));
    let dir = super::state::scratch("adv-fig-price-only");
    let clock = Arc::new(SetClock::at("2026-11-26T10:00:00Z").unwrap());
    let extra = json!({"pod": {"gateway": "local", "model": "pod", "kind": "managed", "price_per_hour_micro_usd": 3_000_000}});
    let d = daemon_with(&dir, &gw, json!({}), extra, clock);
    let row = route_row(&d, "pod");
    let present: Vec<&str> = FIGURES.iter().copied().filter(|k| row.get(*k).is_some()).collect();
    assert_eq!(present, vec!["pod_booked_micro_usd"], "{row}");
    assert_eq!(row["pod_booked_micro_usd"], 0, "{row}");
    super::openapi::check_row("Routes", &row);
    gw.no_call();
}
