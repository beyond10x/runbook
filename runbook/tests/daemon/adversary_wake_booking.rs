//! Adversary cases of the unit that books a wake only when the gateway took it and books a managed route's pod time
//! (stories `a-wake-is-booked-only-when-it-succeeds` and `a-managed-route-books-its-pods-measured-time`).
//!
//! In process, on a test clock, with the stub runtime: no model is called and nothing leaves loopback. The gateway is
//! a stand-in on a port of the test's own; each request is answered on a thread of its own, so a wake it holds does not
//! hold the reads behind it.

use runbook::clock::SetClock;
use runbook::commands::{CancelRun, Daemon, Policy, SubmitRun};
use runbook::routes::{RouteTable, Tables};
use runbook::runtime::{Autonomy, Confinement};
use runbook::stub::{Ask, Script, Step, StubRuntime};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TOKEN: &str = "gw-adv-7e21c0b94d18";
const WAKE: &str = "POST /v1/models/qwen/wake";
const STATUS: &str = "GET /v1/models/qwen/status";

struct Said {
    phase: &'static str,
    /// The status the model's status route answers with: 200 gives the phase, any other an error body.
    status_code: u16,
    /// The answers to the next wakes, in order; once used up, every wake is answered 202.
    wakes: VecDeque<(u16, Value)>,
    /// While true a wake is held unanswered until the case lets it go.
    hold: bool,
    lines: Vec<String>,
    /// How many wakes were answered.
    answered: usize,
}

struct Gw {
    base_url: String,
    said: Arc<Mutex<Said>>,
}

fn respond(stream: &mut std::net::TcpStream, code: u16, body: &Value) {
    let body = body.to_string();
    let _ = stream.write_all(format!("HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
}

impl Gw {
    fn start(phase: &'static str, wakes: Vec<(u16, Value)>) -> Gw {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let said = Arc::new(Mutex::new(Said { phase, status_code: 200, wakes: wakes.into(), hold: false, lines: Vec::new(), answered: 0 }));
        let s = said.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let s = s.clone();
                std::thread::spawn(move || {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                    let mut head = Vec::new();
                    let mut byte = [0u8; 1];
                    while !head.ends_with(b"\r\n\r\n") && head.len() < 64 * 1024 {
                        match stream.read(&mut byte) {
                            Ok(1) => head.push(byte[0]),
                            _ => break,
                        }
                    }
                    let head = String::from_utf8_lossy(&head).into_owned();
                    let line: String = head.lines().next().unwrap_or("").rsplit_once(' ').map(|(l, _)| l.to_owned()).unwrap_or_default();
                    s.lock().unwrap().lines.push(line.clone());
                    match line.as_str() {
                        "GET /ready" => respond(&mut stream, 200, &json!({"ready": true})),
                        "GET /v1/models" => respond(&mut stream, 200, &json!({"object": "list", "data": [{"id": "qwen", "wires": ["chat"]}]})),
                        STATUS => {
                            let (code, phase) = {
                                let said = s.lock().unwrap();
                                (said.status_code, said.phase)
                            };
                            if code == 200 {
                                respond(&mut stream, 200, &json!({"model": "qwen", "phase": phase}));
                            } else {
                                respond(&mut stream, code, &json!({"error": {"code": "internal"}}));
                            }
                        }
                        WAKE => {
                            let (code, body) = s.lock().unwrap().wakes.pop_front().unwrap_or((202, json!({"model": "qwen", "phase": "starting"})));
                            let limit = Instant::now() + Duration::from_secs(4);
                            while s.lock().unwrap().hold && Instant::now() < limit {
                                std::thread::sleep(Duration::from_millis(5));
                            }
                            respond(&mut stream, code, &body);
                            s.lock().unwrap().answered += 1;
                        }
                        _ => respond(&mut stream, 404, &json!({"error": {"code": "not-found"}})),
                    }
                });
            }
        });
        Gw { base_url, said }
    }

    fn set(&self, f: impl FnOnce(&mut Said)) {
        f(&mut self.said.lock().unwrap());
    }

    fn count(&self, line: &str) -> usize {
        self.said.lock().unwrap().lines.iter().filter(|l| *l == line).count()
    }

    fn answered(&self) -> usize {
        self.said.lock().unwrap().answered
    }

    fn until(&self, what: &str, f: impl Fn(&Gw) -> bool) {
        let limit = Instant::now() + Duration::from_secs(10);
        while !f(self) {
            assert!(Instant::now() < limit, "the stand-in never saw {what}: {:?}", self.said.lock().unwrap().lines);
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn no_call(&self) {
        let lines = self.said.lock().unwrap().lines.clone();
        assert!(lines.iter().all(|l| l.starts_with("GET ") || l == WAKE), "the gateway got a model call: {lines:?}");
    }
}

/// A daemon on a test clock with one managed route `qwen` on the stand-in, its keys `extra` added to a wait of 600 s.
fn daemon_in(dir: &Path, gw: &Gw, extra: Value, clock: Arc<SetClock>) -> Daemon {
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    std::fs::create_dir_all(dir.join("runbooks")).unwrap();
    let token_file = dir.join("gateway-token");
    std::fs::write(&token_file, format!("{TOKEN}\n")).unwrap();
    let gateways = json!({"local": {"base_url": gw.base_url, "token_file": token_file.display().to_string()}});
    let mut qwen = json!({"gateway": "local", "model": "qwen", "kind": "managed", "start_wait_seconds": 600});
    qwen.as_object_mut().unwrap().extend(extra.as_object().cloned().unwrap_or_default());
    let routes = json!({"qwen": qwen});
    let mut table = RouteTable::declare(&Tables { gateways: Some(&gateways), routes: Some(&routes), ..Tables::default() }).unwrap();
    table.read_tokens().unwrap();
    let stub = Arc::new(StubRuntime::new());
    stub.script("quick", Script::new(vec![Step::End("done".into())]));
    let option = json!({"letter": "A", "does": "go on", "cost": "none", "consequence": {"admits": [], "blocks": []}});
    let ask = Ask { request_id: "{run}-q1".into(), case_id: "case-1".into(), action: "stub.approve".into(), capability: "approve".into(), question: "go on?".into(), options: vec![option], artifact: None };
    stub.script("held", Script::new(vec![Step::Ask(vec![ask]), Step::End("done".into())]));
    let policy = Policy { default_confinement: Some(Confinement::Linux), kinds: vec!["model".into()], routes: Arc::new(table), ..Policy::default() };
    Daemon::start_full(super::commands::config(dir), policy, stub, clock).unwrap()
}

fn submit_as(d: &Daemon, runbook: &str) -> String {
    let o = d.submit_run(SubmitRun { runbook_id: runbook.into(), intake: "x".into(), autonomy: Some(Autonomy::A5), decider: Some("model".into()), model_route: Some("qwen".into()), ..SubmitRun::default() }).unwrap();
    assert_eq!(o.name, "submitted", "{o:?}");
    o.run_id.unwrap()
}

fn detail(d: &Daemon, run: &str) -> Value {
    d.run_detail(run)[0].clone()
}

fn until_state(d: &Daemon, run: &str, want: &str) -> Value {
    let limit = Instant::now() + Duration::from_secs(10);
    loop {
        let r = detail(d, run);
        if r["state"] == want {
            return r;
        }
        assert!(Instant::now() < limit, "the run did not reach {want}: {r}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn records(d: &Daemon, run: &str, kind: &str) -> Vec<Value> {
    d.events_after(run, 0).into_iter().filter(|e| e["kind"] == kind).map(|e| e["record"].clone()).collect()
}

fn until_n(d: &Daemon, run: &str, kind: &str, n: usize) -> Vec<Value> {
    let limit = Instant::now() + Duration::from_secs(10);
    loop {
        let r = records(d, run, kind);
        if r.len() >= n {
            return r;
        }
        assert!(Instant::now() < limit, "the run has {} events {kind}, not {n}: {:?}", r.len(), d.events_after(run, 0));
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn wake_on(cap: u64, cost: u64) -> Value {
    json!({"wake_on_submit": true, "wake_daily_cap_micro_usd": cap, "wake_cost_micro_usd": cost})
}

fn unavailable() -> (u16, Value) {
    (503, json!({"error": {"code": "target-unavailable", "message": "the pod start failed: no-capacity"}}))
}

/// Acceptance of `a-wake-is-booked-only-when-it-succeeds`: "A wake answered 5xx or `no-capacity` books 0". The run is
/// cancelled while its wake is out; the gateway then answers 503. The wake started nothing, so the day's cap holds
/// only the next run's own wake.
#[test]
fn a_wake_answered_503_after_its_run_was_cancelled_books_nothing() {
    let gw = Gw::start("stopped", vec![unavailable()]);
    gw.set(|s| s.hold = true);
    let dir = super::state::scratch("adv-wake-cancelled");
    let clock = Arc::new(SetClock::at("2026-11-26T10:00:00Z").unwrap());
    let d = daemon_in(&dir, &gw, wake_on(150, 100), clock.clone());
    let first = submit_as(&d, "quick");
    gw.until("the wake", |g| g.count(WAKE) == 1);
    assert_eq!(d.cancel_run(CancelRun { run_id: first.clone() }).unwrap().name, "cancelled");
    until_state(&d, &first, "Cancelled");
    // the gateway answers the wake now: 503, the pod did not start
    gw.set(|s| s.hold = false);
    gw.until("the wake's answer", |g| g.answered() == 1);
    std::thread::sleep(Duration::from_millis(200));

    // a run after the first one's wait: its wake is the day's only one the gateway took or has out
    clock.set("2026-11-26T10:10:01Z");
    let second = submit_as(&d, "quick");
    let wake = until_n(&d, &second, "runbook.route-wake", 1);
    assert_eq!(
        wake[0],
        json!({"route": "qwen", "sent": true, "day": "2026-11-26", "cost_micro_usd": 100, "booked_micro_usd": 100, "cap_micro_usd": 150}),
        "the 503 of a cancelled run's wake stays booked: {:?}",
        d.events_after(&first, 0)
    );
    gw.no_call();
}

/// A run submitted after the run that sent the route's wakes gave up (every try answered 503, its wait over) sends a
/// wake of its own: the gateway took none of those wakes, so none of them is starting the model.
#[test]
fn a_run_submitted_after_the_sender_gave_up_sends_its_own_wake() {
    let gw = Gw::start("stopped", vec![unavailable(), unavailable()]);
    let dir = super::state::scratch("adv-wake-join-dead");
    let clock = Arc::new(SetClock::at("2026-11-26T10:00:00Z").unwrap());
    let mut extra = wake_on(1_000, 100);
    extra["start_wait_seconds"] = json!(60);
    let d = daemon_in(&dir, &gw, extra, clock.clone());
    let first = submit_as(&d, "quick");
    until_n(&d, &first, "runbook.route-wake-answered", 1);
    clock.set("2026-11-26T10:00:30Z");
    until_n(&d, &first, "runbook.route-wake-answered", 2);
    clock.set("2026-11-26T10:01:00Z");
    assert_eq!(until_state(&d, &first, "Ended")["outcome"], "stopped: route");
    assert_eq!(gw.count(WAKE), 2);

    // the first run gave up; a new run of the route
    clock.set("2026-11-26T10:01:01Z");
    let second = submit_as(&d, "quick");
    let wake = until_n(&d, &second, "runbook.route-wake", 1);
    assert_eq!(wake[0]["sent"], true, "the run joined a wake the gateway refused, of a run that had ended: {wake:?}");
    gw.no_call();
}

/// The reader of a priced route's pod: a status read that fails (the model's status route answers 500 while the
/// gateway is ready and lists the model) says nothing of the pod, so it does not open the use of a pod read stopped.
#[test]
fn a_status_read_that_fails_does_not_reopen_the_use_of_a_stopped_pod() {
    let gw = Gw::start("ready", vec![]);
    let dir = super::state::scratch("adv-pod-status-500");
    let clock = Arc::new(SetClock::at("2026-11-26T10:00:00Z").unwrap());
    let mut extra = wake_on(10_000_000, 500_000);
    extra["price_per_hour_micro_usd"] = json!(3_000_000);
    let d = daemon_in(&dir, &gw, extra, clock.clone());
    let run = submit_as(&d, "held");
    until_state(&d, &run, "WaitingOnDecision");

    // the pod stops at 10:20; the reader sees it
    gw.set(|s| s.phase = "stopped");
    clock.set("2026-11-26T10:20:00Z");
    assert_eq!(until_n(&d, &run, "runbook.route-pod-phase", 1), vec![json!({"route": "qwen", "phase": "stopped"})]);

    // the model's status route fails from now on; the reader reads again
    gw.set(|s| s.status_code = 500);
    let before = gw.count(STATUS);
    clock.set("2026-11-26T10:20:10Z");
    gw.until("a status read after the failure", |g| g.count(STATUS) > before);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(records(&d, &run, "runbook.route-pod-phase"), vec![json!({"route": "qwen", "phase": "stopped"})], "a failed status read was taken as the pod ready");
    gw.no_call();
}

/// Acceptance of `a-managed-route-books-its-pods-measured-time`: a run alone on a ready pod from t0 books (t1 - t0) at
/// the price. The daemon ends the run itself at its stop (`orphan_live_runs`, a second stop signal) at 10:30 while the
/// pod read ready the whole time; the run waited on a decision, so its last event is at 10:00. The day's figure of
/// pod time a later wake reads holds the 30 minutes the daemon saw: 1.50 USD at 3.00 USD per hour.
#[test]
fn a_run_the_daemon_orphans_at_its_stop_books_the_pod_time_it_used() {
    let gw = Gw::start("ready", vec![]);
    let dir = super::state::scratch("adv-pod-orphan-stop");
    let clock = Arc::new(SetClock::at("2026-11-26T10:00:00Z").unwrap());
    let mut extra = wake_on(10_000_000, 500_000);
    extra["price_per_hour_micro_usd"] = json!(3_000_000);
    let d = daemon_in(&dir, &gw, extra.clone(), clock.clone());
    let run = submit_as(&d, "held");
    until_state(&d, &run, "WaitingOnDecision");
    // the pod is read ready while the run lives
    let before = gw.count(STATUS);
    clock.set("2026-11-26T10:30:00Z");
    gw.until("a read of the pod", |g| g.count(STATUS) > before);
    assert_eq!(d.orphan_live_runs(), 1);
    assert_eq!(detail(&d, &run)["outcome"], "orphaned");
    d.stop();
    drop(d);

    // the pod stopped later; a run of the same day wakes it and its record shows the day's pod time
    gw.set(|s| s.phase = "stopped");
    clock.set("2026-11-26T11:00:00Z");
    let d = daemon_in(&dir, &gw, extra, clock.clone());
    let next = submit_as(&d, "quick");
    let wake = until_n(&d, &next, "runbook.route-wake", 1);
    assert_eq!(wake[0]["pod_micro_usd"], 1_500_000, "the 30 minutes the run used the ready pod before the daemon ended it are lost: {wake:?}");
    gw.no_call();
}
