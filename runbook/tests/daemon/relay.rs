//! The relay of a run's model calls (`runbook::relay`, `docs/model-routes.md` sections 4, 5 and 11), served in this
//! process behind the daemon's guard, with stand-in gateways on loopback ports of the test's own. No model is called.
//!
//! A call carries the run's token and names a role in `model`; the relay sends it to that role's route with the alias
//! as `model` and the gateway's credential, passes the answer back unchanged and books one line per call. A role
//! without a route, a call past the run's caps and a call without the run's token reach no gateway; a gateway that
//! refuses is passed back and booked `route-unavailable` with its code and the setting that chose the route, and no
//! other route is called.

use super::http::{refusing_port, send, Resp};
use runbook::relay::{Booking, Relay, ResolvedRoute, RouteTable};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const GATEWAY_TOKEN: &str = "gw-relay-5d8e02c1a7f4";
const DAEMON_TOKEN: &str = "relay-daemon-token-0123456789-abcdefghij";

/// One request a stand-in gateway received: its head and its body.
#[derive(Clone, Debug)]
pub(super) struct Got {
    pub(super) head: String,
    pub(super) body: Value,
}

/// A gateway on a loopback port that keeps every request and answers each with `answer`.
pub(super) struct Gateway {
    pub(super) base_url: String,
    got: Arc<Mutex<Vec<Got>>>,
}

pub(super) fn gateway(answer: &'static [u8]) -> Gateway {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let got = Arc::new(Mutex::new(Vec::new()));
    let kept = got.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            let mut raw = Vec::new();
            let mut byte = [0u8; 1];
            while !raw.ends_with(b"\r\n\r\n") && raw.len() < 64 * 1024 {
                match stream.read(&mut byte) {
                    Ok(1) => raw.push(byte[0]),
                    _ => break,
                }
            }
            let head = String::from_utf8_lossy(&raw).into_owned();
            let length = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
            let mut body = vec![0u8; length];
            let _ = stream.read_exact(&mut body);
            kept.lock().unwrap().push(Got { head, body: serde_json::from_slice(&body).unwrap_or(Value::Null) });
            let _ = stream.write_all(answer);
        }
    });
    Gateway { base_url, got }
}

pub(super) fn got(g: &Gateway) -> Vec<Got> {
    g.got.lock().unwrap().clone()
}

/// The run's side of the relay: a cap of calls, a reservation, and the lines it booked.
struct Book {
    max_calls: u64,
    calls: AtomicU64,
    lines: Mutex<Vec<(Value, u64)>>,
}

impl Booking for Book {
    fn admit(&self) -> Result<(), String> {
        self.calls.try_update(Ordering::SeqCst, Ordering::SeqCst, |n| (n < self.max_calls).then_some(n + 1)).map(|_| ()).map_err(|n| format!("{n} of {} calls", self.max_calls))
    }
    fn reservation_micro_usd(&self) -> u64 {
        7_000
    }
    fn step(&self) -> Option<String> {
        Some("notes.write".to_owned())
    }
    fn book(&self, line: Value, micro_usd: u64) -> Result<(), String> {
        self.lines.lock().unwrap().push((line, micro_usd));
        Ok(())
    }
}

fn route(name: &str, base_url: &str, alias: &str, prices: Option<(u64, u64)>) -> ResolvedRoute {
    ResolvedRoute {
        name: name.to_owned(),
        base_url: base_url.to_owned(),
        token: Some(GATEWAY_TOKEN.to_owned()),
        alias: alias.to_owned(),
        kind: "hosted".to_owned(),
        price_in_micro_usd_per_mtok: prices.map(|p| p.0),
        price_out_micro_usd_per_mtok: prices.map(|p| p.1),
    }
}

/// The daemon of a fixture with a token, and the relay mounted beside it as `runbook serve` mounts it, in this
/// process on port 0.
struct Served {
    addr: SocketAddr,
    relay: Arc<Relay>,
    _fx: super::commands::Fx,
    _rt: tokio::runtime::Runtime,
}

fn served(tag: &str) -> Served {
    let fx = super::commands::Fx::new(tag);
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let listener = rt.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).unwrap();
    let addr = listener.local_addr().unwrap();
    let relay = Relay::new();
    relay.set_listen(addr);
    let (daemon, extra) = (fx.d().clone(), relay.router());
    rt.spawn(async move {
        let _ = runbook::api::serve_extra(listener, daemon, Some(DAEMON_TOKEN), runbook::api::Streams::default(), Some(extra), std::future::pending()).await;
    });
    Served { addr, relay, _fx: fx, _rt: rt }
}

fn call(s: &Served, base_url: &str, wire: &str, token: &str, body: &Value) -> Resp {
    let path = format!("{}/v1/{wire}", base_url.strip_prefix(&format!("http://{}", s.addr)).expect("the relay's address is the daemon's listener"));
    send(s.addr, "POST", &path, &[("Content-Type", "application/json"), ("Authorization", &format!("Bearer {token}")), ("anthropic-version", "2023-06-01")], Some(body.to_string().as_bytes()))
}

const OK_ANSWER: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nx-llm-gateway-call: call-0001\r\nContent-Length: 78\r\nConnection: close\r\n\r\n{\"id\":\"msg\",\"content\":[],\"usage\":{\"input_tokens\":1200000,\"output_tokens\":300}}";
const STOPPED_ANSWER: &[u8] = b"HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json\r\nContent-Length: 61\r\nConnection: close\r\n\r\n{\"error\":{\"code\":\"model-stopped\",\"message\":\"the pod is off\"}}";
const STREAM_ANSWER: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n16\r\ndata: {\"choices\":[]}\n\n\r\n49\r\ndata: {\"choices\":[],\"usage\":{\"prompt_tokens\":40,\"completion_tokens\":2}}\n\n\r\n0\r\n\r\n";

#[test]
fn a_call_reaches_its_routes_gateway_with_the_alias_and_the_gateways_credential_and_books_one_line_with_tokens_and_cost() {
    let s = served("relay-reach");
    let review = gateway(OK_ANSWER);
    let table = RouteTable::default().with("review", route("claude", &review.base_url, "claude-alias", Some((3_000_000, 15_000_000))), "run");
    let ticket = s.relay.open("run-reach", table).unwrap();
    let book = Arc::new(Book { max_calls: 10, calls: AtomicU64::new(0), lines: Mutex::new(Vec::new()) });
    ticket.take_calls(book.clone());
    let r = call(&s, ticket.base_url(), "messages", ticket.token(), &json!({"model": "review", "max_tokens": 5, "messages": [{"role": "user", "content": "hi"}]}));
    assert_eq!(r.status, 200, "{}", r.text());
    assert_eq!(r.text(), "{\"id\":\"msg\",\"content\":[],\"usage\":{\"input_tokens\":1200000,\"output_tokens\":300}}", "the answer is passed back unchanged");
    assert_eq!(r.header("x-llm-gateway-call"), Some("call-0001"));
    let seen = got(&review);
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].body["model"], "claude-alias", "the gateway got the role, not the alias: {:?}", seen[0]);
    assert_eq!(seen[0].body["messages"][0]["content"], "hi", "the rest of the call is the caller's");
    assert!(seen[0].head.starts_with("POST /v1/messages HTTP/1.1\r\n") && seen[0].head.contains(&format!("Authorization: Bearer {GATEWAY_TOKEN}\r\n")) && seen[0].head.contains("anthropic-version: 2023-06-01\r\n"), "{}", seen[0].head);
    assert!(!seen[0].head.contains(ticket.token()), "the run's token reached the gateway: {}", seen[0].head);
    // the line is booked once the answer has ended
    let lines = wait_for_lines(&book, 1);
    let (line, micro) = &lines[0];
    assert_eq!(*micro, 3_600_000 + 4_500, "1.2 M input tokens at 3 USD and 300 output tokens at 15 USD per million");
    let want = json!({"what": "model call", "run": "run-reach", "step": "notes.write", "role": "review", "route": "claude", "alias": "claude-alias", "source": "run", "wire": "messages", "gateway_call": "call-0001", "input_tokens": 1_200_000, "output_tokens": 300, "cost_known": true, "micro_usd": 3_604_500});
    assert_eq!(*line, want);
    assert!(!line.to_string().contains(GATEWAY_TOKEN) && !line.to_string().contains(ticket.token()));
}

#[test]
fn a_role_without_a_route_a_call_past_the_cap_and_a_call_without_the_runs_token_reach_no_gateway() {
    let s = served("relay-refuse");
    let review = gateway(OK_ANSWER);
    let table = RouteTable::default().with("review", route("claude", &review.base_url, "claude-alias", None), "default");
    let ticket = s.relay.open("run-refuse", table).unwrap();
    let book = Arc::new(Book { max_calls: 1, calls: AtomicU64::new(0), lines: Mutex::new(Vec::new()) });
    // before the run takes calls: 503, nothing sent
    let r = call(&s, ticket.base_url(), "messages", ticket.token(), &json!({"model": "review"}));
    assert_eq!(r.status, 503, "{}", r.text());
    ticket.take_calls(book.clone());
    let r = call(&s, ticket.base_url(), "messages", ticket.token(), &json!({"model": "writer", "messages": []}));
    assert_eq!((r.status, r.json()["error"]["code"].clone()), (400, json!("route-refused")), "{}", r.text());
    assert!(r.text().contains("writer") && r.text().contains("review"), "the refusal names the role and the run's roles: {}", r.text());
    for (token, what) in [("not-the-runs-token", "a wrong token"), (DAEMON_TOKEN, "the daemon's own token")] {
        let r = call(&s, ticket.base_url(), "messages", token, &json!({"model": "review"}));
        assert_eq!(r.status, 401, "{what}: {}", r.text());
    }
    let r = call(&s, ticket.base_url(), "messages", ticket.token(), &json!({"model": "review"}));
    assert_eq!(r.status, 200, "{}", r.text());
    let r = call(&s, ticket.base_url(), "chat/completions", ticket.token(), &json!({"model": "review"}));
    assert_eq!((r.status, r.json()["error"]["code"].clone()), (429, json!("cap-reached")), "{}", r.text());
    assert!(r.text().contains("1 of 1 calls"), "{}", r.text());
    assert_eq!(got(&review).len(), 1, "a refused call reached the gateway");
    assert_eq!(wait_for_lines(&book, 1).len(), 1, "a refused call was booked");
    // the run ended: its token is void
    let base = ticket.base_url().to_owned();
    let token = ticket.token().to_owned();
    drop(ticket);
    let r = call(&s, &base, "messages", &token, &json!({"model": "review"}));
    assert_eq!(r.status, 401, "{}", r.text());
    assert_eq!(got(&review).len(), 1);
}

#[test]
fn a_gateway_refusal_is_passed_back_and_booked_route_unavailable_with_its_code_and_the_setting_and_no_other_route_is_called() {
    let s = served("relay-down");
    let (writer, review) = (gateway(STOPPED_ANSWER), gateway(OK_ANSWER));
    let (_held, dead_port) = refusing_port();
    let table = RouteTable::default()
        .with("writer", route("qwen", &writer.base_url, "qwen-alias", Some((1, 1))), "protocol")
        .with("review", route("claude", &review.base_url, "claude-alias", None), "default")
        .with("local", route("gone", &format!("http://127.0.0.1:{dead_port}"), "gone-alias", None), "run");
    let ticket = s.relay.open("run-down", table).unwrap();
    let book = Arc::new(Book { max_calls: 10, calls: AtomicU64::new(0), lines: Mutex::new(Vec::new()) });
    ticket.take_calls(book.clone());
    let r = call(&s, ticket.base_url(), "messages", ticket.token(), &json!({"model": "writer"}));
    assert_eq!(r.status, 503);
    assert_eq!(r.text(), "{\"error\":{\"code\":\"model-stopped\",\"message\":\"the pod is off\"}}", "the refusal is passed back unchanged");
    let r = call(&s, ticket.base_url(), "messages", ticket.token(), &json!({"model": "local"}));
    assert_eq!((r.status, r.json()["error"]["code"].clone()), (502, json!("route-unavailable")), "{}", r.text());
    assert!(r.text().contains("gone") && r.text().contains("run"), "the answer names the route and the setting: {}", r.text());
    assert_eq!((got(&writer).len(), got(&review).len()), (1, 0), "a failed call was sent to another route");
    let lines = wait_for_lines(&book, 2);
    let line = |route: &str| lines.iter().find(|(l, _)| l["route"] == route).unwrap_or_else(|| panic!("no line of {route}: {lines:?}")).clone();
    let (stopped, micro) = line("qwen");
    assert_eq!((stopped["cause"].clone(), stopped["code"].clone(), stopped["source"].clone(), micro), (json!("route-unavailable"), json!("model-stopped"), json!("protocol"), 0), "{stopped}");
    let (gone, _) = line("gone");
    assert_eq!((gone["cause"].clone(), gone["code"].clone(), gone["source"].clone()), (json!("route-unavailable"), json!("connection-failed"), json!("run")), "{gone}");
}

#[test]
fn a_streamed_chat_call_asks_for_its_usage_and_is_passed_back_as_it_came() {
    let s = served("relay-stream");
    let review = gateway(STREAM_ANSWER);
    let table = RouteTable::default().with("default", route("claude", &review.base_url, "claude-alias", None), "default");
    let ticket = s.relay.open("run-stream", table).unwrap();
    let book = Arc::new(Book { max_calls: 10, calls: AtomicU64::new(0), lines: Mutex::new(Vec::new()) });
    ticket.take_calls(book.clone());
    let r = call(&s, ticket.base_url(), "chat/completions", ticket.token(), &json!({"model": "default", "stream": true, "messages": []}));
    assert_eq!(r.status, 200);
    assert_eq!(r.text(), "data: {\"choices\":[]}\n\ndata: {\"choices\":[],\"usage\":{\"prompt_tokens\":40,\"completion_tokens\":2}}\n\n");
    assert_eq!(r.header("content-type"), Some("text/event-stream"));
    let seen = got(&review);
    assert_eq!(seen[0].body["stream_options"], json!({"include_usage": true}), "{:?}", seen[0]);
    assert!(seen[0].head.starts_with("POST /v1/chat/completions HTTP/1.1\r\n"), "{}", seen[0].head);
    let lines = wait_for_lines(&book, 1);
    let (line, micro) = &lines[0];
    assert_eq!((line["input_tokens"].clone(), line["output_tokens"].clone(), line["cost_known"].clone(), *micro), (json!(40), json!(2), json!(false), 7_000), "no price: the reservation is booked: {line}");
}

/// A gateway on a loopback port that reads one whole call, keeps it, and answers `answer` only once the test sends on
/// the returned channel (or after 20 s).
fn held_gateway(answer: &'static [u8]) -> (Gateway, std::sync::mpsc::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let got = Arc::new(Mutex::new(Vec::new()));
    let kept = got.clone();
    let (release, released) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else { return };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let mut raw = Vec::new();
        let mut byte = [0u8; 1];
        while !raw.ends_with(b"\r\n\r\n") && raw.len() < 64 * 1024 {
            match stream.read(&mut byte) {
                Ok(1) => raw.push(byte[0]),
                _ => break,
            }
        }
        let head = String::from_utf8_lossy(&raw).into_owned();
        let length = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
        let mut body = vec![0u8; length];
        let _ = stream.read_exact(&mut body);
        kept.lock().unwrap().push(Got { head, body: serde_json::from_slice(&body).unwrap_or(Value::Null) });
        let _ = released.recv_timeout(Duration::from_secs(20));
        let _ = stream.write_all(answer);
    });
    (Gateway { base_url, got }, release)
}

#[test]
fn a_call_whose_caller_hangs_up_before_the_gateway_answers_is_still_booked_once() {
    let s = served("relay-hangup");
    let (review, release) = held_gateway(OK_ANSWER);
    let table = RouteTable::default().with("review", route("claude", &review.base_url, "claude-alias", Some((3_000_000, 15_000_000))), "run");
    let ticket = s.relay.open("run-hangup", table).unwrap();
    let book = Arc::new(Book { max_calls: 10, calls: AtomicU64::new(0), lines: Mutex::new(Vec::new()) });
    ticket.take_calls(book.clone());
    // a model process sends its call and is killed (the run's wall cap, a cancel, its own timeout) before the answer
    let body = json!({"model": "review", "max_tokens": 5, "messages": [{"role": "user", "content": "hi"}]}).to_string();
    let path = format!("{}/v1/messages", ticket.base_url().strip_prefix(&format!("http://{}", s.addr)).unwrap());
    let mut caller = std::net::TcpStream::connect(s.addr).unwrap();
    write!(caller, "POST {path} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nAuthorization: Bearer {}\r\nContent-Length: {}\r\n\r\n{body}", s.addr, ticket.token(), body.len()).unwrap();
    let limit = std::time::Instant::now() + Duration::from_secs(10);
    while got(&review).is_empty() {
        assert!(std::time::Instant::now() < limit, "the call never reached the gateway");
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(caller);
    std::thread::sleep(Duration::from_millis(300));
    // the gateway ran the call and answers it, with its usage: the call was sent, so it is one line
    release.send(()).unwrap();
    let lines = wait_for_lines(&book, 1);
    assert_eq!(lines.len(), 1, "{lines:?}");
}

const BROKEN_STREAM: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n76\r\nevent: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":1200000,\"output_tokens\":300}}}\n\n\r\n";

#[test]
fn a_streamed_answer_that_breaks_off_after_its_usage_is_not_booked_as_a_call_that_cost_nothing() {
    let s = served("relay-broken");
    let review = gateway(BROKEN_STREAM);
    let table = RouteTable::default().with("review", route("claude", &review.base_url, "claude-alias", Some((3_000_000, 15_000_000))), "run");
    let ticket = s.relay.open("run-broken", table).unwrap();
    let book = Arc::new(Book { max_calls: 10, calls: AtomicU64::new(0), lines: Mutex::new(Vec::new()) });
    ticket.take_calls(book.clone());
    let path = format!("{}/v1/messages", ticket.base_url().strip_prefix(&format!("http://{}", s.addr)).unwrap());
    let body = json!({"model": "review", "stream": true, "max_tokens": 5, "messages": []}).to_string();
    // the caller reads what it gets; a stream that breaks off is no complete HTTP answer, so it is read raw
    let mut caller = std::net::TcpStream::connect(s.addr).unwrap();
    caller.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    write!(caller, "POST {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Type: application/json\r\nAuthorization: Bearer {}\r\nContent-Length: {}\r\n\r\n{body}", s.addr, ticket.token(), body.len()).unwrap();
    let mut answer = Vec::new();
    let _ = caller.read_to_end(&mut answer);
    assert!(String::from_utf8_lossy(&answer).starts_with("HTTP/1.1 200"), "{}", String::from_utf8_lossy(&answer));
    let lines = wait_for_lines(&book, 1);
    let (line, micro) = &lines[0];
    // the gateway ran the call (1.2 M input tokens reported) and the connection broke mid-stream: the call cost
    // money, so it is booked at its tokens times the prices, or as a call of unknown cost; never at 0 as known
    assert!(!(*micro == 0 && line["cost_known"] == json!(true)), "a call that ran on the gateway was booked as known to cost nothing: {line}");
}

#[test]
fn a_streamed_chat_call_with_stream_options_null_still_asks_for_its_usage() {
    let s = served("relay-null-opts");
    let review = gateway(STREAM_ANSWER);
    let table = RouteTable::default().with("default", route("claude", &review.base_url, "claude-alias", None), "default");
    let ticket = s.relay.open("run-null-opts", table).unwrap();
    let book = Arc::new(Book { max_calls: 10, calls: AtomicU64::new(0), lines: Mutex::new(Vec::new()) });
    ticket.take_calls(book.clone());
    let r = call(&s, ticket.base_url(), "chat/completions", ticket.token(), &json!({"model": "default", "stream": true, "stream_options": null, "messages": []}));
    assert_eq!(r.status, 200);
    let seen = got(&review);
    assert_eq!(seen[0].body["stream_options"], json!({"include_usage": true}), "a streamed chat call went out without include_usage: {:?}", seen[0].body);
}

fn wait_for_lines(book: &Book, n: usize) -> Vec<(Value, u64)> {
    let limit = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let lines = book.lines.lock().unwrap().clone();
        if lines.len() >= n {
            std::thread::sleep(Duration::from_millis(50));
            return book.lines.lock().unwrap().clone();
        }
        assert!(std::time::Instant::now() < limit, "gave up waiting for {n} booked lines: {lines:?}");
        std::thread::sleep(Duration::from_millis(10));
    }
}
