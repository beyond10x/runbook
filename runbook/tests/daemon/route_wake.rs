//! The wake of a managed route behind a daily wake cap (`docs/model-routes.md`, section 3 point 4, D-5; story
//! `route-wake-behind-a-daily-cap`). It ships off: `wake_on_submit = true` needs a `wake_daily_cap_micro_usd` above 0
//! and a `wake_cost_micro_usd` above 0, else the daemon refuses to start and names the key. With the wake on, a run
//! the daemon holds on a stopped managed route sends one `POST /v1/models/{alias}/wake`, booked in the run event
//! `runbook.route-wake` against the route's cap for the day of the daemon's clock; a wake the day's cap does not cover is
//! not sent and the event says "wake cap reached". A `spend-ceiling` answer to the wake ends the run `stopped: route`
//! and its `runbook.route-wait-ended` names the ceiling.
//!
//! In process, on a test clock, with the stub runtime: no model is called and nothing leaves loopback. The gateway is
//! a stand-in on a port of the test's own; it keeps the head of everything it is sent and answers the wake as the case
//! sets. The daemon's refusal at start runs the `runbook serve` binary on a file of the case.

use runbook::clock::SetClock;
use runbook::commands::{AnswerDecision, Daemon, Policy, SubmitRun};
use runbook::routes::{RouteTable, Tables};
use runbook::runtime::{Autonomy, Confinement};
use runbook::stub::{Ask, Script, Step, StubRuntime};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TOKEN: &str = "gw-wake-3c91f0a7d2e6";
const WAKE: &str = "POST /v1/models/qwen/wake";

/// One answer of the stand-in to a wake: a status and a body, or none (it holds the request open past the daemon's
/// bound of 5 s for an answer, then closes it).
#[derive(Clone)]
enum WakeSaid {
    Answer(u16, Value),
    Silent,
}

/// What the stand-in answers: the phase of `qwen`, the answers to the next wakes in order and, once those are given,
/// the status and body of every wake.
struct Said {
    phase: &'static str,
    next: std::collections::VecDeque<WakeSaid>,
    wake: (u16, Value),
}

struct Gateway {
    base_url: String,
    heads: Arc<Mutex<Vec<String>>>,
    said: Arc<Mutex<Said>>,
}

impl Gateway {
    fn start(phase: &'static str, wake: (u16, Value)) -> Gateway {
        Gateway::start_with(phase, Vec::new(), wake)
    }

    /// A stand-in that answers the first wakes as `next` says, in order, and every later one `wake`.
    fn start_with(phase: &'static str, next: Vec<WakeSaid>, wake: (u16, Value)) -> Gateway {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let said = Arc::new(Mutex::new(Said { phase, next: next.into(), wake }));
        let heads = Arc::new(Mutex::new(Vec::new()));
        let (s, kept) = (said.clone(), heads.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
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
                kept.lock().unwrap().push(head);
                let said = {
                    let mut said = s.lock().unwrap();
                    match line.as_str() {
                        "GET /ready" => WakeSaid::Answer(200, json!({"ready": true})),
                        "GET /v1/models" => WakeSaid::Answer(200, json!({"object": "list", "data": [{"id": "qwen", "wires": ["chat"]}, {"id": "claude", "wires": ["messages"]}]})),
                        "GET /v1/models/qwen/status" => WakeSaid::Answer(200, json!({"model": "qwen", "phase": said.phase})),
                        WAKE => {
                            let wake = said.wake.clone();
                            said.next.pop_front().unwrap_or(WakeSaid::Answer(wake.0, wake.1))
                        }
                        _ => WakeSaid::Answer(404, json!({"error": {"code": "not-found"}})),
                    }
                };
                match said {
                    WakeSaid::Answer(code, body) => {
                        let body = body.to_string();
                        let _ = stream.write_all(format!("HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
                    }
                    // held open on a thread of its own, so the reads after it are answered
                    WakeSaid::Silent => {
                        std::thread::spawn(move || {
                            std::thread::sleep(Duration::from_secs(6));
                            drop(stream);
                        });
                    }
                }
            }
        });
        Gateway { base_url, heads, said }
    }

    fn set_phase(&self, phase: &'static str) {
        self.said.lock().unwrap().phase = phase;
    }

    /// How often the stand-in was asked the status of `qwen`.
    fn status_reads(&self) -> usize {
        self.lines().iter().filter(|l| *l == "GET /v1/models/qwen/status").count()
    }

    /// Waits until the daemon read the status of `qwen` once more than `before`: the run's wait looked at its clock.
    fn until_read_after(&self, before: usize) {
        let limit = Instant::now() + Duration::from_secs(10);
        while self.status_reads() <= before {
            assert!(Instant::now() < limit, "no read: {:?}", self.lines());
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn lines(&self) -> Vec<String> {
        self.heads.lock().unwrap().iter().map(|h| h.lines().next().unwrap_or("").rsplit_once(' ').map(|(l, _)| l.to_owned()).unwrap_or_default()).collect()
    }

    fn wakes(&self) -> usize {
        self.lines().iter().filter(|l| *l == WAKE).count()
    }

    /// Nothing but reads and wakes went to the gateway: no model call.
    fn no_call(&self) {
        let lines = self.lines();
        assert!(lines.iter().all(|l| l.starts_with("GET ") || l == WAKE), "the gateway got a model call: {lines:?}");
    }
}

/// A daemon on a test clock with the decider kind `model`, the stub runtime and two routes on `gw`: `qwen` (managed,
/// waits 600 s, with the wake keys `wake`) and `claude` (hosted). The gateway's token file is in `dir`.
fn daemon_in(dir: &Path, gw: &Gateway, wake: Value, clock: Arc<SetClock>) -> Daemon {
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    std::fs::create_dir_all(dir.join("runbooks")).unwrap();
    let token_file = dir.join("gateway-token");
    std::fs::write(&token_file, format!("{TOKEN}\n")).unwrap();
    let gateways = json!({"local": {"base_url": gw.base_url, "token_file": token_file.display().to_string()}});
    let mut qwen = json!({"gateway": "local", "model": "qwen", "kind": "managed", "start_wait_seconds": 600});
    qwen.as_object_mut().unwrap().extend(wake.as_object().cloned().unwrap_or_default());
    let routes = json!({"qwen": qwen, "claude": {"gateway": "local", "model": "claude", "kind": "hosted"}});
    let mut table = RouteTable::declare(&Tables { gateways: Some(&gateways), routes: Some(&routes), ..Tables::default() }).unwrap();
    table.read_tokens().unwrap();
    let stub = Arc::new(StubRuntime::new());
    stub.script("quick", Script::new(vec![Step::End("done".into())]));
    // a run that waits on one decision until the case answers it
    let option = json!({"letter": "A", "does": "go on", "cost": "none", "consequence": {"admits": [], "blocks": []}});
    let ask = Ask { request_id: "{run}-q1".into(), case_id: "case-1".into(), action: "stub.approve".into(), capability: "approve".into(), question: "go on?".into(), options: vec![option], artifact: None };
    stub.script("held", Script::new(vec![Step::Ask(vec![ask]), Step::End("done".into())]));
    let policy = Policy { default_confinement: Some(Confinement::Linux), kinds: vec!["model".into()], routes: Arc::new(table), ..Policy::default() };
    Daemon::start_full(super::commands::config(dir), policy, stub, clock).unwrap()
}

fn daemon(tag: &str, gw: &Gateway, wake: Value) -> (Daemon, Arc<SetClock>) {
    let dir = super::state::scratch(tag);
    let clock = Arc::new(SetClock::at("2026-11-26T10:00:00Z").unwrap());
    (daemon_in(&dir, gw, wake, clock.clone()), clock)
}

fn submit(d: &Daemon, route: &str) -> String {
    let o = d.submit_run(SubmitRun { runbook_id: "quick".into(), intake: "x".into(), autonomy: Some(Autonomy::A5), decider: Some("model".into()), model_route: Some(route.into()), ..SubmitRun::default() }).unwrap();
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
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The records of the run's events of `kind`.
fn records(d: &Daemon, run: &str, kind: &str) -> Vec<Value> {
    d.events_after(run, 0).into_iter().filter(|e| e["kind"] == kind).map(|e| e["record"].clone()).collect()
}

/// The records of the run's events of `kind`, once there is one.
fn until_records(d: &Daemon, run: &str, kind: &str) -> Vec<Value> {
    let limit = Instant::now() + Duration::from_secs(10);
    loop {
        let r = records(d, run, kind);
        if !r.is_empty() {
            return r;
        }
        assert!(Instant::now() < limit, "the run has no event {kind}: {:?}", d.events_after(run, 0));
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wake_on(cap: u64, cost: u64) -> Value {
    json!({"wake_on_submit": true, "wake_daily_cap_micro_usd": cap, "wake_cost_micro_usd": cost})
}

/// `runbook serve` on `file`: its status and standard error, and whether it wrote its state directory.
fn serve(tag: &str, file: &str) -> (Option<i32>, String, bool) {
    let dir = super::state::scratch(tag);
    std::fs::write(dir.join("runbook.toml"), file).unwrap();
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_runbook"));
    cmd.arg("serve").args(["--principal", super::PRINCIPAL, "--listen", "127.0.0.1:0"]).arg("--state-dir").arg(dir.join("state"));
    cmd.env_remove("RUNBOOK_TOKEN").env("RUNBOOK_CONFIG", dir.join("runbook.toml"));
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let (status, _, err) = super::http::exits_within(cmd, Duration::from_secs(10));
    (status, err, dir.join("state").exists())
}

#[test]
fn wake_on_submit_true_without_a_cap_above_0_is_refused_at_start_by_name() {
    let route = |extra: &str| format!("[gateways.gw]\nbase_url = \"http://127.0.0.1:9\"\n[routes.qwen]\ngateway = \"gw\"\nmodel = \"qwen\"\nkind = \"managed\"\n{extra}");
    let cases = [
        ("wake_on_submit = true\n", "routes.qwen.wake_on_submit", "wake_daily_cap_micro_usd above 0"),
        ("wake_on_submit = true\nwake_daily_cap_micro_usd = 0\nwake_cost_micro_usd = 100\n", "routes.qwen.wake_on_submit", "wake_daily_cap_micro_usd above 0"),
        ("wake_on_submit = true\nwake_cost_micro_usd = 100\n", "routes.qwen.wake_on_submit", "wake_daily_cap_micro_usd above 0"),
        // a wake that books nothing would never reach the cap
        ("wake_on_submit = true\nwake_daily_cap_micro_usd = 500\n", "routes.qwen.wake_on_submit", "wake_cost_micro_usd above 0"),
        ("wake_on_submit = true\nwake_daily_cap_micro_usd = 500\nwake_cost_micro_usd = 0\n", "routes.qwen.wake_on_submit", "wake_cost_micro_usd above 0"),
    ];
    for (n, (extra, key, why)) in cases.iter().enumerate() {
        let (status, err, wrote) = serve(&format!("wake-refused-{n}"), &route(extra));
        assert_eq!(status, Some(2), "case {n}: the daemon started: {err}");
        assert!(err.contains(key) && err.contains(why), "case {n}: the refusal does not name {key} and {why}: {err}");
        assert!(!wrote, "case {n}: the state directory was written");
    }
    // with a cap and a cost above 0 the route is declared with its wake on; without the key it is off
    let gateways = json!({"gw": {"base_url": "http://127.0.0.1:9"}});
    let declare = |wake: Value| {
        let mut qwen = json!({"gateway": "gw", "model": "qwen", "kind": "managed"});
        qwen.as_object_mut().unwrap().extend(wake.as_object().cloned().unwrap_or_default());
        let routes = json!({"qwen": qwen});
        RouteTable::declare(&Tables { gateways: Some(&gateways), routes: Some(&routes), ..Tables::default() }).map(|t| t.route("qwen").unwrap().wake)
    };
    let on = declare(wake_on(500, 100)).unwrap();
    assert_eq!((on.on_submit, on.daily_cap_micro_usd, on.cost_micro_usd), (true, Some(500), Some(100)));
    assert!(!declare(json!({})).unwrap().on_submit, "the shipped default is off");
    assert!(!declare(json!({"wake_daily_cap_micro_usd": 500, "wake_cost_micro_usd": 100})).unwrap().on_submit, "a cap alone turns no wake on");
}

/// A wake that costs more than the day's cap could never be sent: the daemon refuses to start and names the key.
#[test]
fn a_wake_cost_above_the_daily_cap_is_refused_at_start_by_name() {
    let file = "[gateways.gw]\nbase_url = \"http://127.0.0.1:9\"\n[routes.qwen]\ngateway = \"gw\"\nmodel = \"qwen\"\nkind = \"managed\"\nwake_on_submit = true\nwake_daily_cap_micro_usd = 100\nwake_cost_micro_usd = 101\n";
    let (status, err, wrote) = serve("wake-cost-over-cap", file);
    assert_eq!(status, Some(2), "the daemon started: {err}");
    assert!(err.contains("routes.qwen.wake_cost_micro_usd") && err.contains("wake_daily_cap_micro_usd"), "the refusal does not name the keys: {err}");
    assert!(!wrote, "the state directory was written");
    // a wake that costs the whole cap is one wake a day
    let gateways = json!({"gw": {"base_url": "http://127.0.0.1:9"}});
    let routes = json!({"qwen": {"gateway": "gw", "model": "qwen", "kind": "managed", "wake_on_submit": true, "wake_daily_cap_micro_usd": 100, "wake_cost_micro_usd": 100}});
    let declared = RouteTable::declare(&Tables { gateways: Some(&gateways), routes: Some(&routes), ..Tables::default() });
    assert!(declared.is_ok(), "a cost equal to the cap was refused: {:?}", declared.err());
}

/// A run that joins a wake sent for its route within the wait takes that wake's answer: the gateway answered it
/// `spend-ceiling`, so the joining run ends `stopped: route` at once and its `runbook.route-wait-ended` names the ceiling,
/// as the run that sent the wake does.
#[test]
fn a_run_that_joins_a_wake_answered_spend_ceiling_ends_stopped_route_naming_the_ceiling() {
    let gw = Gateway::start("stopped", (503, json!({"error": {"code": "spend-ceiling", "message": "a pod start would pass the daily cap"}})));
    let (d, clock) = daemon("wake-join-ceiling", &gw, wake_on(500, 100));
    let first = submit(&d, "qwen");
    assert_eq!(until_state(&d, &first, "Ended")["outcome"], "stopped: route");
    clock.set("2026-11-26T10:05:00Z");
    let second = submit(&d, "qwen");
    // the clock does not move: the run ends on the answer it joined, not at the end of its wait of 600 s
    let r = until_state(&d, &second, "Ended");
    assert_eq!((r["outcome"].clone(), r["route_phase"].clone()), (json!("stopped: route"), json!("stopped")), "{r}");
    assert_eq!(records(&d, &second, "runbook.route-wake")[0]["reason"], "joined a wake sent within the wait");
    assert_eq!(records(&d, &second, "runbook.route-wait-ended"), vec![json!({"route": "qwen", "phase": "stopped", "ceiling": "spend-ceiling"})]);
    assert_eq!(gw.wakes(), 1, "the joining run sent a wake");
    gw.no_call();
}

#[test]
fn with_a_cap_one_wake_is_sent_for_a_run_on_a_stopped_route_and_booked_and_a_spent_cap_sends_none() {
    let gw = Gateway::start("stopped", (202, json!({"model": "qwen", "phase": "starting"})));
    let dir = super::state::scratch("wake-cap");
    let clock = Arc::new(SetClock::at("2026-11-26T10:00:00Z").unwrap());
    let d = daemon_in(&dir, &gw, wake_on(250, 100), clock.clone());

    let first = submit(&d, "qwen");
    until_state(&d, &first, "WaitingForRoute");
    let booked = until_records(&d, &first, "runbook.route-wake");
    assert_eq!(booked, vec![json!({"route": "qwen", "sent": true, "day": "2026-11-26", "cost_micro_usd": 100, "booked_micro_usd": 100, "cap_micro_usd": 250})]);
    let answered = until_records(&d, &first, "runbook.route-wake-answered");
    assert_eq!(answered, vec![json!({"route": "qwen", "status": 202})]);
    assert_eq!(gw.wakes(), 1);
    let wake_head = gw.heads.lock().unwrap().iter().find(|h| h.starts_with(WAKE)).cloned().unwrap();
    assert!(wake_head.contains(&format!("Authorization: Bearer {TOKEN}\r\n")), "the wake carries the gateway's token: {wake_head}");

    // one wake per run: the run reads its route again while it waits and wakes nothing more
    clock.set("2026-11-26T10:00:05Z");
    let limit = Instant::now() + Duration::from_secs(10);
    while gw.lines().iter().filter(|l| *l == "GET /v1/models/qwen/status").count() < 2 {
        assert!(Instant::now() < limit, "no read 5 s later: {:?}", gw.lines());
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(gw.wakes(), 1, "a waiting run woke its route twice");

    // the next run comes after the first one's wait of 600 s, so it joins no wake and sends its own
    clock.set("2026-11-26T10:10:01Z");
    let second = submit(&d, "qwen");
    let booked = until_records(&d, &second, "runbook.route-wake");
    assert_eq!(booked, vec![json!({"route": "qwen", "sent": true, "day": "2026-11-26", "cost_micro_usd": 100, "booked_micro_usd": 200, "cap_micro_usd": 250})]);
    until_records(&d, &second, "runbook.route-wake-answered");
    assert_eq!(gw.wakes(), 2);

    // the day's cap has 50 left, less than a wake: none is sent, the run waits, and its record says so
    clock.set("2026-11-26T10:20:02Z");
    let third = submit(&d, "qwen");
    let refused = until_records(&d, &third, "runbook.route-wake");
    assert_eq!(refused, vec![json!({"route": "qwen", "sent": false, "reason": "wake cap reached", "day": "2026-11-26", "cost_micro_usd": 100, "booked_micro_usd": 200, "cap_micro_usd": 250})]);
    assert_eq!(detail(&d, &third)["state"], "WaitingForRoute");
    assert!(records(&d, &third, "runbook.route-wake-answered").is_empty());
    assert_eq!(gw.wakes(), 2, "a wake was sent past the day's cap");

    // the booking is the journal's: a daemon started again on the same state counts the day's wakes
    d.stop();
    drop(d);
    clock.set("2026-11-26T10:30:03Z");
    let d = daemon_in(&dir, &gw, wake_on(250, 100), clock.clone());
    let fourth = submit(&d, "qwen");
    let refused = until_records(&d, &fourth, "runbook.route-wake");
    assert_eq!(refused[0]["sent"], false, "{refused:?}");
    assert_eq!(refused[0]["booked_micro_usd"], 200, "{refused:?}");
    assert_eq!(gw.wakes(), 2, "a daemon started again woke past the day's cap");

    // the next day of the daemon's clock has its whole cap again
    clock.set("2026-11-27T00:00:00Z");
    let fifth = submit(&d, "qwen");
    let booked = until_records(&d, &fifth, "runbook.route-wake");
    assert_eq!(booked, vec![json!({"route": "qwen", "sent": true, "day": "2026-11-27", "cost_micro_usd": 100, "booked_micro_usd": 100, "cap_micro_usd": 250})]);
    until_records(&d, &fifth, "runbook.route-wake-answered");
    assert_eq!(gw.wakes(), 3);
    gw.no_call();
}

#[test]
fn a_run_within_the_wait_of_a_wake_sent_for_its_route_joins_that_wake_and_sends_and_books_none() {
    let gw = Gateway::start("stopped", (202, json!({})));
    let dir = super::state::scratch("wake-join");
    let clock = Arc::new(SetClock::at("2026-11-26T10:00:00Z").unwrap());
    let d = daemon_in(&dir, &gw, wake_on(500, 100), clock.clone());
    let first = submit(&d, "qwen");
    until_records(&d, &first, "runbook.route-wake-answered");
    assert_eq!(gw.wakes(), 1);

    // 5 minutes later, within the route's start_wait_seconds of 600: the second run joins the first run's wake
    clock.set("2026-11-26T10:05:00Z");
    let second = submit(&d, "qwen");
    let joined = until_records(&d, &second, "runbook.route-wake");
    let want = json!({"route": "qwen", "sent": false, "reason": "joined a wake sent within the wait", "joined": first, "joined_at": "2026-11-26T10:00:00.000Z"});
    assert_eq!(joined, vec![want.clone()]);
    assert_eq!(detail(&d, &second)["state"], "WaitingForRoute");
    assert!(records(&d, &second, "runbook.route-wake-answered").is_empty());
    assert_eq!(gw.wakes(), 1, "a run within the wait of a wake sent for its route woke it again");

    // the join is read from the journal: a daemon started again joins the wake it sent before
    d.stop();
    drop(d);
    clock.set("2026-11-26T10:09:59.999Z");
    let d = daemon_in(&dir, &gw, wake_on(500, 100), clock.clone());
    let third = submit(&d, "qwen");
    assert_eq!(until_records(&d, &third, "runbook.route-wake"), vec![want]);
    assert_eq!(gw.wakes(), 1, "a daemon started again woke within the wait of a wake it sent");

    // 600 s after the wake its wait is over: the next run sends a wake of its own, and the joins booked nothing
    clock.set("2026-11-26T10:10:00Z");
    let fourth = submit(&d, "qwen");
    let booked = until_records(&d, &fourth, "runbook.route-wake");
    assert_eq!(booked, vec![json!({"route": "qwen", "sent": true, "day": "2026-11-26", "cost_micro_usd": 100, "booked_micro_usd": 200, "cap_micro_usd": 500})]);
    until_records(&d, &fourth, "runbook.route-wake-answered");
    assert_eq!(gw.wakes(), 2);
    gw.no_call();
}

#[test]
fn a_route_that_is_starting_or_hosted_or_has_the_wake_off_is_not_woken() {
    // the shipped default: a stopped managed route with no wake keys waits and is not woken
    let gw = Gateway::start("stopped", (202, json!({})));
    let (d, _) = daemon("wake-off", &gw, json!({"wake_daily_cap_micro_usd": 500, "wake_cost_micro_usd": 100}));
    let run = submit(&d, "qwen");
    until_state(&d, &run, "WaitingForRoute");
    // a hosted route on the same daemon starts at once
    let hosted = submit(&d, "claude");
    until_state(&d, &hosted, "Ended");
    assert!(records(&d, &run, "runbook.route-wake").is_empty() && records(&d, &hosted, "runbook.route-wake").is_empty());
    // with the wake on, a model already starting is not woken
    let starting = Gateway::start("starting", (202, json!({})));
    let (d2, _) = daemon("wake-starting", &starting, wake_on(500, 100));
    let run2 = submit(&d2, "qwen");
    until_state(&d2, &run2, "WaitingForRoute");
    std::thread::sleep(Duration::from_millis(200));
    assert!(records(&d2, &run2, "runbook.route-wake").is_empty(), "{:?}", d2.events_after(&run2, 0));
    assert_eq!((gw.wakes(), starting.wakes()), (0, 0));
    gw.no_call();
    starting.no_call();
}

#[test]
fn a_spend_ceiling_answer_ends_the_run_stopped_route_naming_the_ceiling() {
    let gw = Gateway::start("stopped", (503, json!({"error": {"code": "spend-ceiling", "message": "a pod start would pass the daily cap"}})));
    let (d, _) = daemon("wake-ceiling", &gw, wake_on(500, 100));
    let run = submit(&d, "qwen");
    // the clock does not move: the run ends on the answer, not at the end of its wait
    let r = until_state(&d, &run, "Ended");
    assert_eq!((r["outcome"].clone(), r["route_phase"].clone(), r["started_at"].clone()), (json!("stopped: route"), json!("stopped"), Value::Null), "{r}");
    assert_eq!(records(&d, &run, "runbook.route-wake-answered"), vec![json!({"route": "qwen", "status": 503, "code": "spend-ceiling"})]);
    assert_eq!(records(&d, &run, "runbook.route-wait-ended"), vec![json!({"route": "qwen", "phase": "stopped", "ceiling": "spend-ceiling"})]);
    assert_eq!(gw.wakes(), 1);
    // the wake's record at its send holds the figure it reserved; its 503 answer then books nothing (story
    // a-wake-is-booked-only-when-it-succeeds, the cases below)
    assert_eq!(records(&d, &run, "runbook.route-wake")[0]["booked_micro_usd"], 100);
    gw.no_call();
}

/// The records of the run's events of `kind`, once there are `n`.
fn until_n_records(d: &Daemon, run: &str, kind: &str, n: usize) -> Vec<Value> {
    let limit = Instant::now() + Duration::from_secs(15);
    loop {
        let r = records(d, run, kind);
        if r.len() >= n {
            return r;
        }
        assert!(Instant::now() < limit, "the run has {} events {kind}, not {n}: {:?}", r.len(), d.events_after(run, 0));
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The gateway's answer to a wake whose pod could not start (the gateway logged `no-capacity`).
fn unavailable() -> WakeSaid {
    WakeSaid::Answer(503, json!({"error": {"code": "target-unavailable", "message": "the pod start failed: no-capacity"}}))
}

fn sent(booked: u64, cap: u64) -> Value {
    json!({"route": "qwen", "sent": true, "day": "2026-11-26", "cost_micro_usd": 100, "booked_micro_usd": booked, "cap_micro_usd": cap})
}

/// Story `a-wake-is-booked-only-when-it-succeeds`: a wake the gateway answered 503 books nothing, the run's record names
/// the gateway's code, and the run's wait sends the wake again after a pause of 30 s of the daemon's clock. The third
/// wake is answered 202 and is the only one booked; the run starts once the route is ready, and a fourth wake of the
/// same day (another run's) is still under the cap of 250 that three booked wakes would have passed.
#[test]
fn a_wake_answered_503_books_nothing_and_is_sent_again_after_a_pause_and_only_the_202_is_booked() {
    let gw = Gateway::start_with("stopped", vec![unavailable(), unavailable()], (202, json!({"model": "qwen", "phase": "starting"})));
    let dir = super::state::scratch("wake-retry");
    let clock = Arc::new(SetClock::at("2026-11-26T10:00:00Z").unwrap());
    let d = daemon_in(&dir, &gw, wake_on(250, 100), clock.clone());
    let first = submit(&d, "qwen");
    let answered = until_n_records(&d, &first, "runbook.route-wake-answered", 1);
    assert_eq!(answered, vec![json!({"route": "qwen", "status": 503, "code": "target-unavailable"})]);
    assert_eq!(records(&d, &first, "runbook.route-wake"), vec![sent(100, 250)]);

    // no second wake within the pause: the run reads its route and sends nothing more
    let reads = gw.status_reads();
    clock.set("2026-11-26T10:00:29.999Z");
    gw.until_read_after(reads);
    assert_eq!(gw.wakes(), 1, "a wake was sent again within the pause");

    // the second try: the first booked nothing, so this one's booking is the day's only one
    clock.set("2026-11-26T10:00:30Z");
    let answered = until_n_records(&d, &first, "runbook.route-wake-answered", 2);
    assert_eq!(answered[1], json!({"route": "qwen", "status": 503, "code": "target-unavailable"}));
    assert_eq!(records(&d, &first, "runbook.route-wake"), vec![sent(100, 250), sent(100, 250)]);

    // the third try is answered 202 and booked once
    clock.set("2026-11-26T10:01:00Z");
    let answered = until_n_records(&d, &first, "runbook.route-wake-answered", 3);
    assert_eq!(answered[2], json!({"route": "qwen", "status": 202}));
    assert_eq!(records(&d, &first, "runbook.route-wake"), vec![sent(100, 250), sent(100, 250), sent(100, 250)]);
    assert_eq!(gw.wakes(), 3);

    // the pod is ready: the run starts and ends
    gw.set_phase("ready");
    clock.set("2026-11-26T10:01:05Z");
    let r = until_state(&d, &first, "Ended");
    assert_eq!(r["outcome"], "done", "{r}");

    // a wake of another run later that day (after the wait of the last one) books on top of the one 202: 200 of 250
    gw.set_phase("stopped");
    clock.set("2026-11-26T10:11:00Z");
    let second = submit(&d, "qwen");
    until_n_records(&d, &second, "runbook.route-wake-answered", 1);
    assert_eq!(records(&d, &second, "runbook.route-wake"), vec![sent(200, 250)]);
    assert_eq!(gw.wakes(), 4);

    // the booking is the journal's: a daemon started again counts the one 202 and the second run's wake, not the 503s
    d.stop();
    drop(d);
    gw.set_phase("stopped");
    clock.set("2026-11-26T10:30:00Z");
    let d = daemon_in(&dir, &gw, wake_on(350, 100), clock.clone());
    let third = submit(&d, "qwen");
    until_n_records(&d, &third, "runbook.route-wake-answered", 1);
    assert_eq!(records(&d, &third, "runbook.route-wake"), vec![sent(300, 350)]);
    gw.no_call();
}

/// A wake answered 5xx is sent again only within the run's wait: the route's `start_wait_seconds` of 60 lets two tries
/// (0 s and 30 s) through, and the wait ends the run `stopped: route` at 60 s without a third. Neither 503 stays booked.
#[test]
fn a_wake_answered_5xx_is_sent_again_within_the_wait_and_never_past_it() {
    let gw = Gateway::start_with("stopped", vec![unavailable(), unavailable()], (202, json!({})));
    let dir = super::state::scratch("wake-retry-bound");
    let clock = Arc::new(SetClock::at("2026-11-26T10:00:00Z").unwrap());
    let mut wake = wake_on(250, 100);
    wake["start_wait_seconds"] = json!(60);
    let d = daemon_in(&dir, &gw, wake.clone(), clock.clone());
    let run = submit(&d, "qwen");
    until_n_records(&d, &run, "runbook.route-wake-answered", 1);
    clock.set("2026-11-26T10:00:30Z");
    until_n_records(&d, &run, "runbook.route-wake-answered", 2);
    // the next try would fall at the end of the wait: none is sent, the run ends on its wait
    clock.set("2026-11-26T10:01:00Z");
    let r = until_state(&d, &run, "Ended");
    assert_eq!(r["outcome"], "stopped: route", "{r}");
    assert_eq!(gw.wakes(), 2, "a wake was sent at or past the end of the wait");
    let tries = records(&d, &run, "runbook.route-wake");
    assert_eq!(tries.len(), 2, "each try is one line of the run's record: {tries:?}");
    assert!(tries.iter().all(|t| t["booked_micro_usd"] == 100), "{tries:?}");

    // the next run of the day (after the wait of the last try) books its wake on top of nothing
    clock.set("2026-11-26T10:02:00Z");
    let next = submit(&d, "qwen");
    until_n_records(&d, &next, "runbook.route-wake-answered", 1);
    assert_eq!(records(&d, &next, "runbook.route-wake"), vec![sent(100, 250)]);
    gw.no_call();
}

/// A wake whose answer never comes (the stand-in holds it past the daemon's bound of 5 s) books nothing, its record has
/// `status` null, and it is sent again after the pause like a 5xx. Takes about 5 s: the bound of a wake.
#[test]
fn a_wake_with_no_answer_books_nothing_and_is_sent_again_after_the_pause() {
    let gw = Gateway::start_with("stopped", vec![WakeSaid::Silent], (202, json!({})));
    let dir = super::state::scratch("wake-retry-silent");
    let clock = Arc::new(SetClock::at("2026-11-26T10:00:00Z").unwrap());
    let d = daemon_in(&dir, &gw, wake_on(250, 100), clock.clone());
    let run = submit(&d, "qwen");
    let answered = until_n_records(&d, &run, "runbook.route-wake-answered", 1);
    assert_eq!(answered, vec![json!({"route": "qwen", "status": null})]);
    clock.set("2026-11-26T10:00:30Z");
    let answered = until_n_records(&d, &run, "runbook.route-wake-answered", 2);
    assert_eq!(answered[1], json!({"route": "qwen", "status": 202}));
    assert_eq!(records(&d, &run, "runbook.route-wake"), vec![sent(100, 250), sent(100, 250)]);
    assert_eq!(gw.wakes(), 2);
    gw.no_call();
}

// ---- story a-managed-route-books-its-pods-measured-time ----

/// The wake keys of `wake_on` and the pod's price per hour, 3.00 USD.
fn priced(cap: u64, cost: u64) -> Value {
    let mut wake = wake_on(cap, cost);
    wake["price_per_hour_micro_usd"] = json!(3_000_000);
    wake
}

/// A run of the runbook `held` on `route`: it waits on one decision until [`answer`] ends it.
fn submit_held(d: &Daemon, route: &str) -> String {
    let o = d.submit_run(SubmitRun { runbook_id: "held".into(), intake: "x".into(), autonomy: Some(Autonomy::A5), decider: Some("model".into()), model_route: Some(route.into()), ..SubmitRun::default() }).unwrap();
    assert_eq!(o.name, "submitted", "{o:?}");
    let run = o.run_id.unwrap();
    until_state(d, &run, "WaitingOnDecision");
    run
}

/// Answers the decision of a run of `held`, and waits until the run ended.
fn answer(d: &Daemon, run: &str) -> Value {
    let o = d.answer_decision(AnswerDecision { run_id: run.into(), request_id: format!("{run}-q1"), answer: "A".into(), answered_by: "operator".into() }).unwrap();
    assert!(!o.is_refusal(), "{o:?}");
    until_state(d, run, "Ended")
}

fn pod_time(pod_millis: u64, share_millis: u64, cost: u64) -> Value {
    json!({"route": "qwen", "pod_millis": pod_millis, "share_millis": share_millis, "price_per_hour_micro_usd": 3_000_000, "cost_micro_usd": cost})
}

/// A run alone on a ready pod books the pod's time from its start (the pod read ready) to the pod's stop, which the
/// daemon reads while the run lives, at the route's price per hour: 20 minutes at 3.00 USD is 1.00 USD. The time
/// after the stop, while the run still waits on its decision, books nothing.
#[test]
fn a_run_alone_on_a_ready_pod_books_its_time_to_the_pods_stop_at_the_price_per_hour() {
    let gw = Gateway::start("ready", (202, json!({})));
    let (d, clock) = daemon("pod-alone", &gw, priced(10_000_000, 500_000));
    let run = submit_held(&d, "qwen");
    clock.set("2026-11-26T10:20:00Z");
    gw.set_phase("stopped");
    assert_eq!(until_records(&d, &run, "runbook.route-pod-phase"), vec![json!({"route": "qwen", "phase": "stopped"})]);
    clock.set("2026-11-26T10:40:00Z");
    assert_eq!(answer(&d, &run)["outcome"], "done");
    assert_eq!(records(&d, &run, "runbook.route-pod-time"), vec![pod_time(1_200_000, 1_200_000, 1_000_000)]);
    gw.no_call();
}

/// The split rule: each stretch of the pod's time is split equally over the runs that used the pod in it. Two runs
/// that share 30 minutes at 3.00 USD per hour book 0.75 USD each. Two runs that overlap for 15 of 45 minutes book
/// 15 minutes alone and half of the 15 shared: 1.125 USD each, and the two shares add up to 45 minutes at the price.
#[test]
fn runs_that_share_a_pod_split_its_time_equally_over_each_stretch_they_share() {
    let gw = Gateway::start("ready", (202, json!({})));
    let (d, clock) = daemon("pod-shared", &gw, priced(10_000_000, 500_000));
    let (a, b) = (submit_held(&d, "qwen"), submit_held(&d, "qwen"));
    clock.set("2026-11-26T10:30:00Z");
    answer(&d, &a);
    answer(&d, &b);
    assert_eq!(records(&d, &a, "runbook.route-pod-time"), vec![pod_time(1_800_000, 900_000, 750_000)]);
    assert_eq!(records(&d, &b, "runbook.route-pod-time"), vec![pod_time(1_800_000, 900_000, 750_000)]);

    clock.set("2026-11-26T11:00:00Z");
    let c = submit_held(&d, "qwen");
    clock.set("2026-11-26T11:15:00Z");
    let e = submit_held(&d, "qwen");
    clock.set("2026-11-26T11:30:00Z");
    answer(&d, &c);
    clock.set("2026-11-26T11:45:00Z");
    answer(&d, &e);
    let (c, e) = (records(&d, &c, "runbook.route-pod-time"), records(&d, &e, "runbook.route-pod-time"));
    assert_eq!(c, vec![pod_time(1_800_000, 1_350_000, 1_125_000)]);
    assert_eq!(e, vec![pod_time(1_800_000, 1_350_000, 1_125_000)]);
    // 45 minutes at 3.00 USD per hour
    assert_eq!(c[0]["cost_micro_usd"].as_u64().unwrap() + e[0]["cost_micro_usd"].as_u64().unwrap(), 2_250_000);
    gw.no_call();
}

/// The route's daily cap counts its wakes and its pod's time together. A run whose pod never became ready books its
/// wake and no time; a run on the woken pod books 30 minutes (1.50 USD); the next wake would bring the day to 3.00 USD,
/// past the cap of 2.80 USD, so it is not sent and the run waits as a run past the wake cap does.
#[test]
fn the_daily_cap_counts_wakes_and_pod_time_together_and_a_pod_never_ready_books_no_time() {
    let gw = Gateway::start("stopped", (202, json!({"model": "qwen", "phase": "starting"})));
    let dir = super::state::scratch("pod-cap");
    let clock = Arc::new(SetClock::at("2026-11-26T09:00:00Z").unwrap());
    let d = daemon_in(&dir, &gw, priced(2_800_000, 500_000), clock.clone());
    let wake = |booked: u64, pod: u64| json!({"route": "qwen", "sent": true, "day": "2026-11-26", "cost_micro_usd": 500_000, "booked_micro_usd": booked, "pod_micro_usd": pod, "cap_micro_usd": 2_800_000});

    // the pod never becomes ready: the run ends on its wait, its wake booked, no time
    let never = submit(&d, "qwen");
    until_records(&d, &never, "runbook.route-wake-answered");
    assert_eq!(records(&d, &never, "runbook.route-wake"), vec![wake(500_000, 0)]);
    clock.set("2026-11-26T09:10:00Z");
    assert_eq!(until_state(&d, &never, "Ended")["outcome"], "stopped: route");
    assert!(records(&d, &never, "runbook.route-pod-time").is_empty(), "{:?}", d.events_after(&never, 0));

    // a woken pod that a run uses for 30 minutes
    clock.set("2026-11-26T10:00:00Z");
    let o = d.submit_run(SubmitRun { runbook_id: "held".into(), intake: "x".into(), autonomy: Some(Autonomy::A5), decider: Some("model".into()), model_route: Some("qwen".into()), ..SubmitRun::default() }).unwrap();
    let used = o.run_id.unwrap();
    until_records(&d, &used, "runbook.route-wake-answered");
    assert_eq!(records(&d, &used, "runbook.route-wake"), vec![wake(1_000_000, 0)]);
    gw.set_phase("ready");
    clock.set("2026-11-26T10:00:05Z");
    until_state(&d, &used, "WaitingOnDecision");
    clock.set("2026-11-26T10:30:05Z");
    answer(&d, &used);
    assert_eq!(records(&d, &used, "runbook.route-pod-time"), vec![pod_time(1_800_000, 1_800_000, 1_500_000)]);

    // the day booked 1.00 USD of wakes and 1.50 USD of pod time: a wake of 0.50 USD would pass the cap of 2.80 USD
    gw.set_phase("stopped");
    clock.set("2026-11-26T11:00:00Z");
    let late = submit(&d, "qwen");
    let refused = until_records(&d, &late, "runbook.route-wake");
    assert_eq!(refused, vec![json!({"route": "qwen", "sent": false, "reason": "wake cap reached", "day": "2026-11-26", "cost_micro_usd": 500_000, "booked_micro_usd": 2_500_000, "pod_micro_usd": 1_500_000, "cap_micro_usd": 2_800_000})]);
    assert_eq!(detail(&d, &late)["state"], "WaitingForRoute");
    assert_eq!(gw.wakes(), 2, "a wake was sent past the day's cap of wakes and pod time");

    // the booking is the journal's: a daemon started again counts the pod's time too
    d.stop();
    drop(d);
    clock.set("2026-11-26T11:30:00Z");
    let d = daemon_in(&dir, &gw, priced(2_800_000, 500_000), clock.clone());
    let again = submit(&d, "qwen");
    let refused = until_records(&d, &again, "runbook.route-wake");
    assert_eq!((refused[0]["sent"].clone(), refused[0]["booked_micro_usd"].clone()), (json!(false), json!(2_500_000)), "{refused:?}");
    gw.no_call();
}

/// The pod's price is a key of a managed route only, like the wake keys.
#[test]
fn a_pod_price_on_a_route_that_is_not_managed_is_refused_at_start_by_name() {
    let file = "[gateways.gw]\nbase_url = \"http://127.0.0.1:9\"\n[routes.claude]\ngateway = \"gw\"\nmodel = \"claude\"\nkind = \"hosted\"\nprice_per_hour_micro_usd = 3000000\n";
    let (status, err, wrote) = serve("pod-price-hosted", file);
    assert_eq!(status, Some(2), "the daemon started: {err}");
    assert!(err.contains("routes.claude.price_per_hour_micro_usd") && err.contains("managed route only"), "the refusal does not name the key: {err}");
    assert!(!wrote, "the state directory was written");
}
