use super::commands::Fx;
use super::openapi;
use runbook::api::{self, routes};
use runbook::commands::{MAX_INTAKE_BYTES, OUTCOMES};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

/// The daemon of a fixture, served in this process on port 0.
pub struct Server {
    pub fx: Fx,
    pub addr: SocketAddr,
    rt: Option<tokio::runtime::Runtime>,
}

impl Server {
    pub fn start(tag: &str, token: Option<&str>) -> Server {
        let fx = Fx::new(tag);
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        let listener = rt.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).unwrap();
        let addr = listener.local_addr().unwrap();
        let (daemon, token) = (fx.d().clone(), token.map(String::from));
        rt.spawn(async move {
            let _ = api::serve(listener, daemon, token.as_deref(), std::future::pending()).await;
        });
        Server { fx, addr, rt: Some(rt) }
    }

    pub fn get(&self, path: &str) -> Resp {
        self.send("GET", path, &[], None)
    }

    pub fn get_with(&self, path: &str, headers: &[(&str, &str)]) -> Resp {
        self.send("GET", path, headers, None)
    }

    pub fn post(&self, command: &str, body: &Value) -> Resp {
        self.send("POST", &routes::command_path(command), &[("Content-Type", "application/json")], Some(body.to_string().as_bytes()))
    }

    pub fn send(&self, method: &str, path: &str, headers: &[(&str, &str)], body: Option<&[u8]>) -> Resp {
        send(self.addr, method, path, headers, body)
    }

    pub fn view(&self, name: &str, query: &str) -> Vec<Value> {
        let r = self.get(&format!("{}?{query}", routes::view_path(name)));
        assert_eq!(r.status, 200, "{name}: {}", r.text());
        r.json()["rows"].as_array().unwrap().clone()
    }

    pub fn wait_state(&self, run: &str, want: &str) {
        self.fx.wait_state(run, want);
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(rt) = self.rt.take() {
            rt.shutdown_background();
        }
    }
}

pub struct Resp {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Resp {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or_else(|e| panic!("not JSON ({e}): {}", self.text()))
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

/// One HTTP/1.1 request on a fresh connection, read to the end of the response.
pub fn send(addr: SocketAddr, method: &str, path: &str, headers: &[(&str, &str)], body: Option<&[u8]>) -> Resp {
    let mut s = TcpStream::connect(addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let mut head = format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n");
    for (k, v) in headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    if let Some(b) = body {
        head.push_str(&format!("Content-Length: {}\r\n", b.len()));
    }
    head.push_str("\r\n");
    s.write_all(head.as_bytes()).unwrap();
    if let Some(b) = body {
        s.write_all(b).unwrap();
    }
    read_response(&mut s)
}

fn read_response(s: &mut TcpStream) -> Resp {
    let mut raw = Vec::new();
    let _ = s.read_to_end(&mut raw);
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap_or_else(|| panic!("no response head in {:?}", String::from_utf8_lossy(&raw)));
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let mut lines = head.lines();
    let status = lines.next().unwrap().split(' ').nth(1).unwrap().parse().unwrap();
    let headers: Vec<(String, String)> = lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect();
    let mut body = raw[split + 4..].to_vec();
    if headers.iter().any(|(k, v)| k.eq_ignore_ascii_case("transfer-encoding") && v.contains("chunked")) {
        body = dechunk(&body);
    }
    Resp { status, headers, body }
}

fn dechunk(mut rest: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let eol = rest.windows(2).position(|w| w == b"\r\n").unwrap();
        let n = usize::from_str_radix(std::str::from_utf8(&rest[..eol]).unwrap().trim(), 16).unwrap();
        if n == 0 {
            return out;
        }
        out.extend_from_slice(&rest[eol + 2..eol + 2 + n]);
        rest = &rest[eol + 2 + n + 2..];
    }
}

fn run_of(r: &Resp) -> String {
    r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_string()
}

#[test]
fn starting_a_server_costs_milliseconds() {
    let t = Instant::now();
    let n = 10;
    for i in 0..n {
        let s = Server::start(&format!("cost-{i}"), None);
        assert_eq!(s.get("/daemon/views/runs").status, 200);
    }
    let each = t.elapsed() / n;
    println!("one server with its daemon, one request and the stop: {each:?}");
    assert!(each < Duration::from_millis(250), "{each:?}");
}

#[test]
fn a_token_is_checked_by_one_middleware_on_every_route() {
    let s = Server::start("auth", Some("s3cret-token"));
    let paths = [
        ("GET", "/daemon/views/runs"),
        ("GET", "/daemon/views/configuration"),
        ("GET", "/daemon/views/events?run_id=x&after_position=0"),
        ("POST", "/daemon/commands/open-session"),
        ("POST", "/daemon/commands/submit-run"),
        ("GET", "/no/such/route"),
        ("POST", "/no/such/route"),
        ("GET", "/"),
        ("DELETE", "/daemon/views/runs"),
    ];
    let wrong = ["Bearer nope", "Bearer s3cret-token-and-more", "Bearer s3cret-toke", "Basic s3cret-token", "s3cret-token", "Bearer", "Bearer "];
    for (method, path) in paths {
        let r = s.send(method, path, &[], None);
        assert_eq!(r.status, 401, "{method} {path} without a token: {}", r.text());
        assert_eq!(r.header("www-authenticate"), Some("Bearer"));
        assert_eq!(r.json()["refused"], "unauthorized");
        for w in wrong {
            assert_eq!(s.send(method, path, &[("Authorization", w)], None).status, 401, "{method} {path} with {w:?}");
        }
        assert!(!r.text().contains("s3cret"), "the token is never echoed");
    }
    let ok = s.get_with("/daemon/views/runs", &[("Authorization", "Bearer s3cret-token")]);
    assert_eq!((ok.status, ok.json()), (200, json!({"rows": []})));
    assert_eq!(s.get_with("/daemon/views/runs", &[("authorization", "bearer s3cret-token")]).status, 200, "scheme and header name are case-insensitive");
    assert_eq!(s.get_with("/no/such/route", &[("Authorization", "Bearer s3cret-token")]).status, 404, "with the token, an unknown route is 404");
    let cmd = s.send(
        "POST",
        "/daemon/commands/open-session",
        &[("Authorization", "Bearer s3cret-token")],
        Some(json!({"session_id": "s1", "client": "t"}).to_string().as_bytes()),
    );
    assert_eq!(cmd.status, 202);
}

#[test]
fn without_a_token_a_loopback_daemon_answers_every_request() {
    let s = Server::start("no-auth", None);
    assert_eq!(s.get("/daemon/views/runs").status, 200);
    assert_eq!(s.get("/no/such/route").status, 404);
}

#[test]
fn one_round_trip_per_client_command() {
    let s = Server::start("roundtrip", None);
    // open-session
    let r = s.post("open-session", &json!({"session_id": "s1", "client": "curl"}));
    assert_eq!((r.status, r.json()["outcome"].clone()), (202, json!("opened")));
    openapi::check_response("open-session", "opened", &r.json());
    let r = s.post("open-session", &json!({"session_id": "s1", "client": "curl"}));
    assert_eq!((r.status, r.json()["error"].clone()), (422, json!("sdc.daemon.SessionExists")));
    openapi::check_response("open-session", "session-exists", &r.json());
    // submit-run: 202, the run id is in the event it publishes, and in the Location header
    let r = s.post("submit-run", &json!({"runbook_id": "ask", "intake": "text", "session_id": "s1", "settings": {"k": "v"}, "idempotency_key": "once"}));
    assert_eq!((r.status, r.json()["outcome"].clone()), (202, json!("submitted")));
    openapi::check_response("submit-run", "submitted", &r.json());
    let run = run_of(&r);
    assert_eq!(r.header("location"), Some(format!("/daemon/views/run?run_id={run}").as_str()));
    let replay = s.post("submit-run", &json!({"runbook_id": "ask", "intake": "text", "session_id": "s1", "idempotency_key": "once"}));
    assert_eq!((replay.status, replay.json()["outcome"].clone()), (202, json!("already-submitted")));
    openapi::check_response("submit-run", "already-submitted", &replay.json());
    assert_eq!(replay.header("location"), r.header("location"), "the replay names the first run in its Location header");
    assert_eq!(replay.json()["published"][0]["event"], "sdc.daemon.SubmitReplayed");
    assert_eq!(replay.json()["published"][0]["payload"]["run_id"], json!(run), "and in its body, as data of the event it publishes");
    assert_eq!(s.view("runs", "").len(), 1);
    // answer-decision
    s.fx.wait_open_decisions(&run, 1);
    let open = s.view("open-decisions", &format!("run_id={run}"));
    assert_eq!(open[0]["request_id"], "q1");
    let answer = json!({"run_id": run, "request_id": "q1", "answer": "A", "answered_by": "operator"});
    let r = s.post("answer-decision", &answer);
    assert_eq!((r.status, r.json()["outcome"].clone()), (202, json!("answered")));
    openapi::check_response("answer-decision", "answered", &r.json());
    s.wait_state(&run, "Ended");
    let r = s.post("answer-decision", &json!({"run_id": run, "request_id": "nope", "answer": "A", "answered_by": "o"}));
    assert_eq!((r.status, r.json()["error"].clone()), (404, json!("sdc.daemon.NoSuchDecisionRequest")));
    openapi::check_response("answer-decision", "no-request", &r.json());
    // A second answer to the only open request of a run that has ended: the request's state is judged
    // first, so the answer is 409 DecisionAlreadyAnswered and not RunNotWaiting.
    let r = s.post("answer-decision", &answer);
    assert_eq!((r.status, r.json()["error"].clone()), (409, json!("sdc.daemon.DecisionAlreadyAnswered")));
    assert_eq!(r.json()["payload"], json!({"request_id": "q1"}));
    openapi::check_response("answer-decision", "already-answered", &r.json());
    assert_eq!(s.view("decisions", &format!("run_id={run}"))[0]["answered_by"], "operator", "the first answer stands");
    // cancel-run
    let held = run_of(&s.post("submit-run", &json!({"runbook_id": "hold", "intake": "x"})));
    s.wait_state(&held, "Running");
    let r = s.post("cancel-run", &json!({"run_id": held}));
    assert_eq!((r.status, r.json()["outcome"].clone()), (202, json!("cancelled")));
    openapi::check_response("cancel-run", "cancelled", &r.json());
    assert_eq!(s.view("run", &format!("run_id={held}"))[0]["state"], "Cancelled");
    let r = s.post("cancel-run", &json!({"run_id": held}));
    assert_eq!((r.status, r.json()["error"].clone()), (409, json!("sdc.daemon.RunAlreadyEnded")));
    openapi::check_response("cancel-run", "already-ended", &r.json());
    assert_eq!(s.view("run", &format!("run_id={held}"))[0]["state"], "Cancelled", "the state stays");
    let r = s.post("cancel-run", &json!({"run_id": "none"}));
    assert_eq!((r.status, r.json()["error"].clone()), (404, json!("sdc.daemon.NoSuchRun")));
    // close-session
    let r = s.post("close-session", &json!({"session_id": "s1"}));
    assert_eq!((r.status, r.json()["outcome"].clone()), (202, json!("closed")));
    openapi::check_response("close-session", "closed", &r.json());
    let r = s.post("close-session", &json!({"session_id": "s1"}));
    assert_eq!((r.status, r.json()["error"].clone()), (409, json!("sdc.daemon.SessionStateConflict")));
    let r = s.post("close-session", &json!({"session_id": "none"}));
    assert_eq!((r.status, r.json()["error"].clone()), (404, json!("sdc.daemon.NoSuchSession")));
    // the refusals of a submit carry their status
    let refusals = [
        (json!({"runbook_id": "nothing", "intake": "x"}), 422, "sdc.daemon.NoSuchRunbook"),
        (json!({"runbook_id": "quick", "intake": "x", "confinement": "local"}), 409, "sdc.daemon.BelowConfinementFloor"),
        (json!({"runbook_id": "quick", "intake": "x", "session_id": "none"}), 409, "sdc.daemon.NoSuchSession"),
        (json!({"runbook_id": "quick", "intake": "x", "session_id": "s1"}), 409, "sdc.daemon.SessionClosedToRuns"),
        (json!({"runbook_id": "quick", "intake": "x", "context_path": "/"}), 422, "sdc.daemon.ContextOutsideRoots"),
    ];
    for (body, status, error) in refusals {
        let r = s.post("submit-run", &body);
        assert_eq!((r.status, r.json()["error"].clone()), (status, json!(error)), "{body}");
        let outcome = r.json()["outcome"].as_str().unwrap().to_string();
        openapi::check_response("submit-run", &outcome, &r.json());
    }
}

#[test]
fn one_round_trip_per_view() {
    let s = Server::start("views", None);
    s.post("open-session", &json!({"session_id": "s1", "client": "curl"}));
    let a = run_of(&s.post("submit-run", &json!({"runbook_id": "ask2", "intake": "text", "session_id": "s1"})));
    let b = run_of(&s.post("submit-run", &json!({"runbook_id": "quick", "intake": "x"})));
    s.wait_state(&b, "Ended");
    s.fx.wait_open_decisions(&a, 2);
    s.post("answer-decision", &json!({"run_id": a, "request_id": "q1", "answer": "A", "answered_by": "operator"}));

    let ids = |rows: Vec<Value>| rows.iter().map(|r| r["run_id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    let runs = s.view("runs", "");
    runs.iter().for_each(|r| openapi::check_row("Runs", r));
    assert_eq!(ids(runs), [b.clone(), a.clone()]);
    let of = s.view("runs-of-session", "session_id=s1");
    of.iter().for_each(|r| openapi::check_row("RunsOfSession", r));
    assert_eq!(ids(of), std::slice::from_ref(&a));
    let st = s.view("runs-in-state", "state=Ended");
    st.iter().for_each(|r| openapi::check_row("RunsInState", r));
    assert_eq!(ids(st), std::slice::from_ref(&b));
    let both = s.view("runs-of-session-in-state", "session_id=s1&state=WaitingOnDecision");
    both.iter().for_each(|r| openapi::check_row("RunsOfSessionInState", r));
    assert_eq!(ids(both), std::slice::from_ref(&a));
    assert!(s.view("runs-of-session-in-state", "session_id=s1&state=Ended").is_empty());
    let one = s.view("run", &format!("run_id={a}"));
    one.iter().for_each(|r| openapi::check_row("RunDetail", r));
    assert_eq!((one[0]["state"].as_str(), one[0]["intake"].as_str(), one[0]["confinement"].as_str()), (Some("WaitingOnDecision"), Some("text"), Some("linux")));
    assert!(s.view("run", "run_id=none").is_empty());
    let open = s.view("open-decisions", &format!("run_id={a}"));
    open.iter().for_each(|r| openapi::check_row("OpenDecisionsOfRun", r));
    assert_eq!(open.len(), 1, "q1 is answered");
    let all = s.view("decisions", &format!("run_id={a}"));
    all.iter().for_each(|r| openapi::check_row("DecisionsOfRun", r));
    assert_eq!(all.iter().map(|d| d["state"].as_str().unwrap()).collect::<Vec<_>>(), ["Answered", "Open"]);
    let sessions = s.view("sessions", "");
    sessions.iter().for_each(|r| openapi::check_row("Sessions", r));
    assert_eq!(sessions[0]["state"], "Open");
    let cfg = s.view("configuration", "");
    cfg.iter().for_each(|r| openapi::check_row("DaemonConfiguration", r));
    assert_eq!((cfg.len(), cfg[0]["confinement_floor"].as_str()), (1, Some("linux")));

    let events = s.view("events", &format!("run_id={a}&after_position=0"));
    events.iter().for_each(|r| openapi::check_row("EventsAfter", r));
    assert_eq!(events[0]["kind"], "runbook.submitted");
    let last = events.last().unwrap()["position"].as_u64().unwrap();
    assert!(s.view("events", &format!("run_id={a}&after_position={last}")).is_empty());
    let sev = s.view("session-events", "session_id=s1&after_position=0");
    sev.iter().for_each(|r| openapi::check_row("EventsOfSessionAfter", r));
    assert!(sev.iter().all(|e| e["run_id"] == json!(a)), "the session holds one run");
    let dev = s.view("daemon-events", "after_position=0");
    dev.iter().for_each(|r| openapi::check_row("EventsOfDaemonAfter", r));
    let positions: Vec<u64> = dev.iter().map(|e| e["position"].as_u64().unwrap()).collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]) && positions[0] == 1, "{positions:?}");
    let mid = s.view("daemon-events", &format!("after_position={}", positions[0]));
    assert_eq!(mid.len(), dev.len() - 1);
}

#[test]
fn the_server_has_the_routes_of_the_openapi_document_and_no_others() {
    // Routes the document has and the server must serve; there is no exception in this direction.
    let doc = openapi::routes();
    let ours: BTreeSet<(String, String)> = routes::COMMANDS
        .iter()
        .map(|c| ("POST".to_string(), routes::command_path(c)))
        .chain(routes::VIEWS.iter().map(|v| ("GET".to_string(), routes::view_path(v))))
        .collect();
    assert_eq!(ours, doc, "runbook/src/api/routes.rs against ess/generated/openapi/openapi/runbookd.yaml");
    // The live server, route by route: a documented route is not 404, a path beside them is.
    let s = Server::start("routes", None);
    for (method, path) in &doc {
        let r = s.send(method, path, &[], if method == "POST" { Some(b"{}") } else { None });
        assert_ne!(r.status, 404, "{method} {path}");
        assert_ne!(r.status, 405, "{method} {path}");
    }
    // Exceptions to "no other route": none. The 406 is an answer of a documented route, and the
    // document has no liveness route, so there is no /healthz.
    for path in ["/healthz", "/daemon", "/daemon/views", "/daemon/views/nope", "/daemon/commands/nope", "/ui", "/events", "/mcp", "/daemon/views/runs/"] {
        assert_eq!(s.get(path).status, 404, "GET {path}");
    }
    // Every command the specification has that is not a client command (the runtime's reports,
    // `start-daemon`, the daemon's own `end-orphaned-run`) is not accepted from the network. The list is
    // the outcome table, which a test holds equal to the domain file: a new command is in it by itself.
    let internal: BTreeSet<&str> = OUTCOMES.iter().map(|o| o.command).filter(|c| !routes::COMMANDS.contains(c)).collect();
    for must in ["start-run", "end-run", "start-daemon", "end-orphaned-run", "withdraw-decision"] {
        assert!(internal.contains(must), "{must} is a command of the specification and not a client command");
    }
    for command in internal {
        assert_eq!(s.post(command, &json!({})).status, 404, "POST {command}");
        assert!(!doc.contains(&("POST".to_string(), routes::command_path(command))), "{command} has no route in the document");
    }
    assert_eq!(s.send("POST", "/daemon/views/runs", &[], Some(b"{}")).status, 405);
    assert_eq!(s.get("/daemon/commands/submit-run").status, 405);
}

#[test]
fn a_body_the_command_layer_cannot_take_is_400_and_a_wrong_query_too() {
    let s = Server::start("bad", None);
    let bad = [
        s.send("POST", "/daemon/commands/submit-run", &[], Some(b"not json")),
        s.send("POST", "/daemon/commands/submit-run", &[], Some(b"")),
        s.post("submit-run", &json!({"intake": "x"})),
        s.post("submit-run", &json!({"runbook_id": "quick", "intake": 5})),
        s.post("submit-run", &json!({"runbook_id": "quick", "intake": "x", "confinement": "oci"})),
        s.post("submit-run", &json!({"runbook_id": "quick", "intake": "x", "idempotency_key": "k".repeat(201)})),
        s.post("open-session", &json!({"session_id": "s", "client": "c".repeat(201)})),
        s.get("/daemon/views/run"),
        s.get("/daemon/views/events?run_id=r"),
        s.get("/daemon/views/events?run_id=r&after_position=-1"),
        s.get("/daemon/views/daemon-events?after_position=abc"),
        s.get("/daemon/views/runs-in-state?state=Finished"),
    ];
    for r in bad {
        assert_eq!(r.status, 400, "{}", r.text());
        let body = r.json();
        assert!(body["refused"].is_string() && body["message"].is_string(), "{body}");
    }
    assert!(s.view("runs", "").is_empty(), "nothing was made");
}

#[test]
fn what_the_daemon_fills_a_client_cannot_choose() {
    let s = Server::start("filled", None);
    // The whole input of the OpenAPI document: run_id, daemon_id, runbook_known, context_inside_roots and
    // at are the handler's; sent by a client they are ignored.
    let r = s.post(
        "submit-run",
        &json!({"run_id": "mine", "daemon_id": "x", "runbook_known": true, "context_inside_roots": true, "at": "2000-01-01T00:00:00Z",
                "runbook_id": "nothing", "intake": "x", "settings": {}, "confinement": "linux"}),
    );
    assert_eq!((r.status, r.json()["error"].clone()), (422, json!("sdc.daemon.NoSuchRunbook")), "runbook_known is the daemon's fact");
    let r = s.post("submit-run", &json!({"run_id": "mine", "runbook_id": "quick", "intake": "x", "context_path": "/", "context_inside_roots": true}));
    assert_eq!(r.json()["error"], "sdc.daemon.ContextOutsideRoots", "context_inside_roots is the daemon's fact");
    let r = s.post("submit-run", &json!({"run_id": "mine", "runbook_id": "quick", "intake": "x", "at": "2000-01-01T00:00:00Z"}));
    let run = run_of(&r);
    assert_ne!(run, "mine");
    assert!(!s.view("run", &format!("run_id={run}"))[0]["submitted_at"].as_str().unwrap().starts_with("2000"));
    let r = s.post("cancel-run", &json!({"run_id": run, "at": "2000-01-01T00:00:00Z"}));
    assert!(r.status == 202 || r.status == 409);
}

#[test]
fn the_intake_limit_holds_over_http_and_the_body_limit_is_a_little_over_what_a_valid_submit_needs() {
    let s = Server::start("limits", None);
    let body = |intake: &str| json!({"runbook_id": "quick", "intake": intake}).to_string().into_bytes();
    let submit = |b: &[u8]| s.send("POST", "/daemon/commands/submit-run", &[("Content-Type", "application/json")], Some(b));
    let at = submit(&body(&"a".repeat(MAX_INTAKE_BYTES)));
    assert_eq!((at.status, at.json()["outcome"].clone()), (202, json!("submitted")));
    let over = submit(&body(&"a".repeat(MAX_INTAKE_BYTES + 1)));
    assert_eq!((over.status, over.json()["error"].clone()), (422, json!("sdc.daemon.IntakeTooLarge")));
    let multi = submit(&body(&"é".repeat(MAX_INTAKE_BYTES / 2)));
    assert_eq!(multi.status, 202, "1,048,576 bytes of two-byte characters");
    let multi_over = submit(&body(&"é".repeat(MAX_INTAKE_BYTES / 2 + 1)));
    assert_eq!((multi_over.status, multi_over.json()["error"].clone()), (422, json!("sdc.daemon.IntakeTooLarge")));
    // A valid intake that JSON writes at its widest: a control character is six bytes of body.
    let widest = submit(&body(&"\u{1}".repeat(MAX_INTAKE_BYTES)));
    assert_eq!(widest.status, 202, "a valid submit of 1 MiB of control characters is a body of {} bytes", MAX_INTAKE_BYTES * 6 + 60);
    // Over the body limit: refused on the header, before any of the body is sent.
    let declared = api::MAX_BODY_BYTES + 1;
    let mut stream = TcpStream::connect(s.addr).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    stream.write_all(format!("POST /daemon/commands/submit-run HTTP/1.1\r\nHost: x\r\nConnection: close\r\nContent-Length: {declared}\r\n\r\n").as_bytes()).unwrap();
    let r = read_response(&mut stream);
    assert_eq!(r.status, 413, "{}", r.text());
    assert_eq!(r.json()["refused"], "request body too large");
    assert_eq!(s.view("runs", "").len(), 3, "the refused submits made no run");
}

#[test]
fn a_chunked_body_over_the_limit_is_refused_while_it_streams() {
    let s = Server::start("chunked", None);
    let mut stream = TcpStream::connect(s.addr).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    stream.write_all(b"POST /daemon/commands/submit-run HTTP/1.1\r\nHost: x\r\nConnection: close\r\nTransfer-Encoding: chunked\r\n\r\n").unwrap();
    let chunk = vec![b'a'; 1 << 20];
    let mut sent = 0usize;
    let mut refused = false;
    while sent < api::MAX_BODY_BYTES * 2 {
        let mut frame = format!("{:x}\r\n", chunk.len()).into_bytes();
        frame.extend_from_slice(&chunk);
        frame.extend_from_slice(b"\r\n");
        if stream.write_all(&frame).is_err() {
            refused = true;
            break;
        }
        sent += chunk.len();
    }
    let r = read_response(&mut stream);
    assert_eq!(r.status, 413, "{}", r.text());
    assert!(refused || sent >= api::MAX_BODY_BYTES, "sent {sent}");
    assert!(sent < api::MAX_BODY_BYTES * 2 || !refused, "the server read the whole body: {sent}");
}

#[test]
fn every_client_outcome_of_the_table_is_in_the_document_with_its_error() {
    for s in OUTCOMES.iter().filter(|s| routes::COMMANDS.contains(&s.command)) {
        let schema = &openapi::doc()["components"]["schemas"];
        let name = format!(
            "sdc.daemon.{}.{}.Response",
            s.command.split('-').map(|w| w[..1].to_uppercase() + &w[1..]).collect::<String>(),
            s.outcome
        );
        let r = &schema[&name];
        assert!(!r.is_null(), "{name}");
        let want = s.error.map(|e| format!("sdc.daemon.{e}"));
        assert_eq!(r["properties"]["error"]["const"].as_str().map(String::from), want, "{name}");
    }
}

// ---- the binary: `runbook serve` as a process ----

struct Proc {
    child: std::process::Child,
}

impl Drop for Proc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn serve_cmd(tag: &str, args: &[&str], token: Option<&str>) -> (std::process::Command, std::path::PathBuf) {
    let dir = super::state::scratch(tag);
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_runbook"));
    cmd.arg("serve").arg("--state-dir").arg(dir.join("state")).args(args);
    cmd.env_remove("RUNBOOK_TOKEN");
    if let Some(t) = token {
        cmd.env("RUNBOOK_TOKEN", t);
    }
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    (cmd, dir)
}

/// Run a command that must exit by itself within `limit`; its status, standard output and standard error.
fn exits_within(mut cmd: std::process::Command, limit: Duration) -> (Option<i32>, String, String) {
    let mut p = Proc { child: cmd.spawn().unwrap() };
    let (mut out, mut err) = (p.child.stdout.take().unwrap(), p.child.stderr.take().unwrap());
    let readers = std::thread::spawn(move || {
        let (mut o, mut e) = (String::new(), String::new());
        let _ = out.read_to_string(&mut o);
        let _ = err.read_to_string(&mut e);
        (o, e)
    });
    let until = Instant::now() + limit;
    let status = loop {
        if let Some(s) = p.child.try_wait().unwrap() {
            break s.code();
        }
        assert!(Instant::now() < until, "the process did not exit within {limit:?}");
        std::thread::sleep(Duration::from_millis(10));
    };
    let (o, e) = readers.join().unwrap();
    (status, o, e)
}

#[test]
fn a_listen_address_that_is_not_loopback_needs_a_token_and_the_process_exits_1_at_once() {
    let t = Instant::now();
    let (cmd, dir) = serve_cmd("nonloop", &["--listen", "0.0.0.0:7000"], None);
    let (status, out, err) = exits_within(cmd, Duration::from_secs(2));
    assert_eq!(status, Some(1), "{err}");
    assert!(err.contains("a token is required") && err.contains("0.0.0.0:7000"), "{err}");
    assert!(out.is_empty(), "nothing was announced: {out}");
    assert!(!dir.join("state").exists(), "nothing was written before the floor held");
    println!("exit after {:?}", t.elapsed());
}

#[test]
fn a_token_file_that_others_can_read_is_refused_and_the_message_names_the_mode() {
    use std::os::unix::fs::PermissionsExt;
    let dir = super::state::scratch("tokenmode");
    let file = dir.join("token");
    std::fs::write(&file, "s3cret\n").unwrap();
    for mode in [0o644, 0o604, 0o607, 0o666] {
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(mode)).unwrap();
        let (cmd, _) = serve_cmd("tokenmode-run", &["--listen", "127.0.0.1:0", "--token-file", file.to_str().unwrap()], None);
        let (status, out, err) = exits_within(cmd, Duration::from_secs(2));
        assert_eq!(status, Some(1), "mode {mode:o}: {err}");
        assert!(err.contains(&format!("mode {mode:04o}")), "{err}");
        assert!(!err.contains("s3cret") && !out.contains("s3cret"), "the token is never printed");
    }
    // The group may read it; the others may not.
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o640)).unwrap();
    let (cmd, _) = serve_cmd("tokenmode-ok", &["--listen", "127.0.0.1:0", "--token-file", file.to_str().unwrap()], None);
    let mut p = Proc { child: { let mut c = cmd; c.spawn().unwrap() } };
    let line = first_line(&mut p);
    assert!(line.starts_with("runbook: listening on http://127.0.0.1:"), "{line}");
}

fn first_line(p: &mut Proc) -> String {
    let out = p.child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = std::io::BufRead::read_line(&mut std::io::BufReader::new(out), &mut line);
        let _ = tx.send(line);
    });
    rx.recv_timeout(Duration::from_secs(10)).expect("no first line of output within 10 s").trim_end().to_string()
}

fn bound(line: &str) -> SocketAddr {
    line.strip_prefix("runbook: listening on http://").unwrap_or_else(|| panic!("{line}")).parse().unwrap()
}

#[test]
fn without_a_state_dir_the_record_is_under_xdg_state_home_runbook() {
    let dir = super::state::scratch("xdg");
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_runbook"));
    cmd.args(["serve", "--listen", "127.0.0.1:0"]).env_remove("RUNBOOK_TOKEN").env("XDG_STATE_HOME", dir.join("xdg"));
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut p = Proc { child: cmd.spawn().unwrap() };
    let addr = bound(&first_line(&mut p));
    let body = json!({"runbook_id": "sdc", "intake": "hello"}).to_string();
    let r = send(addr, "POST", "/daemon/commands/submit-run", &[("Content-Type", "application/json")], Some(body.as_bytes()));
    let run = run_of(&r);
    let root = dir.join("xdg/runbook");
    assert!(root.join("runs").join(&run).is_dir(), "runs/<id>");
    assert!(root.join("workspaces").join(&run).is_dir(), "workspaces/<id>");
    assert!(root.join("journal.jsonl").is_file() && root.join("daemon.lock").is_file());
}

#[test]
fn serve_prints_the_bound_address_keeps_running_and_checks_the_token_of_the_file_or_the_environment() {
    let dir = super::state::scratch("process");
    let file = dir.join("token");
    std::fs::write(&file, "from-file\n").unwrap();
    std::fs::set_permissions(&file, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();
    // The file wins over the environment.
    let (cmd, _) = serve_cmd("process-run", &["--listen", "127.0.0.1:0", "--token-file", file.to_str().unwrap()], Some("from-env"));
    let mut p = Proc { child: { let mut c = cmd; c.spawn().unwrap() } };
    let addr = bound(&first_line(&mut p));
    assert!(addr.ip().is_loopback() && addr.port() != 0);
    assert_eq!(send(addr, "GET", "/daemon/views/runs", &[], None).status, 401);
    assert_eq!(send(addr, "GET", "/no/such", &[], None).status, 401);
    assert_eq!(send(addr, "GET", "/daemon/views/runs", &[("Authorization", "Bearer from-env")], None).status, 401);
    let ok = send(addr, "GET", "/daemon/views/runs", &[("Authorization", "Bearer from-file")], None);
    assert_eq!((ok.status, ok.json()), (200, json!({"rows": []})));
    assert!(p.child.try_wait().unwrap().is_none(), "it keeps running");
    drop(p);

    // The environment alone.
    let (cmd, _) = serve_cmd("process-env", &["--listen", "127.0.0.1:0"], Some("from-env"));
    let mut p = Proc { child: { let mut c = cmd; c.spawn().unwrap() } };
    let addr = bound(&first_line(&mut p));
    assert_eq!(send(addr, "GET", "/daemon/views/runs", &[], None).status, 401);
    assert_eq!(send(addr, "GET", "/daemon/views/runs", &[("Authorization", "Bearer from-env")], None).status, 200);
    drop(p);

    // No token on loopback: open. A second daemon on the same state directory is refused.
    let (cmd, dir2) = serve_cmd("process-open", &["--listen", "127.0.0.1:0"], None);
    let mut p = Proc { child: { let mut c = cmd; c.spawn().unwrap() } };
    let addr = bound(&first_line(&mut p));
    assert_eq!(send(addr, "GET", "/daemon/views/runs", &[], None).status, 200);
    let submit = send(addr, "POST", "/daemon/commands/submit-run", &[("Content-Type", "application/json")], Some(json!({"runbook_id": "sdc", "intake": "hello"}).to_string().as_bytes()));
    assert_eq!((submit.status, submit.json()["outcome"].clone()), (202, json!("submitted")));
    let mut second = std::process::Command::new(env!("CARGO_BIN_EXE_runbook"));
    second.arg("serve").arg("--state-dir").arg(dir2.join("state")).args(["--listen", "127.0.0.1:0"]);
    second.env_remove("RUNBOOK_TOKEN").stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let (status, _, err) = exits_within(second, Duration::from_secs(5));
    assert_eq!(status, Some(1), "{err}");
    assert!(err.contains("in use by another daemon"), "{err}");
}

