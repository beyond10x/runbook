//! A run starts only on a route that can answer (`docs/model-routes.md`, section 3; story
//! `route-readiness-before-a-run`). At the submit the daemon asks the gateway of each declared route of the run
//! `GET /ready`, `GET /v1/models` and the status of the route's model: a gateway that is not ready refuses the submit
//! `503 route-unavailable`, an alias it does not list `route-refused`, and a model that cannot answer yet holds the run
//! in `WaitingForRoute`: the status is read again at most once every 5 s of the daemon's clock, up to the route's
//! `start_wait_seconds`; then the run starts, or ends `stopped: route` with the last phase. No call and no wake.
//!
//! In process, on a test clock, with the stub runtime: no model is called and nothing leaves loopback. The gateway is
//! a stand-in on a port of the test's own whose `/ready` status and model phase the case sets, and which keeps the
//! request line of everything it is sent.

use runbook::clock::SetClock;
use runbook::commands::{CancelRun, Daemon, Fault, Policy, SubmitRun};
use runbook::routes::{RouteTable, Tables};
use runbook::runtime::{Autonomy, Confinement};
use runbook::stub::{Script, Step, StubRuntime};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// What the stand-in answers: the status of `/ready`, the aliases `/v1/models` lists, and the phase of `qwen`.
struct Said {
    ready: u16,
    listed: Vec<&'static str>,
    phase: &'static str,
}

struct Gateway {
    base_url: String,
    said: Arc<Mutex<Said>>,
    lines: Arc<Mutex<Vec<String>>>,
}

impl Gateway {
    fn start(ready: u16, listed: Vec<&'static str>, phase: &'static str) -> Gateway {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let said = Arc::new(Mutex::new(Said { ready, listed, phase }));
        let lines = Arc::new(Mutex::new(Vec::new()));
        let (s, kept) = (said.clone(), lines.clone());
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
                let line: String = String::from_utf8_lossy(&head).lines().next().unwrap_or("").rsplit_once(' ').map(|(l, _)| l.to_owned()).unwrap_or_default();
                kept.lock().unwrap().push(line.clone());
                let (code, body) = {
                    let said = s.lock().unwrap();
                    match line.as_str() {
                        "GET /ready" => (said.ready, json!({"ready": said.ready == 200})),
                        "GET /v1/models" => (200, json!({"object": "list", "data": said.listed.iter().map(|id| json!({"id": id, "wires": ["chat"]})).collect::<Vec<_>>()})),
                        "GET /v1/models/qwen/status" => (200, json!({"model": "qwen", "phase": said.phase})),
                        _ => (404, json!({"error": {"code": "not-found"}})),
                    }
                };
                let body = body.to_string();
                let _ = stream.write_all(format!("HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            }
        });
        Gateway { base_url, said, lines }
    }

    fn lines(&self) -> Vec<String> {
        self.lines.lock().unwrap().clone()
    }

    fn count(&self, line: &str) -> usize {
        self.lines().iter().filter(|l| *l == line).count()
    }

    /// Only reads went to the gateway: no model call and no wake.
    fn only_reads(&self) {
        let lines = self.lines();
        assert!(lines.iter().all(|l| l.starts_with("GET ")), "the gateway got more than reads: {lines:?}");
    }
}

/// A daemon on a test clock with the decider kind `model`, the stub runtime and two routes on `gw`: `qwen` (managed,
/// waits `wait` seconds) and `claude` (hosted).
fn daemon(tag: &str, gw: &Gateway, wait: u64) -> (Daemon, Arc<SetClock>) {
    let dir = super::state::scratch(tag);
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    std::fs::create_dir_all(dir.join("runbooks")).unwrap();
    let gateways = json!({"local": {"base_url": gw.base_url}});
    let routes = json!({
        "qwen": {"gateway": "local", "model": "qwen", "kind": "managed", "start_wait_seconds": wait},
        "claude": {"gateway": "local", "model": "claude", "kind": "hosted"},
    });
    let mut table = RouteTable::declare(&Tables { gateways: Some(&gateways), routes: Some(&routes), ..Tables::default() }).unwrap();
    table.read_tokens().unwrap();
    let stub = Arc::new(StubRuntime::new());
    stub.script("quick", Script::new(vec![Step::End("done".into())]));
    let clock = Arc::new(SetClock::at("2026-11-26T10:00:00Z").unwrap());
    let policy = Policy { default_confinement: Some(Confinement::Linux), kinds: vec!["model".into()], routes: Arc::new(table), ..Policy::default() };
    let d = Daemon::start_full(super::commands::config(&dir), policy, stub, clock.clone()).unwrap();
    (d, clock)
}

fn submit(d: &Daemon, route: &str) -> Result<runbook::commands::Outcome, Fault> {
    d.submit_run(SubmitRun { runbook_id: "quick".into(), intake: "x".into(), autonomy: Some(Autonomy::A5), decider: Some("model".into()), model_route: Some(route.into()), ..SubmitRun::default() })
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

fn kinds(d: &Daemon, run: &str) -> Vec<String> {
    d.events_after(run, 0).iter().filter_map(|e| e["kind"].as_str().map(str::to_owned)).collect()
}

#[test]
fn a_gateway_that_is_not_ready_refuses_the_submit_503_route_unavailable_and_nothing_is_written() {
    let gw = Gateway::start(503, vec!["qwen", "claude"], "ready");
    let (d, _) = daemon("ready-503", &gw, 600);
    match submit(&d, "claude") {
        Err(Fault::Refused { status, what, message }) => {
            assert_eq!((status, what), (503, "route-unavailable"), "{message}");
            assert!(message.contains("claude") && message.contains("gateway not ready") && !message.contains(&gw.base_url), "{message}");
        }
        other => panic!("not refused 503: {other:?}"),
    }
    assert!(d.runs().is_empty(), "a run was written: {:?}", d.runs());
    assert_eq!(gw.lines(), vec!["GET /ready"], "after a gateway that is not ready nothing more is asked");
}

#[test]
fn an_alias_the_gateway_does_not_list_refuses_the_submit_route_refused_and_nothing_is_written() {
    let gw = Gateway::start(200, vec!["claude"], "ready");
    let (d, _) = daemon("ready-unlisted", &gw, 600);
    let o = submit(&d, "qwen").unwrap();
    assert_eq!((o.name, o.status), ("route-refused", 422), "{o:?}");
    let p = o.payload.unwrap();
    assert!(p["reason"].as_str().unwrap().contains("serves no model qwen"), "{p}");
    assert_eq!(p["declared"], json!(["claude", "qwen"]));
    assert!(d.runs().is_empty());
    gw.only_reads();
}

#[test]
fn a_managed_route_that_turns_callable_during_the_wait_starts_the_run_and_nothing_was_called_before() {
    let gw = Gateway::start(200, vec!["qwen", "claude"], "starting");
    let (d, clock) = daemon("ready-comes-up", &gw, 600);
    let o = submit(&d, "qwen").unwrap();
    assert_eq!(o.name, "submitted");
    let run = o.run_id.unwrap();
    let r = until_state(&d, &run, "WaitingForRoute");
    assert_eq!(r["route_phase"], "starting", "{r}");
    let reads = gw.count("GET /v1/models/qwen/status");
    // the clock has not moved 5 s: no read
    clock.set("2026-11-26T10:00:04.999Z");
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(gw.count("GET /v1/models/qwen/status"), reads, "the status was read again within 5 s of the daemon's clock");
    assert_eq!(detail(&d, &run)["state"], "WaitingForRoute");
    // still starting at the next read: the run waits on
    clock.set("2026-11-26T10:00:05Z");
    let limit = Instant::now() + Duration::from_secs(10);
    while gw.count("GET /v1/models/qwen/status") == reads {
        assert!(Instant::now() < limit, "no read 5 s later");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(detail(&d, &run)["state"], "WaitingForRoute");
    gw.said.lock().unwrap().phase = "ready";
    clock.set("2026-11-26T10:00:10Z");
    let r = until_state(&d, &run, "Ended");
    assert_eq!((r["outcome"].clone(), r["started_at"].clone()), (json!("done"), json!("2026-11-26T10:00:10.000Z")), "{r}");
    let k = kinds(&d, &run);
    assert!(k.contains(&"runbook.waiting-for-route".to_owned()) && !k.contains(&"runbook.route-wait-ended".to_owned()), "{k:?}");
    gw.only_reads();
}

#[test]
fn past_its_wait_the_run_ends_stopped_route_naming_the_last_phase_and_nothing_was_called() {
    let gw = Gateway::start(200, vec!["qwen", "claude"], "stopped");
    let (d, clock) = daemon("ready-stays-down", &gw, 12);
    let run = submit(&d, "qwen").unwrap().run_id.unwrap();
    until_state(&d, &run, "WaitingForRoute");
    gw.said.lock().unwrap().phase = "starting";
    for at in ["2026-11-26T10:00:05Z", "2026-11-26T10:00:10Z"] {
        clock.set(at);
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(detail(&d, &run)["state"], "WaitingForRoute", "at {at}");
    }
    clock.set("2026-11-26T10:00:12Z");
    let r = until_state(&d, &run, "Ended");
    assert_eq!((r["outcome"].clone(), r["route_phase"].clone(), r["started_at"].clone()), (json!("stopped: route"), json!("starting"), Value::Null), "{r}");
    let ended: Vec<Value> = d.events_after(&run, 0).into_iter().filter(|e| e["kind"] == "runbook.route-wait-ended").map(|e| e["record"].clone()).collect();
    assert_eq!(ended, vec![json!({"route": "qwen", "phase": "starting"})]);
    gw.only_reads();
}

#[test]
fn a_hosted_route_without_a_status_starts_at_once_and_a_cancel_ends_a_wait() {
    let gw = Gateway::start(200, vec!["qwen", "claude"], "stopped");
    let (d, _) = daemon("ready-hosted", &gw, 600);
    let run = submit(&d, "claude").unwrap().run_id.unwrap();
    let r = until_state(&d, &run, "Ended");
    assert_eq!(r["outcome"], "done", "{r}");
    assert!(r.get("route_phase").is_none() && !kinds(&d, &run).contains(&"runbook.waiting-for-route".to_owned()), "{r}");
    let waiting = submit(&d, "qwen").unwrap().run_id.unwrap();
    until_state(&d, &waiting, "WaitingForRoute");
    assert_eq!(d.cancel_run(CancelRun { run_id: waiting.clone() }).unwrap().name, "cancelled");
    let r = until_state(&d, &waiting, "Cancelled");
    assert!(r["started_at"].is_null(), "{r}");
    gw.only_reads();
}
