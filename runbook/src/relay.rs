//! The relay of a run's model calls (`docs/model-routes.md`, sections 4, 5 and 11): every model call of a run on a
//! route goes to runbookd, never to a gateway, and runbookd sends it on.
//!
//! runbookd serves, on its own listener, `/daemon/relay/<run>/v1/messages`, `/v1/chat/completions` and
//! `/v1/responses`. A call carries the run's own token (minted when the run starts, void when it ends) as its bearer
//! or `x-api-key`, and names a role in its `model`. The relay looks the role up in the run's [`RouteTable`] and
//! refuses a role the run has no route for (`400 route-refused`), takes the call against the run's caps
//! ([`Booking::admit`]: `429 cap-reached`, nothing sent), sets `model` to the route's alias (and
//! `stream_options.include_usage` on a streamed chat call), adds the gateway's credential, sends the call (one
//! request per connection) and streams the answer back unchanged. It reads the usage from the answer (the JSON
//! `usage` object, or the usage of the events of a stream) and books one line per call ([`Booking::book`]): run,
//! step, role, route, alias, the gateway's call id, input and output tokens and the cost (the route's declared price
//! per million tokens, else `cost_known: false` and the reservation).
//!
//! A gateway that refuses the call, or does not answer, is passed back unchanged (a connection that fails answers
//! `502` with the code `route-unavailable`), and the call is booked with the cause `route-unavailable`, the route,
//! the gateway's code and the setting that chose the route. The relay never sends a call to another route.
//!
//! The run's processes get the relay through the route file ([`RouteFile`]): `route.json`, `runbook.route/1`, mode
//! 0600 in the run's private directory, named by [`ROUTE_FILE_VARIABLE`]. It holds the relay's address, the run's
//! token and the roles; the gateway's credential never leaves runbookd.

use axum::body::{Body, Bytes};
use axum::extract::{Path as UrlPath, Request, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::Response;
use axum::routing::post;
use axum::Router;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

/// The variable that names the route file for every model process of a run.
pub const ROUTE_FILE_VARIABLE: &str = "RUNBOOK_ROUTE_FILE";
/// The format of the route file.
pub const ROUTE_FORMAT: &str = "runbook.route/1";
/// The name of the route file in the run's private directory.
pub const ROUTE_FILE: &str = "route.json";
/// The path every relay route starts with.
pub const PATH_PREFIX: &str = "/daemon/relay/";
/// The wires the relay serves, as the path after `/v1/`.
pub const WIRES: [&str; 3] = ["messages", "chat/completions", "responses"];
/// The refusal of a role the run has no route for.
pub const ROUTE_REFUSED: &str = "route-refused";
/// The refusal of a call past the run's caps.
pub const CAP_REACHED: &str = "cap-reached";
/// The cause of a call the gateway refused or did not answer.
pub const ROUTE_UNAVAILABLE: &str = "route-unavailable";
/// The code of a call whose gateway did not answer: nothing ran.
const CONNECTION_FAILED: &str = "connection-failed";
/// The code of a call whose answer broke off after its head: the call ran and is booked at its cost.
const ANSWER_BROKEN: &str = "answer-broken";

/// The most bytes of a call's body the relay reads.
const MAX_CALL_BYTES: usize = 64 * 1024 * 1024;
/// The most bytes of an answer the relay keeps to read its usage from; the answer itself is passed on whole.
const KEEP_ANSWER_BYTES: usize = 32 * 1024 * 1024;
/// The most bytes of the head of the gateway's answer.
const MAX_HEAD_BYTES: usize = 64 * 1024;
const CONNECT_WAIT: Duration = Duration::from_secs(10);
/// How long the relay waits for the next bytes of the gateway's answer.
const READ_WAIT: Duration = Duration::from_secs(900);

/// A route as a run uses it: the gateway's address and credential, the alias the gateway serves, the kind and the
/// declared prices. Its `Debug` holds no token.
#[derive(Clone, PartialEq, Eq)]
pub struct ResolvedRoute {
    /// The route's name in the daemon's declaration.
    pub name: String,
    /// The gateway's address: `http://host[:port][/path]`.
    pub base_url: String,
    /// The gateway's credential; none for a gateway without ingress auth.
    pub token: Option<String>,
    /// The model the gateway serves, as its `/v1/models` lists it.
    pub alias: String,
    /// `managed`, `hosted` or `local`.
    pub kind: String,
    /// What one million input tokens cost, micro USD, when the route declares it.
    pub price_in_micro_usd_per_mtok: Option<u64>,
    /// What one million output tokens cost, micro USD, when the route declares it.
    pub price_out_micro_usd_per_mtok: Option<u64>,
}

impl std::fmt::Debug for ResolvedRoute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedRoute")
            .field("name", &self.name)
            .field("base_url", &self.base_url)
            .field("token", &if self.token.is_some() { "<named>" } else { "<none>" })
            .field("alias", &self.alias)
            .field("kind", &self.kind)
            .field("price_in_micro_usd_per_mtok", &self.price_in_micro_usd_per_mtok)
            .field("price_out_micro_usd_per_mtok", &self.price_out_micro_usd_per_mtok)
            .finish()
    }
}

impl From<crate::routes::Resolved<'_>> for ResolvedRoute {
    /// A declared route as a run uses it: its gateway's address and token, its alias, its kind and its prices.
    fn from(r: crate::routes::Resolved<'_>) -> ResolvedRoute {
        ResolvedRoute {
            name: r.route.to_owned(),
            base_url: r.base_url.to_owned(),
            token: r.token.map(str::to_owned),
            alias: r.alias.to_owned(),
            kind: r.kind.as_str().to_owned(),
            price_in_micro_usd_per_mtok: r.prices.in_micro_usd_per_mtok,
            price_out_micro_usd_per_mtok: r.prices.out_micro_usd_per_mtok,
        }
    }
}

/// The route of each role of a run, and the setting that chose it (`run`, `protocol`, `default` or the name of the
/// daemon's setting).
#[derive(Clone, Debug, Default)]
pub struct RouteTable {
    roles: BTreeMap<String, (ResolvedRoute, String)>,
}

impl RouteTable {
    /// The table with `role` on `route`, chosen by `source`.
    pub fn with(mut self, role: &str, route: ResolvedRoute, source: &str) -> RouteTable {
        self.roles.insert(role.to_owned(), (route, source.to_owned()));
        self
    }

    /// The route of `role` and the setting that chose it.
    pub fn get(&self, role: &str) -> Option<(&ResolvedRoute, &str)> {
        self.roles.get(role).map(|(r, s)| (r, s.as_str()))
    }

    /// The roles, in order.
    pub fn roles(&self) -> Vec<&str> {
        self.roles.keys().map(String::as_str).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.roles.is_empty()
    }
}

/// What the relay asks of the run a call belongs to: the run's caps and its spend ledger.
pub trait Booking: Send + Sync {
    /// Takes one call against the run's caps; `Err` names the cap that is reached, and nothing is sent.
    fn admit(&self) -> Result<(), String>;
    /// What a call of unknown cost books, micro USD.
    fn reservation_micro_usd(&self) -> u64;
    /// The step of the run in flight, when one is.
    fn step(&self) -> Option<String>;
    /// Books one call: `line` is the ledger line, `micro_usd` what it costs.
    fn book(&self, line: Value, micro_usd: u64) -> Result<(), String>;
}

struct Entry {
    digest: [u8; 32],
    table: RouteTable,
    booking: OnceLock<Arc<dyn Booking>>,
}

/// The relay of one daemon: the runs on a route, each with its token and table.
#[derive(Default)]
pub struct Relay {
    /// The relay's address once the daemon listens; why it serves no run when the listener is not loopback.
    base: Mutex<Option<Result<String, String>>>,
    runs: Mutex<HashMap<String, Arc<Entry>>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn digest(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

fn same(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter().zip(b).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

/// 32 bytes from the kernel's random source, as hex.
fn mint_token() -> Result<String, String> {
    let mut buf = [0u8; 32];
    let mut filled = 0;
    while filled < buf.len() {
        // SAFETY: the pointer and length name the unfilled part of `buf`.
        let n = unsafe { libc::getrandom(buf[filled..].as_mut_ptr().cast(), buf.len() - filled, 0) };
        if n < 0 {
            let e = std::io::Error::last_os_error();
            if e.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(format!("the run's relay token could not be minted: {e}"));
        }
        filled += n as usize;
    }
    Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
}

/// A run's place in the relay: its token and address. The token is void when this is dropped.
pub struct Ticket {
    relay: Arc<Relay>,
    run: String,
    entry: Arc<Entry>,
    token: String,
    base_url: String,
}

impl std::fmt::Debug for Ticket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ticket").field("run", &self.run).field("base_url", &self.base_url).finish()
    }
}

impl Ticket {
    /// The run's token, for the route file.
    pub fn token(&self) -> &str {
        &self.token
    }

    /// The relay's address for this run: `http://<listener>/daemon/relay/<run>`.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// From now on the run's calls are taken against `booking`; before, a call is answered 503.
    pub fn take_calls(&self, booking: Arc<dyn Booking>) {
        let _ = self.entry.booking.set(booking);
    }
}

impl Drop for Ticket {
    fn drop(&mut self) {
        let mut runs = lock(&self.relay.runs);
        if runs.get(&self.run).is_some_and(|e| Arc::ptr_eq(e, &self.entry)) {
            runs.remove(&self.run);
        }
    }
}

impl Relay {
    pub fn new() -> Arc<Relay> {
        Arc::new(Relay::default())
    }

    /// The address the daemon listens on: the relay's address in every route file. The relay serves a loopback
    /// listener only (`docs/model-routes.md`, section 11): on any other, every interface included, it serves no run,
    /// and [`Relay::serves`] says so.
    pub fn set_listen(&self, addr: SocketAddr) {
        let base = match addr.ip() {
            ip if !ip.is_loopback() => Err(format!("the relay of model calls serves a loopback listener only, and the daemon listens on {addr}")),
            std::net::IpAddr::V4(ip) => Ok(format!("http://{ip}:{}", addr.port())),
            std::net::IpAddr::V6(ip) => Ok(format!("http://[{ip}]:{}", addr.port())),
        };
        *lock(&self.base) = Some(base);
    }

    /// Whether the relay serves runs: the daemon listens, on loopback.
    pub fn serves(&self) -> bool {
        matches!(*lock(&self.base), Some(Ok(_)))
    }

    /// Takes the run `run` with its table: a token is minted for it. Refused when the relay has no address yet or
    /// serves no run, when the table is empty, or when the run is on the relay already.
    pub fn open(self: &Arc<Self>, run: &str, table: RouteTable) -> Result<Ticket, String> {
        let base = lock(&self.base).clone().ok_or("the relay of the daemon has no address yet")??;
        if table.is_empty() {
            return Err("the run's route table names no role".to_owned());
        }
        if run.is_empty() || !run.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err("a run id the relay takes is letters, digits, '-' and '_'".to_owned());
        }
        let token = mint_token()?;
        let entry = Arc::new(Entry { digest: digest(&token), table, booking: OnceLock::new() });
        {
            let mut runs = lock(&self.runs);
            if runs.contains_key(run) {
                return Err(format!("run {run} is on the relay already"));
            }
            runs.insert(run.to_owned(), entry.clone());
        }
        Ok(Ticket { relay: self.clone(), run: run.to_owned(), entry, token, base_url: format!("{base}{PATH_PREFIX}{run}") })
    }

    /// The routes of the relay: one per wire, every one behind the run's token.
    pub fn router(self: &Arc<Self>) -> Router {
        let mut router = Router::new();
        for wire in WIRES {
            router = router.route(
                &format!("{PATH_PREFIX}{{run}}/v1/{wire}"),
                post(move |State(relay): State<Arc<Relay>>, UrlPath(run): UrlPath<String>, request: Request| async move { relay.call(&run, wire, request).await }),
            );
        }
        router.with_state(self.clone())
    }

    fn entry(&self, run: &str) -> Option<Arc<Entry>> {
        lock(&self.runs).get(run).cloned()
    }

    async fn call(&self, run: &str, wire: &'static str, request: Request) -> Response {
        let Some(entry) = self.entry(run) else { return unauthorized() };
        if !presented(request.headers()).is_some_and(|t| same(&digest(t), &entry.digest)) {
            return unauthorized();
        }
        let Some(booking) = entry.booking.get().cloned() else {
            return error(StatusCode::SERVICE_UNAVAILABLE, "run-not-ready", "the run takes no model call yet");
        };
        let passed = passed_headers(request.headers());
        let Ok(bytes) = axum::body::to_bytes(request.into_body(), MAX_CALL_BYTES).await else {
            return error(StatusCode::PAYLOAD_TOO_LARGE, "too-large", &format!("a call is at most {MAX_CALL_BYTES} bytes"));
        };
        let Ok(Value::Object(mut body)) = serde_json::from_slice::<Value>(&bytes) else {
            return error(StatusCode::BAD_REQUEST, "bad-request", "a call is a JSON object");
        };
        let role = body.get("model").and_then(Value::as_str).unwrap_or("").to_owned();
        let Some((route, source)) = entry.table.get(&role) else {
            let roles = entry.table.roles().join(", ");
            return error(StatusCode::BAD_REQUEST, ROUTE_REFUSED, &format!("the run has no route for the role {role:?}; its roles: {roles}"));
        };
        let (route, source) = (route.clone(), source.to_owned());
        body.insert("model".to_owned(), Value::String(route.alias.clone()));
        if wire == "chat/completions" && body.get("stream") == Some(&Value::Bool(true)) {
            match body.get_mut("stream_options") {
                Some(Value::Object(o)) => {
                    o.insert("include_usage".to_owned(), Value::Bool(true));
                }
                _ => {
                    body.insert("stream_options".to_owned(), json!({"include_usage": true}));
                }
            }
        }
        let sent = serde_json::to_vec(&Value::Object(body)).unwrap_or_default();
        let call = Call { run: run.to_owned(), step: booking.step(), role, route, source, wire, booking };
        // The caps, the exchange with the gateway and the booking run in a task of their own: a caller that hangs up
        // drops this handler and not the call, so a call that was sent is booked, once.
        let (answered, answer) = tokio::sync::oneshot::channel::<Response>();
        tokio::spawn(async move {
            if answered.is_closed() {
                return;
            }
            let _ = answered.send(exchange(call, passed, sent).await);
        });
        answer.await.unwrap_or_else(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "relay-failed", "the relay lost the call"))
    }
}

/// Takes `call` against the run's caps, sends it and answers it; every call that is sent is booked once.
async fn exchange(call: Call, passed: Vec<(String, String)>, sent: Vec<u8>) -> Response {
    let admitting = call.booking.clone();
    match tokio::task::spawn_blocking(move || admitting.admit()).await {
        Ok(Ok(())) => {}
        Ok(Err(detail)) => return error(StatusCode::TOO_MANY_REQUESTS, CAP_REACHED, &format!("the run reached a cap, nothing was sent: {detail}")),
        Err(_) => return error(StatusCode::INTERNAL_SERVER_ERROR, "relay-failed", "the run's caps could not be read"),
    }
    match open_upstream(&call.route, call.wire, &passed, &sent).await {
        Err(why) => {
            let line = call.line(None, &Usage::default(), Some((CONNECTION_FAILED.to_owned(), why.clone())));
            call.book(line).await;
            error(StatusCode::BAD_GATEWAY, ROUTE_UNAVAILABLE, &format!("route {} (chosen by {}): the gateway did not answer: {why}", call.route.name, call.source))
        }
        Ok(upstream) => answer(call, upstream),
    }
}

/// Whether `path` is a path of the relay: `/daemon/relay/<run>/v1/<wire>`, the run id of letters, digits, `-` and
/// `_`, the wire one of [`WIRES`]. The daemon's guard leaves the token of such a path to the relay.
pub fn is_relay_path(path: &str) -> bool {
    let Some(rest) = path.strip_prefix(PATH_PREFIX) else { return false };
    let Some((run, wire)) = rest.split_once("/v1/") else { return false };
    !run.is_empty() && run.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') && WIRES.contains(&wire)
}

fn unauthorized() -> Response {
    let mut r = error(StatusCode::UNAUTHORIZED, "unauthorized", "the call carries no token of a live run");
    r.headers_mut().insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
    r
}

/// An answer of the relay itself, in the gateway's shape: `{"error": {"code", "message"}}`.
fn error(status: StatusCode, code: &str, message: &str) -> Response {
    let mut r = Response::new(Body::from(json!({"error": {"code": code, "message": message}}).to_string()));
    *r.status_mut() = status;
    r.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
    r
}

/// The token a call presents: `Authorization: Bearer <token>`, else `x-api-key`.
fn presented(headers: &HeaderMap) -> Option<&str> {
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .map(|(_, t)| t.trim());
    bearer.or_else(|| headers.get("x-api-key").and_then(|v| v.to_str().ok()).map(str::trim)).filter(|t| !t.is_empty())
}

/// The headers of a call the gateway is given: the wire's own, never a credential.
fn passed_headers(headers: &HeaderMap) -> Vec<(String, String)> {
    ["anthropic-version", "anthropic-beta", "accept", "openai-beta"]
        .iter()
        .flat_map(|name| headers.get_all(*name).iter().filter_map(|v| v.to_str().ok()).map(|v| ((*name).to_owned(), v.to_owned())).collect::<Vec<_>>())
        .collect()
}

/// One call on its way.
struct Call {
    run: String,
    step: Option<String>,
    role: String,
    route: ResolvedRoute,
    source: String,
    wire: &'static str,
    booking: Arc<dyn Booking>,
}

/// The tokens an answer reported; a value that was not reported is `None`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

impl Usage {
    fn take(&mut self, usage: &Value) {
        let n = |keys: &[&str]| keys.iter().find_map(|k| usage.get(*k).and_then(Value::as_u64));
        if let Some(i) = n(&["input_tokens", "prompt_tokens"]) {
            self.input_tokens = Some(self.input_tokens.map_or(i, |x| x.max(i)));
        }
        if let Some(o) = n(&["output_tokens", "completion_tokens"]) {
            self.output_tokens = Some(self.output_tokens.map_or(o, |x| x.max(o)));
        }
    }

    fn take_document(&mut self, v: &Value) {
        for usage in [v.get("usage"), v.get("message").and_then(|m| m.get("usage")), v.get("response").and_then(|r| r.get("usage"))].into_iter().flatten() {
            if usage.is_object() {
                self.take(usage);
            }
        }
    }
}

/// The usage an answer reports: the `usage` of a JSON answer, or the largest of each count over the events of a
/// stream (`data: <json>` lines; a stream's counts grow, its last is the largest).
pub fn usage_of(body: &[u8]) -> Usage {
    let mut usage = Usage::default();
    if let Ok(v) = serde_json::from_slice::<Value>(body) {
        usage.take_document(&v);
        return usage;
    }
    for line in body.split(|b| *b == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let Some(data) = line.strip_prefix(b"data:") else { continue };
        if let Ok(v) = serde_json::from_slice::<Value>(data.trim_ascii()) {
            usage.take_document(&v);
        }
    }
    usage
}

/// The cost of a call on `route` with `usage`: tokens times the declared prices (a price that is not declared
/// beside one that is counts 0), rounded up to a whole micro USD; a local route without prices costs 0. `None`: the
/// cost is not known (no price, or the tokens were not reported).
pub fn cost_of(route: &ResolvedRoute, usage: &Usage) -> Option<u64> {
    let (pin, pout) = (route.price_in_micro_usd_per_mtok, route.price_out_micro_usd_per_mtok);
    if pin.is_none() && pout.is_none() {
        return (route.kind == "local").then_some(0);
    }
    let part = |tokens: Option<u64>, price: Option<u64>| -> Option<u128> {
        match price {
            None | Some(0) => Some(0),
            Some(p) => tokens.map(|t| (u128::from(t) * u128::from(p)).div_ceil(1_000_000)),
        }
    };
    let total = part(usage.input_tokens, pin)? + part(usage.output_tokens, pout)?;
    Some(u64::try_from(total).unwrap_or(u64::MAX))
}

impl Call {
    /// The ledger line of this call. `failed`: the gateway's code and what was wrong, for a call that failed. A call the
    /// gateway refused or did not answer cost nothing; one whose answer broke off ran, and is booked as a call that
    /// ended is.
    fn line(&self, call_id: Option<String>, usage: &Usage, failed: Option<(String, String)>) -> (Value, u64) {
        let (micro_usd, cost_known) = match (&failed, cost_of(&self.route, usage)) {
            (Some((code, _)), _) if code != ANSWER_BROKEN => (0, true),
            (Some(_), Some(cost)) => (cost, true),
            (Some(_), None) => (self.booking.reservation_micro_usd(), false),
            (None, Some(cost)) => (cost, true),
            (None, None) => (self.booking.reservation_micro_usd(), false),
        };
        let mut line = json!({
            "what": "model call",
            "run": self.run,
            "step": self.step,
            "role": self.role,
            "route": self.route.name,
            "alias": self.route.alias,
            "source": self.source,
            "wire": self.wire,
            "gateway_call": call_id,
            "input_tokens": usage.input_tokens,
            "output_tokens": usage.output_tokens,
            "cost_known": cost_known,
            "micro_usd": micro_usd,
        });
        if let Some((code, detail)) = failed {
            line["cause"] = json!(ROUTE_UNAVAILABLE);
            line["code"] = json!(code);
            line["detail"] = json!(detail);
        }
        (line, micro_usd)
    }

    async fn book(&self, (line, micro_usd): (Value, u64)) {
        let booking = self.booking.clone();
        let _ = tokio::task::spawn_blocking(move || booking.book(line, micro_usd)).await;
    }
}

/// The address of `base_url` and the path of `wire` below it. Only `http://` is served.
fn target(base_url: &str, wire: &str) -> Result<(String, String, String), String> {
    let rest = base_url.strip_prefix("http://").ok_or("the relay sends to an http:// gateway only")?;
    let (authority, path) = rest.split_once('/').map_or((rest, ""), |(a, p)| (a, p));
    let path = path.trim_end_matches('/');
    let has_port = if authority.starts_with('[') { authority.contains("]:") } else { authority.contains(':') };
    let addr = if has_port { authority.to_owned() } else { format!("{authority}:80") };
    let path = if path.is_empty() { format!("/v1/{wire}") } else { format!("/{path}/v1/{wire}") };
    Ok((addr, authority.to_owned(), path))
}

/// The gateway's answer as it comes in: its status, its headers and its body.
struct Upstream {
    status: u16,
    headers: Vec<(String, String)>,
    body: BodyReader,
}

enum Framing {
    Chunked,
    Length(u64),
    Close,
}

struct BodyReader {
    reader: BufReader<TcpStream>,
    framing: Framing,
    done: bool,
}

async fn timed<T>(f: impl std::future::Future<Output = std::io::Result<T>>) -> std::io::Result<T> {
    tokio::time::timeout(READ_WAIT, f).await.map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "the gateway sent nothing for too long"))?
}

impl BodyReader {
    /// The next bytes of the body, decoded from its framing; `None` at its end.
    async fn next(&mut self) -> std::io::Result<Option<Vec<u8>>> {
        if self.done {
            return Ok(None);
        }
        match self.framing {
            Framing::Close => {
                let mut buf = vec![0u8; 16 * 1024];
                let n = timed(self.reader.read(&mut buf)).await?;
                if n == 0 {
                    self.done = true;
                    return Ok(None);
                }
                buf.truncate(n);
                Ok(Some(buf))
            }
            Framing::Length(ref mut left) => {
                if *left == 0 {
                    self.done = true;
                    return Ok(None);
                }
                let want = (*left).min(16 * 1024) as usize;
                let mut buf = vec![0u8; want];
                let n = timed(self.reader.read(&mut buf)).await?;
                if n == 0 {
                    return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "the gateway closed the answer before its length"));
                }
                *left -= n as u64;
                buf.truncate(n);
                Ok(Some(buf))
            }
            Framing::Chunked => {
                let mut size_line = Vec::new();
                timed(self.reader.read_until(b'\n', &mut size_line)).await?;
                let text = String::from_utf8_lossy(&size_line);
                let size = u64::from_str_radix(text.trim().split(';').next().unwrap_or("").trim(), 16).map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "a chunk of the gateway's answer has no size"))?;
                if size == 0 {
                    loop {
                        let mut trailer = Vec::new();
                        let n = timed(self.reader.read_until(b'\n', &mut trailer)).await?;
                        if n == 0 || trailer == b"\r\n" || trailer == b"\n" {
                            break;
                        }
                    }
                    self.done = true;
                    return Ok(None);
                }
                let mut buf = vec![0u8; usize::try_from(size).map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "a chunk is too large"))?];
                timed(self.reader.read_exact(&mut buf)).await?;
                let mut end = [0u8; 2];
                timed(self.reader.read_exact(&mut end)).await?;
                Ok(Some(buf))
            }
        }
    }
}

/// Sends the call to the gateway of `route` (one request on its own connection) and reads the head of the answer.
async fn open_upstream(route: &ResolvedRoute, wire: &str, passed: &[(String, String)], body: &[u8]) -> Result<Upstream, String> {
    let (addr, host, path) = target(&route.base_url, wire)?;
    let stream = tokio::time::timeout(CONNECT_WAIT, TcpStream::connect(&addr)).await.map_err(|_| "no connection within 10 s".to_owned())?.map_err(|e| format!("{}", e.kind()))?;
    let mut head = format!("POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n", body.len());
    for (name, value) in passed {
        if !value.contains(['\r', '\n']) {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
    }
    if let Some(token) = &route.token {
        head.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    head.push_str("\r\n");
    let mut reader = BufReader::new(stream);
    let written = async {
        reader.get_mut().write_all(head.as_bytes()).await?;
        reader.get_mut().write_all(body).await?;
        reader.get_mut().flush().await
    };
    timed(written).await.map_err(|e| format!("the call could not be sent: {}", e.kind()))?;
    let mut status_line = Vec::new();
    timed(reader.read_until(b'\n', &mut status_line)).await.map_err(|e| format!("no answer: {}", e.kind()))?;
    let status_text = String::from_utf8_lossy(&status_line).into_owned();
    let status = status_text.split_whitespace().nth(1).and_then(|s| s.parse::<u16>().ok()).filter(|_| status_text.starts_with("HTTP/")).ok_or("the gateway's answer has no status line")?;
    let mut headers = Vec::new();
    let mut read = status_line.len();
    loop {
        let mut line = Vec::new();
        let n = timed(reader.read_until(b'\n', &mut line)).await.map_err(|e| format!("the head of the answer: {}", e.kind()))?;
        read += n;
        if n == 0 || read > MAX_HEAD_BYTES {
            return Err("the head of the gateway's answer is cut off".to_owned());
        }
        let text = String::from_utf8_lossy(&line);
        let text = text.trim_end_matches(['\r', '\n']);
        if text.is_empty() {
            break;
        }
        if let Some((name, value)) = text.split_once(':') {
            headers.push((name.trim().to_ascii_lowercase(), value.trim().to_owned()));
        }
    }
    let get = |name: &str| headers.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str());
    let framing = if get("transfer-encoding").is_some_and(|v| v.to_ascii_lowercase().contains("chunked")) {
        Framing::Chunked
    } else if let Some(n) = get("content-length").and_then(|v| v.parse::<u64>().ok()) {
        Framing::Length(n)
    } else {
        Framing::Close
    };
    Ok(Upstream { status, headers, body: BodyReader { reader, framing, done: false } })
}

/// The headers of the gateway's answer that are about its connection, not its content.
const HOP: [&str; 6] = ["connection", "keep-alive", "transfer-encoding", "content-length", "proxy-connection", "upgrade"];

/// The gateway's code in a refusal: `error.code`, else `error.type`, else `status-<n>`.
fn gateway_code(status: u16, body: &[u8]) -> String {
    let v: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    let err = v.get("error");
    err.and_then(|e| e.get("code")).and_then(Value::as_str).or_else(|| err.and_then(|e| e.get("type")).and_then(Value::as_str)).map_or_else(|| format!("status-{status}"), str::to_owned)
}

/// The answer to the caller: the gateway's status, headers and body, streamed as they come; the call is booked
/// when the gateway's answer has ended, whether the caller read it all or not.
fn answer(call: Call, upstream: Upstream) -> Response {
    let Upstream { status, headers, mut body } = upstream;
    let call_id = headers.iter().find(|(n, _)| n == "x-llm-gateway-call").map(|(_, v)| v.clone());
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(16);
    tokio::spawn(async move {
        let mut kept = Vec::new();
        let mut broken = None;
        loop {
            match body.next().await {
                Ok(Some(bytes)) => {
                    if kept.len() < KEEP_ANSWER_BYTES {
                        kept.extend_from_slice(&bytes[..bytes.len().min(KEEP_ANSWER_BYTES - kept.len())]);
                    }
                    let _ = tx.send(Ok(Bytes::from(bytes))).await;
                }
                Ok(None) => break,
                Err(e) => {
                    broken = Some(format!("the answer broke off: {}", e.kind()));
                    let _ = tx.send(Err(e)).await;
                    break;
                }
            }
        }
        let usage = usage_of(&kept);
        let failed = if status >= 400 {
            Some((gateway_code(status, &kept), format!("the gateway answered {status}")))
        } else {
            broken.map(|why| (ANSWER_BROKEN.to_owned(), why))
        };
        let line = call.line(call_id, &usage, failed);
        // Booked before the caller sees the end of the answer: a process that read its answer finds its call booked.
        call.book(line).await;
        drop(tx);
    });
    // Before an error is passed on the stream is pending once: the server writes out what it holds (the head, the
    // chunks before the break) before the error ends the caller's connection.
    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        let item = rx.recv().await?;
        if item.is_err() {
            tokio::task::yield_now().await;
        }
        Some((item, rx))
    });
    let mut response = Response::new(Body::from_stream(stream));
    *response.status_mut() = StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
    for (name, value) in &headers {
        if HOP.contains(&name.as_str()) {
            continue;
        }
        if let (Ok(n), Ok(v)) = (HeaderName::from_bytes(name.as_bytes()), HeaderValue::from_str(value)) {
            response.headers_mut().append(n, v);
        }
    }
    response
}

/// The run's private directory, `$XDG_RUNTIME_DIR/runbook/<name>/` (mode 0700), with the route file in it
/// (`route.json`, mode 0600). Both go when this is dropped.
pub struct RouteFile {
    dir: PathBuf,
    file: PathBuf,
}

fn owned_directory(path: &Path, what: &str) -> Result<(), String> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| format!("{what} {}: {}", path.display(), e.kind()))?;
    // SAFETY: geteuid has no preconditions.
    let me = unsafe { libc::geteuid() };
    if !meta.is_dir() || meta.uid() != me {
        return Err(format!("{what} {} is not a directory owned by the user running the daemon", path.display()));
    }
    Ok(())
}

impl RouteFile {
    /// Writes the route file of `ticket` and `table` into the private directory `name`. Refused, with nothing made,
    /// when `XDG_RUNTIME_DIR` is not set, not absolute or not a directory of the user, and when the directory exists.
    pub fn write(name: &str, ticket: &Ticket, table: &RouteTable) -> Result<RouteFile, String> {
        let base = match std::env::var_os("XDG_RUNTIME_DIR") {
            Some(v) if !v.is_empty() => PathBuf::from(v),
            _ => return Err("the run is on a route (the daemon's gateway) and XDG_RUNTIME_DIR is not set: its route file lives in $XDG_RUNTIME_DIR/runbook/<run>/, not in the run directory".to_owned()),
        };
        if !base.is_absolute() {
            return Err("XDG_RUNTIME_DIR is not an absolute path".to_owned());
        }
        owned_directory(&base, "XDG_RUNTIME_DIR")?;
        let parent = base.join("runbook");
        match std::fs::DirBuilder::new().mode(0o700).create(&parent) {
            Ok(()) => std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).map_err(|e| format!("{}: {}", parent.display(), e.kind()))?,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => owned_directory(&parent, "the runtime directory")?,
            Err(e) => return Err(format!("{}: {}", parent.display(), e.kind())),
        }
        let dir = parent.join(name);
        match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Err(format!("{}: exists (left by a run that was killed? remove it when no run of this name is on)", dir.display())),
            Err(e) => return Err(format!("{}: {}", dir.display(), e.kind())),
        }
        let made = RouteFile { file: dir.join(ROUTE_FILE), dir };
        std::fs::set_permissions(&made.dir, std::fs::Permissions::from_mode(0o700)).map_err(|e| format!("{}: {}", made.dir.display(), e.kind()))?;
        let routes: serde_json::Map<String, Value> = table.roles.iter().map(|(role, (route, _))| (role.clone(), json!({"route": route.name}))).collect();
        let text = serde_json::to_string_pretty(&json!({"format": ROUTE_FORMAT, "base_url": ticket.base_url, "token": ticket.token, "routes": routes})).unwrap_or_default() + "\n";
        let written = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&made.file).and_then(|mut f| std::io::Write::write_all(&mut f, text.as_bytes()));
        written.map_err(|e| format!("{}: could not write the route file: {}", made.file.display(), e.kind()))?;
        Ok(made)
    }

    /// The route file.
    pub fn path(&self) -> &Path {
        &self.file
    }

    /// The private directory the file is in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

impl Drop for RouteFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(kind: &str, pin: Option<u64>, pout: Option<u64>) -> ResolvedRoute {
        ResolvedRoute { name: "r".into(), base_url: "http://127.0.0.1:1".into(), token: Some("secret-token-1".into()), alias: "a".into(), kind: kind.into(), price_in_micro_usd_per_mtok: pin, price_out_micro_usd_per_mtok: pout }
    }

    #[test]
    fn the_usage_of_a_json_answer_and_of_a_stream_is_read_in_each_wire() {
        assert_eq!(usage_of(br#"{"usage":{"input_tokens":12,"output_tokens":7}}"#), Usage { input_tokens: Some(12), output_tokens: Some(7) });
        assert_eq!(usage_of(br#"{"usage":{"prompt_tokens":3,"completion_tokens":4}}"#), Usage { input_tokens: Some(3), output_tokens: Some(4) });
        let messages = b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":20,\"output_tokens\":1}}}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":9}}\n\n";
        assert_eq!(usage_of(messages), Usage { input_tokens: Some(20), output_tokens: Some(9) });
        let chat = b"data: {\"choices\":[]}\r\n\r\ndata: {\"choices\":[],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":6}}\r\n\r\ndata: [DONE]\r\n\r\n";
        assert_eq!(usage_of(chat), Usage { input_tokens: Some(5), output_tokens: Some(6) });
        let responses = b"data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":8,\"output_tokens\":2}}}\n\n";
        assert_eq!(usage_of(responses), Usage { input_tokens: Some(8), output_tokens: Some(2) });
        assert_eq!(usage_of(b"{}"), Usage::default());
    }

    #[test]
    fn a_cost_is_tokens_times_the_declared_prices_and_unknown_without_them() {
        let used = Usage { input_tokens: Some(1_000_000), output_tokens: Some(500_000) };
        assert_eq!(cost_of(&route("hosted", Some(3_000_000), Some(15_000_000)), &used), Some(3_000_000 + 7_500_000));
        assert_eq!(cost_of(&route("hosted", Some(1), None), &Usage { input_tokens: Some(1), output_tokens: None }), Some(1), "a part of a micro USD is booked as one");
        assert_eq!(cost_of(&route("hosted", None, None), &used), None);
        assert_eq!(cost_of(&route("local", None, None), &used), Some(0), "a local route without prices costs 0");
        assert_eq!(cost_of(&route("hosted", Some(5), Some(5)), &Usage::default()), None, "no tokens reported: unknown");
    }

    #[test]
    fn the_debug_of_a_route_holds_no_token() {
        let shown = format!("{:?}", route("hosted", None, None));
        assert!(!shown.contains("secret-token-1") && shown.contains("<named>"), "{shown}");
    }

    #[test]
    fn a_relay_path_is_a_run_id_and_one_of_the_three_wires_and_nothing_else() {
        for ok in ["/daemon/relay/r-1_a/v1/messages", "/daemon/relay/R9/v1/chat/completions", "/daemon/relay/x/v1/responses"] {
            assert!(is_relay_path(ok), "{ok}");
        }
        for not in [
            "/daemon/relay//v1/messages",
            "/daemon/relay/../commands/submit-run/v1/messages",
            "/daemon/relay/a.b/v1/messages",
            "/daemon/relay/a%2e/v1/messages",
            "/daemon/relay/a/v1/models",
            "/daemon/relay/a/v1/messages/",
            "/daemon/relay/a/b/v1/messages",
            "/daemon/commands/submit-run",
            "/daemon/relay/a",
        ] {
            assert!(!is_relay_path(not), "{not}");
        }
    }

    #[test]
    fn a_gateway_address_is_split_into_its_host_and_the_wire_path() {
        assert_eq!(target("http://127.0.0.1:8080", "messages").unwrap(), ("127.0.0.1:8080".to_owned(), "127.0.0.1:8080".to_owned(), "/v1/messages".to_owned()));
        assert_eq!(target("http://gw.test/base/", "chat/completions").unwrap(), ("gw.test:80".to_owned(), "gw.test".to_owned(), "/base/v1/chat/completions".to_owned()));
        assert_eq!(target("http://[::1]:9/", "responses").unwrap().0, "[::1]:9");
        assert!(target("https://gw.test", "messages").is_err());
    }

    #[test]
    fn a_relay_whose_daemon_listens_on_an_address_that_is_not_loopback_serves_no_run() {
        for addr in ["0.0.0.0:7000", "[::]:7000", "192.0.2.1:7000"] {
            let relay = Relay::new();
            relay.set_listen(addr.parse().unwrap());
            assert!(!relay.serves(), "{addr}");
            let why = relay.open("run-a", RouteTable::default().with("default", route_named("default"), "default")).err().unwrap_or_default();
            assert!(why.contains("loopback"), "{addr}: {why}");
        }
        let relay = Relay::new();
        relay.set_listen("127.0.0.1:7000".parse().unwrap());
        assert!(relay.serves());
    }

    fn route_named(name: &str) -> ResolvedRoute {
        ResolvedRoute { name: name.to_owned(), base_url: "http://127.0.0.1:9".to_owned(), token: None, alias: "a".to_owned(), kind: "hosted".to_owned(), price_in_micro_usd_per_mtok: None, price_out_micro_usd_per_mtok: None }
    }
}
