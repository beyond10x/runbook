//! Adversary A on the wave "model routes": routes and money (`docs/model-routes.md`, sections 2, 3, 8 and 9; stories
//! `run-names-a-model-route`, `route-readiness-before-a-run`, `daemon-takes-a-delegation`,
//! `delegation-classifies-or-builds-a-protocol`).
//!
//! Each case states what the design and the stories hold and fails where the code does not. No model is called and
//! nothing leaves loopback: the gateway is a stand-in on a port of the test's own, and the daemon runs in process on a
//! test clock with the stub runtime, or as `runbook serve` with the Loom runtime for the classifier's calls.

use super::default_confinement::rows;
use super::http::send;
use runbook::clock::SetClock;
use runbook::commands::{Daemon, Fault, Outcome, Policy, SubmitRun};
use runbook::routes::{RouteTable, Tables};
use runbook::runtime::{Autonomy, Confinement};
use runbook::stub::{Script, Step, StubRuntime};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// What the stand-in answers: the status of `/ready`, the aliases `/v1/models` lists and the phase of each alias at
/// `/v1/models/{alias}/status` (an alias without one answers 404 there).
struct Said {
    ready: u16,
    listed: Vec<&'static str>,
    phases: BTreeMap<&'static str, &'static str>,
}

/// A stand-in gateway: it keeps the request line of every request and the `model` of every POST, and answers a POST
/// with one Messages answer that carries usage.
struct Gateway {
    base_url: String,
    said: Arc<Mutex<Said>>,
    lines: Arc<Mutex<Vec<String>>>,
    models: Arc<Mutex<Vec<String>>>,
}

impl Gateway {
    fn start(listed: Vec<&'static str>, phases: &[(&'static str, &'static str)]) -> Gateway {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let said = Arc::new(Mutex::new(Said { ready: 200, listed, phases: phases.iter().copied().collect() }));
        let (lines, models) = (Arc::new(Mutex::new(Vec::new())), Arc::new(Mutex::new(Vec::new())));
        let (s, kept, posted) = (said.clone(), lines.clone(), models.clone());
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
                let text = String::from_utf8_lossy(&head).into_owned();
                let line: String = text.lines().next().unwrap_or("").rsplit_once(' ').map(|(l, _)| l.to_owned()).unwrap_or_default();
                kept.lock().unwrap().push(line.clone());
                let (code, body) = if line.starts_with("POST ") {
                    let length = text.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
                    let mut body = vec![0u8; length];
                    let _ = stream.read_exact(&mut body);
                    let sent: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
                    posted.lock().unwrap().push(sent["model"].as_str().unwrap_or("").to_owned());
                    (200, json!({"id": "m", "type": "message", "role": "assistant", "content": [{"type": "text", "text": "no json"}], "usage": {"input_tokens": 7, "output_tokens": 3}}))
                } else {
                    let said = s.lock().unwrap();
                    let path = line.strip_prefix("GET ").unwrap_or("");
                    match path {
                        "/ready" => (said.ready, json!({"ready": said.ready == 200})),
                        "/v1/models" => (200, json!({"object": "list", "data": said.listed.iter().map(|id| json!({"id": id, "wires": ["messages", "chat"]})).collect::<Vec<_>>()})),
                        _ => match path.strip_prefix("/v1/models/").and_then(|p| p.strip_suffix("/status")).and_then(|a| said.phases.get(a)) {
                            Some(phase) => (200, json!({"phase": phase})),
                            None => (404, json!({"error": {"code": "not-found"}})),
                        },
                    }
                };
                let body = body.to_string();
                let _ = stream.write_all(format!("HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            }
        });
        Gateway { base_url, said, lines, models }
    }

    fn lines(&self) -> Vec<String> {
        self.lines.lock().unwrap().clone()
    }

    fn posted_to(&self, alias: &str) -> usize {
        self.models.lock().unwrap().iter().filter(|m| *m == alias).count()
    }
}

/// A daemon on a test clock with the decider kind `model`, the stub runtime (its runbook `quick` ends at once) and the
/// routes `routes` on the gateway `gw`.
fn daemon(tag: &str, gw: &Gateway, routes: Value) -> (Daemon, Arc<SetClock>) {
    let dir = super::state::scratch(tag);
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    std::fs::create_dir_all(dir.join("runbooks")).unwrap();
    let gateways = json!({"local": {"base_url": gw.base_url}});
    let mut table = RouteTable::declare(&Tables { gateways: Some(&gateways), routes: Some(&routes), ..Tables::default() }).unwrap();
    table.read_tokens().unwrap();
    let stub = Arc::new(StubRuntime::new());
    stub.script("quick", Script::new(vec![Step::End("done".into())]));
    let clock = Arc::new(SetClock::at("2026-11-26T10:00:00Z").unwrap());
    let policy = Policy { default_confinement: Some(Confinement::Linux), kinds: vec!["model".into()], routes: Arc::new(table), ..Policy::default() };
    let d = Daemon::start_full(super::commands::config(&dir), policy, stub, clock.clone()).unwrap();
    (d, clock)
}

fn model_submit(route: &str) -> SubmitRun {
    SubmitRun { runbook_id: "quick".into(), intake: "x".into(), autonomy: Some(Autonomy::A5), decider: Some("model".into()), model_route: Some(route.into()), ..SubmitRun::default() }
}

fn state_of(d: &Daemon, run: &str) -> Value {
    d.run_detail(run)[0].clone()
}

fn until_state(d: &Daemon, run: &str, want: &str, within: Duration) -> Value {
    let limit = Instant::now() + within;
    loop {
        let r = state_of(d, run);
        if r["state"] == want || Instant::now() >= limit {
            return r;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn outcome(reply: Result<Outcome, Fault>) -> Result<(&'static str, Option<String>), String> {
    match reply {
        Ok(o) => Ok((o.name, o.run_id)),
        Err(Fault::Refused { status, what, message }) => Err(format!("{status} {what}: {message}")),
        Err(other) => Err(format!("{other:?}")),
    }
}

/// A client that retries a submit with its idempotency key (its first answer was lost) gets the run it already made, as
/// a delegation does ("a replay is not asked about", `Daemon::delegate_for`): the gateway's readiness at the retry is
/// no reason to answer 503 for a run that exists. SubmitRun: "a second submit with a key already used returns the first
/// run and creates none (outcome already-submitted, answered with the first run)".
#[test]
fn adversary_a_a_replayed_submit_answers_its_first_run_while_the_gateway_is_not_ready() {
    let gw = Gateway::start(vec!["claude"], &[]);
    let (d, _) = daemon("adv-a-replay", &gw, json!({"claude": {"gateway": "local", "model": "claude", "kind": "hosted"}}));
    let keyed = || SubmitRun { idempotency_key: Some("adv-a-key-1".into()), ..model_submit("claude") };
    let (name, first) = outcome(d.submit_run(keyed())).expect("the first submit");
    assert_eq!(name, "submitted");
    gw.said.lock().unwrap().ready = 503;
    let replay = outcome(d.submit_run(keyed()));
    assert_eq!(replay, Ok(("already-submitted", first)), "a retry of a submit whose run exists was not answered with that run");
    assert_eq!(d.runs().len(), 1);
}

/// A run on two routes that cannot answer waits at most the least `start_wait_seconds` of them (`RouteWait::wait_millis`:
/// "the least `start_wait_seconds` of the routes that could not answer"; `docs/model-routes.md`, section 3: a route not
/// callable makes the run wait up to its `start_wait_seconds`, then end `stopped: route`). The role `writer` is on
/// `b-pod`, which waits 10 s; 30 s later the run still waits, held by the 600 s of the role `default`'s route.
#[test]
fn adversary_a_a_run_on_two_routes_that_cannot_answer_waits_no_longer_than_the_least_wait() {
    let gw = Gateway::start(vec!["a-pod", "b-pod"], &[("a-pod", "stopped"), ("b-pod", "stopped")]);
    let routes = json!({
        "a-pod": {"gateway": "local", "model": "a-pod", "kind": "managed", "start_wait_seconds": 600},
        "b-pod": {"gateway": "local", "model": "b-pod", "kind": "managed", "start_wait_seconds": 10},
    });
    let (d, clock) = daemon("adv-a-least-wait", &gw, routes);
    let input = SubmitRun { model_routes: Some(BTreeMap::from([("writer".to_owned(), "b-pod".to_owned())])), ..model_submit("a-pod") };
    let (name, run) = outcome(d.submit_run(input)).expect("the submit");
    assert_eq!(name, "submitted");
    let run = run.unwrap();
    let r = until_state(&d, &run, "WaitingForRoute", Duration::from_secs(10));
    assert_eq!(r["state"], "WaitingForRoute", "{r}");
    clock.set("2026-11-26T10:00:05Z");
    std::thread::sleep(Duration::from_millis(300));
    clock.set("2026-11-26T10:00:30Z");
    let r = until_state(&d, &run, "Ended", Duration::from_secs(3));
    assert_eq!((r["state"].clone(), r["outcome"].clone()), (json!("Ended"), json!("stopped: route")), "30 s after the submit the run still waits past the 10 s of b-pod: {r}");
    assert!(gw.lines().iter().all(|l| l.starts_with("GET ")), "{:?}", gw.lines());
}

/// The classifier route of a delegation that names no protocol is a route its run calls (`docs/model-routes.md`,
/// sections 3 and 9; story `route-readiness-before-a-run`: "A route turning callable during the wait starts the run with
/// no call before"). A managed classifier route whose model is `stopped` gets no call: the delegation waits for it or is
/// refused. The daemon reads only the delegation's own route and sends the classify call to the stopped model (a call
/// that, on a gateway whose `start_on_request` is on, creates a billed pod).
#[test]
fn adversary_a_a_delegation_sends_no_classify_call_to_a_classifier_route_that_is_not_callable() {
    let gw = Gateway::start(vec!["a-model", "c-model"], &[("c-model", "stopped")]);
    let tables = format!("{}[routes.classify]\ngateway = \"default\"\nmodel = \"c-model\"\nkind = \"managed\"\n[route_policy]\nclassifier = \"classify\"\n", super::delegate::ROUTE);
    let (_h, l, _roots) = super::delegate::daemon_at("adv-a-classifier-stopped", &gw.base_url, &tables);
    let body = json!({"request": "green or black", "model_route": "default"}).to_string();
    let r = send(l.addr, "POST", "/daemon/commands/delegate", &[("Content-Type", "application/json")], Some(body.as_bytes()));
    if r.status == 202 {
        let run = r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned();
        let limit = Instant::now() + Duration::from_secs(20);
        loop {
            let row = rows(&l, &format!("/daemon/views/run?run_id={run}"))[0].clone();
            if ["Ended", "FailedToStart", "Cancelled", "WaitingForRoute"].iter().any(|s| row["state"] == *s) || Instant::now() >= limit {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    assert_eq!(gw.posted_to("c-model"), 0, "the classify call went to the classifier route while its model was stopped (delegate answered {}): {:?}", r.status, gw.lines());
}
