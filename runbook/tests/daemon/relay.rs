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
    serving(move |_| answer.to_vec(), &[], None)
}

/// A gateway like [`gateway`] that is also ready and lists `aliases`: `GET /ready` answers 200, `GET /v1/models` the
/// list and any other `GET` 404 (no status of a model: it is read no further), none of them kept. A daemon asks a
/// declared route's gateway these before a run starts (`docs/model-routes.md`, section 3); every other request is
/// kept and answered with `answer`.
pub(super) fn gateway_listing(answer: &'static [u8], aliases: &[&str]) -> Gateway {
    serving(move |_| answer.to_vec(), aliases, None)
}

/// A gateway on a loopback port that keeps every request and answers each with what `answer` makes of it, and is ready
/// and lists `aliases` as [`gateway_listing`] is (none: every request is kept and answered by `answer`).
pub(super) fn gateway_answering(answer: impl Fn(&Got) -> Vec<u8> + Send + 'static, aliases: &[&str]) -> Gateway {
    serving(answer, aliases, None)
}

/// A gateway like [`gateway_answering`] whose `GET /v1/status` answers `status` (the status of every model, by alias).
pub(super) fn gateway_with_status(answer: impl Fn(&Got) -> Vec<u8> + Send + 'static, aliases: &[&str], status: Value) -> Gateway {
    serving(answer, aliases, Some(status.to_string()))
}

fn serving(answer: impl Fn(&Got) -> Vec<u8> + Send + 'static, aliases: &[&str], status: Option<String>) -> Gateway {
    let reads = !aliases.is_empty();
    let models = json!({"object": "list", "data": aliases.iter().map(|a| json!({"id": a, "wires": ["messages", "chat"]})).collect::<Vec<_>>()}).to_string();
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
            let line = head.lines().next().unwrap_or("").to_owned();
            if reads && line.starts_with("GET ") {
                let (code, body) = match line.split_whitespace().nth(1).unwrap_or("") {
                    "/ready" => (200, "{\"status\":\"ready\"}".to_owned()),
                    "/v1/models" => (200, models.clone()),
                    "/v1/status" if status.is_some() => (200, status.clone().unwrap_or_default()),
                    _ => (404, "{\"error\":{\"code\":\"not-found\"}}".to_owned()),
                };
                let _ = stream.write_all(format!("HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
                continue;
            }
            let length = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
            let mut body = vec![0u8; length];
            let _ = stream.read_exact(&mut body);
            let got = Got { head, body: serde_json::from_slice(&body).unwrap_or(Value::Null) };
            let bytes = answer(&got);
            kept.lock().unwrap().push(got);
            let _ = stream.write_all(&bytes);
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

/// A call names its role bare (`review`) or as the route file's model of the role (`runbook-review`); a client that adds
/// a bracketed suffix of its own (`runbook-review[1m]`, as Claude Code does to a name it takes for an alias of its own) is
/// taken for the same role. A name of no role is refused naming the role.
#[test]
fn a_call_names_its_role_bare_or_as_runbook_role_and_a_bracketed_suffix_is_dropped() {
    let s = served("relay-role-names");
    let review = gateway(OK_ANSWER);
    let table = RouteTable::default().with("review", route("claude", &review.base_url, "claude-alias", None), "run");
    let ticket = s.relay.open("run-role-names", table).unwrap();
    let book = Arc::new(Book { max_calls: 10, calls: AtomicU64::new(0), lines: Mutex::new(Vec::new()) });
    ticket.take_calls(book.clone());
    let names = ["review", "runbook-review", "runbook-review[1m]", "review[1m]"];
    for name in names {
        let r = call(&s, ticket.base_url(), "messages", ticket.token(), &json!({"model": name, "max_tokens": 5, "messages": []}));
        assert_eq!(r.status, 200, "{name}: {}", r.text());
    }
    let r = call(&s, ticket.base_url(), "messages", ticket.token(), &json!({"model": "runbook-writer[1m]", "messages": []}));
    assert_eq!((r.status, r.json()["error"]["code"].clone()), (400, json!("route-refused")), "{}", r.text());
    assert!(r.json()["error"]["message"].as_str().unwrap_or_default().contains("the role \"writer\""), "the refusal does not name the role: {}", r.text());
    let lines = wait_for_lines(&book, names.len());
    assert!(lines.iter().all(|(l, _)| l["role"] == "review"), "a line names another role: {lines:?}");
    assert!(got(&review).iter().all(|g| g.body["model"] == "claude-alias") && got(&review).len() == names.len(), "{:?}", got(&review));
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

const CACHED_ANSWER: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 124\r\nConnection: close\r\n\r\n{\"id\":\"msg\",\"usage\":{\"input_tokens\":2,\"cache_read_input_tokens\":30000,\"cache_creation_input_tokens\":1000,\"output_tokens\":7}}";
const NO_USAGE_ANSWER: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 25\r\nConnection: close\r\n\r\n{\"id\":\"msg\",\"content\":[]}";

/// The ledger line of a call counts the input read from and written to the prompt cache in fields of its own beside
/// `input_tokens`, and a declared input price takes them; a call answered with no usage on a route without prices books
/// its reservation with `cost_known: false`, on a route with prices 0 at those prices.
#[test]
fn a_calls_line_counts_the_cached_input_and_a_call_with_no_usage_books_its_reservation_on_an_unpriced_route() {
    let s = served("relay-cached");
    let (cached, bare, priced) = (gateway(CACHED_ANSWER), gateway(NO_USAGE_ANSWER), gateway(NO_USAGE_ANSWER));
    let local = ResolvedRoute { kind: "local".to_owned(), ..route("local", &bare.base_url, "local-alias", None) };
    let table = RouteTable::default()
        .with("default", route("claude", &cached.base_url, "claude-alias", Some((1_000_000, 2_000_000))), "run")
        .with("bare", local, "run")
        .with("priced", route("priced", &priced.base_url, "priced-alias", Some((3_000_000, 15_000_000))), "run");
    let ticket = s.relay.open("run-cached", table).unwrap();
    let book = Arc::new(Book { max_calls: 10, calls: AtomicU64::new(0), lines: Mutex::new(Vec::new()) });
    ticket.take_calls(book.clone());
    for (n, role) in ["default", "bare", "priced"].into_iter().enumerate() {
        let r = call(&s, ticket.base_url(), "messages", ticket.token(), &json!({"model": role, "max_tokens": 5, "messages": []}));
        assert_eq!(r.status, 200, "{role}: {}", r.text());
        wait_for_lines(&book, n + 1);
    }
    let lines = book.lines.lock().unwrap().clone();
    let of = |i: usize| -> (Value, Value, Value, Value, Value, u64) {
        let (l, m) = &lines[i];
        (l["input_tokens"].clone(), l["cache_read_input_tokens"].clone(), l["cache_creation_input_tokens"].clone(), l["output_tokens"].clone(), l["cost_known"].clone(), *m)
    };
    assert_eq!(of(0), (json!(2), json!(30_000), json!(1_000), json!(7), json!(true), 31_002 + 14), "{:?}", lines[0]);
    assert_eq!(of(1), (Value::Null, Value::Null, Value::Null, Value::Null, json!(false), 7_000), "no usage on an unpriced local route: {:?}", lines[1]);
    assert_eq!(of(2), (Value::Null, Value::Null, Value::Null, Value::Null, json!(true), 0), "no usage on a priced route: {:?}", lines[2]);
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
    let limit = std::time::Instant::now() + Duration::from_secs(3);
    while got(&review).is_empty() {
        assert!(std::time::Instant::now() < limit, "the call never reached the gateway");
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(caller);
    std::thread::sleep(Duration::from_millis(300));
    // the gateway ran the call and answers it, with its usage: the call was sent, so it is one line
    release.send(()).unwrap();
    let lines = wait_for_lines_within(&book, 1, Duration::from_secs(3));
    assert_eq!(lines.len(), 1, "{lines:?}");
}

/// A gateway on a loopback port that reads one whole call, sends `head` at once (nothing when it is empty) and then
/// holds the call open without another byte, as a model that is still generating does. The returned slot holds the
/// instant its read of the relay's connection ended (the relay closed the call), if it ended within 10 s.
fn holding_gateway(head: &'static [u8]) -> (Gateway, Arc<Mutex<Option<std::time::Instant>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let got = Arc::new(Mutex::new(Vec::new()));
    let kept = got.clone();
    let closed = Arc::new(Mutex::new(None));
    let noted = closed.clone();
    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else { return };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
        let mut raw = Vec::new();
        let mut byte = [0u8; 1];
        while !raw.ends_with(b"\r\n\r\n") && raw.len() < 64 * 1024 {
            match stream.read(&mut byte) {
                Ok(1) => raw.push(byte[0]),
                _ => break,
            }
        }
        let head_text = String::from_utf8_lossy(&raw).into_owned();
        let length = head_text.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
        let mut body = vec![0u8; length];
        let _ = stream.read_exact(&mut body);
        let _ = stream.write_all(head);
        kept.lock().unwrap().push(Got { head: head_text, body: serde_json::from_slice(&body).unwrap_or(Value::Null) });
        let mut rest = [0u8; 256];
        loop {
            match stream.read(&mut rest) {
                Ok(0) => {
                    *noted.lock().unwrap() = Some(std::time::Instant::now());
                    return;
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => {
                    *noted.lock().unwrap() = Some(std::time::Instant::now());
                    return;
                }
                Err(_) => return,
            }
        }
    });
    (Gateway { base_url, got }, closed)
}

/// The head of a streamed answer and its first event, which reports the call's input tokens; the stream goes on.
const STARTED_STREAM: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nx-llm-gateway-call: call-held\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n76\r\nevent: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":1200000,\"output_tokens\":300}}}\n\n\r\n";

/// A caller that hangs up on its call (its timeout killed it) before the gateway's answer began, and one that hangs up
/// while the answer streams: the relay closes its request to the gateway within 1 s (the stand-in gateway notes the
/// close), and books the call once as abandoned (`call-abandoned`, `caller-hung-up`) with what it knows: the tokens the
/// stream reported, and, with no price on the route, the call's reservation as a cost not known.
#[test]
fn a_call_whose_caller_hangs_up_is_closed_at_the_gateway_within_a_second_and_booked_as_abandoned() {
    for (tag, head, tokens) in [("relay-gone-before", &b""[..], Value::Null), ("relay-gone-streaming", STARTED_STREAM, json!(1_200_000))] {
        let s = served(tag);
        let (review, closed) = holding_gateway(head);
        let table = RouteTable::default().with("review", route("claude", &review.base_url, "claude-alias", None), "run");
        let ticket = s.relay.open(&format!("run-{tag}"), table).unwrap();
        let book = Arc::new(Book { max_calls: 10, calls: AtomicU64::new(0), lines: Mutex::new(Vec::new()) });
        ticket.take_calls(book.clone());
        let body = json!({"model": "review", "stream": true, "max_tokens": 5, "messages": [{"role": "user", "content": "hi"}]}).to_string();
        let path = format!("{}/v1/messages", ticket.base_url().strip_prefix(&format!("http://{}", s.addr)).unwrap());
        let mut caller = std::net::TcpStream::connect(s.addr).unwrap();
        caller.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        write!(caller, "POST {path} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nAuthorization: Bearer {}\r\nContent-Length: {}\r\n\r\n{body}", s.addr, ticket.token(), body.len()).unwrap();
        let limit = std::time::Instant::now() + Duration::from_secs(3);
        while got(&review).is_empty() {
            assert!(std::time::Instant::now() < limit, "{tag}: the call never reached the gateway");
            std::thread::sleep(Duration::from_millis(10));
        }
        if !head.is_empty() {
            // the caller reads the head of the answer and its first event: the answer streams when it hangs up
            let mut seen = Vec::new();
            let mut buf = [0u8; 1024];
            while !String::from_utf8_lossy(&seen).contains("message_start") {
                let n = caller.read(&mut buf).unwrap_or(0);
                assert!(n > 0, "{tag}: the answer's head never reached the caller: {}", String::from_utf8_lossy(&seen));
                seen.extend_from_slice(&buf[..n]);
            }
        }
        let hung_up = std::time::Instant::now();
        drop(caller);
        let limit = hung_up + Duration::from_secs(3);
        let at = loop {
            if let Some(at) = *closed.lock().unwrap() {
                break at;
            }
            assert!(std::time::Instant::now() < limit, "{tag}: the relay kept the call open at the gateway after its caller hung up");
            std::thread::sleep(Duration::from_millis(10));
        };
        assert!(at.duration_since(hung_up) < Duration::from_secs(1), "{tag}: the relay closed the call at the gateway {:?} after its caller hung up", at.duration_since(hung_up));
        let lines = wait_for_lines_within(&book, 1, Duration::from_secs(3));
        assert_eq!(lines.len(), 1, "{tag}: {lines:?}");
        let (line, micro) = &lines[0];
        assert_eq!((line["cause"].clone(), line["code"].clone()), (json!("call-abandoned"), json!("caller-hung-up")), "{tag}: {line}");
        assert_eq!((line["input_tokens"].clone(), line["cost_known"].clone(), *micro), (tokens, json!(false), 7_000), "{tag}: the abandoned call is not booked with what is known: {line}");
    }
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

#[test]
fn a_relay_path_on_a_listener_that_is_not_loopback_needs_the_daemons_token() {
    let fx = super::commands::Fx::new("relay-wide");
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let listener = rt.block_on(tokio::net::TcpListener::bind("0.0.0.0:0")).unwrap();
    let addr: SocketAddr = format!("127.0.0.1:{}", listener.local_addr().unwrap().port()).parse().unwrap();
    // a relay that took a run elsewhere, mounted on a listener of every interface by mistake
    let relay = Relay::new();
    relay.set_listen("127.0.0.1:9".parse().unwrap());
    let review = gateway(OK_ANSWER);
    let ticket = relay.open("run-wide", RouteTable::default().with("review", route("claude", &review.base_url, "claude-alias", None), "default")).unwrap();
    ticket.take_calls(Arc::new(Book { max_calls: 10, calls: AtomicU64::new(0), lines: Mutex::new(Vec::new()) }));
    let (daemon, extra) = (fx.d().clone(), relay.router());
    rt.spawn(async move {
        let _ = runbook::api::serve_extra(listener, daemon, Some(DAEMON_TOKEN), runbook::api::Streams::default(), Some(extra), std::future::pending()).await;
    });
    let r = send(addr, "POST", "/daemon/relay/run-wide/v1/messages", &[("Content-Type", "application/json"), ("Authorization", &format!("Bearer {}", ticket.token()))], Some(json!({"model": "review", "messages": []}).to_string().as_bytes()));
    assert_eq!(r.status, 401, "{}", r.text());
    assert!(got(&review).is_empty(), "a call on a listener that is not loopback reached the gateway");
}

fn wait_for_lines(book: &Book, n: usize) -> Vec<(Value, u64)> {
    wait_for_lines_within(book, n, Duration::from_secs(10))
}

fn wait_for_lines_within(book: &Book, n: usize, within: Duration) -> Vec<(Value, u64)> {
    let limit = std::time::Instant::now() + within;
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
