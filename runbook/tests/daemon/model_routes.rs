//! The model routes a daemon declares (`docs/model-routes.md`, section 1; `docs/daemon-api.md`, "Model routes"):
//! `[gateways.<name>]`, `[routes.<name>]` and `[route_policy]` of the daemon's `runbook.toml`, a lone `[gateway]`
//! read as the gateway `default`, and the view `routes` (`GET /daemon/views/routes`, the MCP tool `list_routes`)
//! with each route's alias, kind, wires and status as the gateway gives them.
//!
//! No test calls a model, a pod or a network beyond loopback: every gateway is a stand-in on a loopback port of the
//! test's own, which answers `GET` requests from a table and records the head of every request it gets. The token
//! is a string the case invents.

use super::default_confinement::{live, rows};
use super::http::exits_within;
use super::openapi;
use runbook::api::routes::view_answer;
use runbook::clock::SetClock;
use runbook::commands::{Daemon, Policy};
use runbook::routes::{RouteKind, RouteTable, Tables};
use runbook::stub::StubRuntime;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const TOKEN: &str = "gw-routes-5d0c8e17a2b4";

/// A gateway on a loopback port: it answers `<METHOD> <path>` from its table (404 for any other) and keeps the head
/// of every request.
struct Standin {
    base_url: String,
    heads: Arc<Mutex<Vec<String>>>,
}

impl Standin {
    fn start(table: Vec<(&str, u16, Value)>) -> Standin {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let heads = Arc::new(Mutex::new(Vec::new()));
        let kept = heads.clone();
        let table: Vec<(String, u16, String)> = table.into_iter().map(|(line, status, body)| (line.to_owned(), status, body.to_string())).collect();
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
                let (status, body) = table.iter().find(|(l, _, _)| *l == line).map(|(_, s, b)| (*s, b.clone())).unwrap_or((404, "{\"error\":{\"code\":\"not-found\",\"message\":\"no such route\"}}".to_owned()));
                let _ = stream.write_all(format!("HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            }
        });
        Standin { base_url, heads }
    }

    fn heads(&self) -> Vec<String> {
        self.heads.lock().unwrap().clone()
    }

    /// The request lines (`GET /ready`) in the order they came.
    fn lines(&self) -> Vec<String> {
        self.heads().iter().map(|h| h.lines().next().unwrap_or("").rsplit_once(' ').map(|(l, _)| l.to_owned()).unwrap_or_default()).collect()
    }

    fn count(&self, line: &str) -> usize {
        self.lines().iter().filter(|l| *l == line).count()
    }
}

fn models(entries: &[(&str, &[&str])]) -> Value {
    json!({"object": "list", "data": entries.iter().map(|(id, wires)| json!({"id": id, "object": "model", "owned_by": "llm-gateway", "wires": wires})).collect::<Vec<_>>()})
}

/// Today's gateway: `/ready`, `/v1/models`, the status of the managed model `qwen`, and no `/v1/status`.
fn todays_gateway() -> Standin {
    Standin::start(vec![
        ("GET /ready", 200, json!({"status": "ready"})),
        ("GET /v1/models", 200, models(&[("claude", &["messages"]), ("qwen", &["chat", "responses"])])),
        ("GET /v1/models/qwen/status", 200, json!({"model": "qwen", "phase": "stopped"})),
    ])
}

fn token_file(dir: &Path) -> PathBuf {
    let secret = dir.join("secret");
    std::fs::create_dir_all(&secret).unwrap();
    let file = secret.join("gateway-token");
    std::fs::write(&file, format!("{TOKEN}\n")).unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    file
}

fn two_routes(base_url: &str, token_file: &Path) -> String {
    format!(
        "[gateways.default]\nbase_url = \"{base_url}\"\ntoken_file = \"{}\"\n\n[routes.qwen]\ngateway = \"default\"\nmodel = \"qwen\"\nkind = \"managed\"\nstart_wait_seconds = 600\nprice_in_micro_usd_per_mtok = 0\nprice_out_micro_usd_per_mtok = 0\nwake_on_submit = false\n\n[routes.claude]\ngateway = \"default\"\nmodel = \"claude\"\nkind = \"hosted\"\nprice_in_micro_usd_per_mtok = 3000000\nprice_out_micro_usd_per_mtok = 15000000\n\n[route_policy]\ndefault = \"claude\"\nclassifier = \"claude\"\n",
        token_file.display()
    )
}

/// The rows of the view without the instant of the read.
fn without_read_at(rows: &[Value]) -> Vec<Value> {
    rows.iter()
        .map(|r| {
            let mut r = r.clone();
            assert!(r["read_at"].as_str().is_some_and(|t| t.ends_with('Z')), "a row has no instant of its read: {r}");
            r.as_object_mut().unwrap().remove("read_at");
            r
        })
        .collect()
}

#[test]
fn two_routes_on_one_standin_gateway_are_listed_with_alias_kind_wires_and_status() {
    let gw = todays_gateway();
    let dir = super::state::scratch("routes-two-files");
    let file = two_routes(&gw.base_url, &token_file(&dir));
    let l = live("routes-two", &[], Some(&file));
    let rows = rows(&l, "/daemon/views/routes");
    rows.iter().for_each(|r| openapi::check_row("Routes", r));
    assert_eq!(
        without_read_at(&rows),
        vec![
            json!({"route": "claude", "gateway": "default", "alias": "claude", "kind": "hosted", "wires": ["messages"], "status": "listed"}),
            json!({"route": "qwen", "gateway": "default", "alias": "qwen", "kind": "managed", "wires": ["chat", "responses"], "status": "not-ready", "phase": "stopped"}),
        ]
    );
    // what the daemon asked: the gateway's readiness, its list, the status route of /v1/status and, as the gateway
    // has none, the managed route's own; every request a GET with the gateway's token, and no model call
    let mut lines = gw.lines();
    lines.sort();
    assert_eq!(lines, ["GET /ready", "GET /v1/models", "GET /v1/models/qwen/status", "GET /v1/status"], "{:?}", gw.heads());
    for head in gw.heads() {
        assert!(head.contains(&format!("Authorization: Bearer {TOKEN}\r\n")), "a read reached the gateway without its token: {head}");
    }
    // the view names no address, port, token, token file or price
    let text = serde_json::to_string(&rows).unwrap();
    let port = gw.base_url.rsplit(':').next().unwrap();
    for value in [TOKEN, gw.base_url.as_str(), port, "gateway-token", "3000000", "price"] {
        assert!(!text.contains(value), "{value} is in the view: {text}");
    }
}

#[test]
fn the_status_of_v1_status_is_read_for_every_kind_when_the_gateway_serves_it_and_no_status_route_of_a_model_is_asked() {
    let gw = Standin::start(vec![
        ("GET /ready", 200, json!({"status": "ready"})),
        ("GET /v1/models", 200, models(&[("claude", &["messages"]), ("qwen", &["chat"]), ("qwen-local", &["chat"])])),
        (
            "GET /v1/status",
            200,
            json!({"format": "llm-gateway.status/1", "models": [
                {"model": "claude", "kind": "hosted", "wires": ["messages"], "phase": "ready", "callable": true},
                {"model": "qwen", "kind": "managed", "wires": ["chat"], "phase": "starting", "callable": true},
                {"model": "qwen-local", "kind": "local", "wires": ["chat"], "phase": "stopped", "sub_phase": "model-not-pulled", "callable": false},
            ]}),
        ),
    ]);
    let file = format!(
        "[gateways.gw]\nbase_url = \"{}\"\n\n[routes.claude]\ngateway = \"gw\"\nmodel = \"claude\"\nkind = \"hosted\"\n\n[routes.qwen]\ngateway = \"gw\"\nmodel = \"qwen\"\nkind = \"managed\"\n\n[routes.local-qwen]\ngateway = \"gw\"\nmodel = \"qwen-local\"\nkind = \"local\"\n",
        gw.base_url
    );
    let l = live("routes-v1-status", &[], Some(&file));
    let rows = rows(&l, "/daemon/views/routes");
    rows.iter().for_each(|r| openapi::check_row("Routes", r));
    assert_eq!(
        without_read_at(&rows),
        vec![
            json!({"route": "claude", "gateway": "gw", "alias": "claude", "kind": "hosted", "wires": ["messages"], "status": "ready", "phase": "ready"}),
            json!({"route": "local-qwen", "gateway": "gw", "alias": "qwen-local", "kind": "local", "wires": ["chat"], "status": "not-ready", "phase": "stopped"}),
            json!({"route": "qwen", "gateway": "gw", "alias": "qwen", "kind": "managed", "wires": ["chat"], "status": "ready", "phase": "starting"}),
        ],
        "callable decides ready, not the phase"
    );
    let mut lines = gw.lines();
    lines.sort();
    assert_eq!(lines, ["GET /ready", "GET /v1/models", "GET /v1/status"]);
    assert!(gw.heads().iter().all(|h| !h.contains("Authorization")), "a gateway without a token file gets no credential");
}

#[test]
fn a_gateway_that_does_not_answer_is_not_ready_or_does_not_list_the_alias_gives_each_route_its_cause() {
    let closed = TcpListener::bind("127.0.0.1:0").unwrap();
    let closed_url = format!("http://{}", closed.local_addr().unwrap());
    drop(closed);
    let unready = Standin::start(vec![("GET /ready", 503, json!({"error": {"code": "unavailable"}}))]);
    let ready = Standin::start(vec![("GET /ready", 200, json!({})), ("GET /v1/models", 200, models(&[("other", &["chat"])]))]);
    let file = format!(
        "[gateways.closed]\nbase_url = \"{closed_url}\"\n[gateways.unready]\nbase_url = \"{}\"\n[gateways.ready]\nbase_url = \"{}\"\n[gateways.tls]\nbase_url = \"https://127.0.0.1:1\"\n\
         [routes.a]\ngateway = \"closed\"\nmodel = \"m\"\nkind = \"managed\"\n[routes.b]\ngateway = \"unready\"\nmodel = \"m\"\nkind = \"local\"\n[routes.c]\ngateway = \"ready\"\nmodel = \"m\"\nkind = \"managed\"\n[routes.d]\ngateway = \"tls\"\nmodel = \"m\"\nkind = \"hosted\"\n",
        unready.base_url, ready.base_url
    );
    let l = live("routes-causes", &[], Some(&file));
    let rows = rows(&l, "/daemon/views/routes");
    rows.iter().for_each(|r| openapi::check_row("Routes", r));
    let got: Vec<(String, String, Value)> = rows.iter().map(|r| (r["route"].as_str().unwrap().to_owned(), r["status"].as_str().unwrap().to_owned(), r["wires"].clone())).collect();
    let none = json!([]);
    assert_eq!(
        got,
        vec![
            ("a".to_owned(), "gateway-unreachable".to_owned(), none.clone()),
            ("b".to_owned(), "gateway-not-ready".to_owned(), none.clone()),
            ("c".to_owned(), "model-unknown".to_owned(), none.clone()),
            ("d".to_owned(), "gateway-unreachable".to_owned(), none),
        ]
    );
    assert!(rows.iter().all(|r| r.get("phase").is_none()), "{rows:?}");
    assert_eq!(unready.lines(), ["GET /ready"], "a gateway that is not ready is asked nothing more");
    assert_eq!(ready.lines(), ["GET /ready", "GET /v1/models"], "an alias the gateway does not list has no status read");
}

#[test]
fn a_local_route_on_a_gateway_without_its_status_routes_is_listed_and_not_unreachable() {
    // today's gateway answers 404 for /v1/status and for the status of a model it does not start
    let gw = Standin::start(vec![("GET /ready", 200, json!({"status": "ready"})), ("GET /v1/models", 200, models(&[("qwen-local", &["chat"])]))]);
    let file = format!("[gateways.gw]\nbase_url = \"{}\"\n[routes.local-qwen]\ngateway = \"gw\"\nmodel = \"qwen-local\"\nkind = \"local\"\n", gw.base_url);
    let l = live("routes-local-404", &[], Some(&file));
    let rows = rows(&l, "/daemon/views/routes");
    rows.iter().for_each(|r| openapi::check_row("Routes", r));
    assert_eq!(without_read_at(&rows), vec![json!({"route": "local-qwen", "gateway": "gw", "alias": "qwen-local", "kind": "local", "wires": ["chat"], "status": "listed"})]);
    assert_eq!(gw.lines(), ["GET /ready", "GET /v1/models", "GET /v1/status", "GET /v1/models/qwen-local/status"]);
}

#[test]
fn what_a_gateway_says_reaches_a_row_only_as_short_visible_words() {
    let long = "p".repeat(65);
    let many: Vec<String> = (0..20).map(|i| format!("w{i}")).collect();
    let gw = Standin::start(vec![
        ("GET /ready", 200, json!({})),
        ("GET /v1/models", 200, json!({"object": "list", "data": [
            {"id": "a", "wires": ["chat", "bad wire", long, 7, "messages"]},
            {"id": "b", "wires": many},
            {"id": "c", "wires": "chat"},
        ]})),
        ("GET /v1/status", 200, json!({"models": [
            {"model": "a", "phase": "p".repeat(65), "callable": true},
            {"model": "b", "phase": "stopped\nready", "callable": false},
            {"model": "c", "phase": 5, "callable": "yes"},
        ]})),
    ]);
    let file = format!(
        "[gateways.gw]\nbase_url = \"{}\"\n[routes.a]\ngateway = \"gw\"\nmodel = \"a\"\nkind = \"hosted\"\n[routes.b]\ngateway = \"gw\"\nmodel = \"b\"\nkind = \"managed\"\n[routes.c]\ngateway = \"gw\"\nmodel = \"c\"\nkind = \"hosted\"\n",
        gw.base_url
    );
    let l = live("routes-hostile", &[], Some(&file));
    let rows = rows(&l, "/daemon/views/routes");
    rows.iter().for_each(|r| openapi::check_row("Routes", r));
    assert_eq!(
        without_read_at(&rows),
        vec![
            json!({"route": "a", "gateway": "gw", "alias": "a", "kind": "hosted", "wires": ["chat", "messages"], "status": "ready"}),
            json!({"route": "b", "gateway": "gw", "alias": "b", "kind": "managed", "wires": many[..16], "status": "not-ready"}),
            json!({"route": "c", "gateway": "gw", "alias": "c", "kind": "hosted", "wires": [], "status": "listed"}),
        ],
        "a wire or a phase that is not a short visible word is left out, at most 16 wires are kept, and a status with neither a phase nor a callable that is a boolean is no status"
    );
}

/// The tables of a daemon on one gateway with one managed route, its token read.
fn one_route(base_url: &str, dir: &Path) -> RouteTable {
    let gateways = json!({"default": {"base_url": base_url, "token_file": token_file(dir).display().to_string()}});
    let routes = json!({"qwen": {"gateway": "default", "model": "qwen", "kind": "managed"}});
    let mut table = RouteTable::declare(&Tables { gateways: Some(&gateways), routes: Some(&routes), ..Tables::default() }).unwrap();
    table.read_tokens().unwrap();
    table
}

#[test]
fn the_gateway_is_read_at_most_once_every_15_seconds_of_the_daemons_clock_and_the_mcp_tool_answers_the_view() {
    let gw = todays_gateway();
    let dir = super::state::scratch("routes-cache");
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    let clock = Arc::new(SetClock::at("2026-11-25T10:00:00Z").unwrap());
    let policy = Policy { routes: Arc::new(one_route(&gw.base_url, &dir)), ..Policy::default() };
    let d = Daemon::start_full(super::commands::config(&dir), policy, Arc::new(StubRuntime::new()), clock.clone()).unwrap();
    let view = |d: &Daemon| {
        let a = view_answer("routes", d, &HashMap::new());
        assert_eq!(a.status, 200, "{}", a.body);
        a.body
    };
    let first = view(&d);
    assert_eq!(first["rows"][0]["read_at"], "2026-11-25T10:00:00.000Z", "{first}");
    assert_eq!(first["rows"][0]["status"], "not-ready", "{first}");
    assert_eq!(gw.count("GET /ready"), 1);
    clock.set("2026-11-25T10:00:14.999Z");
    assert_eq!(view(&d), first, "a read within 15 s answers the rows of the last read");
    let tool = runbook::mcp::tools::call(&runbook::mcp::Local(d.clone()), "list_routes", &json!({})).unwrap();
    assert_eq!((tool.status.as_u16(), tool.body.clone()), (200, first.clone()), "the MCP tool answers the view's body");
    assert_eq!((gw.count("GET /ready"), gw.count("GET /v1/models/qwen/status")), (1, 1), "{:?}", gw.lines());
    clock.set("2026-11-25T10:00:15Z");
    assert_eq!(view(&d), first, "a read 15 s later still answers the rows of the last read at once and starts one read");
    let read = std::time::Instant::now();
    let later = loop {
        let v = view(&d);
        if v != first || read.elapsed() > Duration::from_secs(5) {
            break v;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(later["rows"][0]["read_at"], "2026-11-25T10:00:15.000Z", "{later}");
    assert_eq!(view(&d), later);
    assert_eq!((gw.count("GET /ready"), gw.count("GET /v1/models/qwen/status")), (2, 2), "one read in the background, and none while its rows are fresh: {:?}", gw.lines());
    // list_routes takes no argument
    assert!(runbook::mcp::tools::call(&runbook::mcp::Local(d.clone()), "list_routes", &json!({"route": "qwen"})).is_err());
    d.stop();
}

#[test]
fn a_route_name_resolves_to_its_gateway_address_token_alias_kind_and_prices() {
    let dir = super::state::scratch("routes-resolve");
    let gateways = json!({"default": {"base_url": "http://127.0.0.1:1", "token_file": token_file(&dir).display().to_string()}, "open": {"base_url": "http://127.0.0.1:2/prefix"}});
    let routes = json!({
        "claude": {"gateway": "default", "model": "claude-alias", "kind": "hosted", "price_in_micro_usd_per_mtok": 3_000_000, "price_out_micro_usd_per_mtok": 15_000_000},
        "local-qwen": {"gateway": "open", "model": "qwen-local", "kind": "local", "start_wait_seconds": 30},
    });
    let policy = json!({"default": "claude", "classifier": "local-qwen"});
    let mut table = RouteTable::declare(&Tables { gateways: Some(&gateways), routes: Some(&routes), route_policy: Some(&policy), ..Tables::default() }).unwrap();
    assert!(format!("{table:?}").contains("claude") && !format!("{table:?}").contains(TOKEN));
    table.read_tokens().unwrap();
    assert!(!format!("{table:?}").contains(TOKEN), "Debug holds no token: {table:?}");
    let claude = table.resolve("claude").unwrap();
    assert_eq!((claude.base_url, claude.token, claude.alias, claude.kind), ("http://127.0.0.1:1", Some(TOKEN), "claude-alias", RouteKind::Hosted));
    assert_eq!((claude.prices.in_micro_usd_per_mtok, claude.prices.out_micro_usd_per_mtok), (Some(3_000_000), Some(15_000_000)));
    let local = table.resolve("local-qwen").unwrap();
    assert_eq!((local.base_url, local.token, local.alias, local.kind), ("http://127.0.0.1:2/prefix", None, "qwen-local", RouteKind::Local));
    assert_eq!((local.prices.in_micro_usd_per_mtok, local.prices.out_micro_usd_per_mtok), (None, None));
    assert!(table.resolve("qwen").is_none());
    assert_eq!((table.default_route(), table.classifier()), (Some("claude"), Some("local-qwen")));
    assert_eq!(table.route("local-qwen").map(|r| r.start_wait_seconds), Some(Some(30)));
    assert_eq!(table.names(), ["claude", "local-qwen"]);
}

/// `runbook serve` on `file` that must refuse to start: its status, standard output and standard error.
fn refused(tag: &str, file: &str) -> (Option<i32>, String, String, PathBuf) {
    let dir = super::state::scratch(tag);
    std::fs::write(dir.join("runbook.toml"), file).unwrap();
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_runbook"));
    cmd.arg("serve").args(["--principal", super::PRINCIPAL, "--listen", "127.0.0.1:0"]).arg("--state-dir").arg(dir.join("state"));
    cmd.env_remove("RUNBOOK_TOKEN").env("RUNBOOK_CONFIG", dir.join("runbook.toml"));
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let (status, out, err) = exits_within(cmd, Duration::from_secs(10));
    // the path of the file is named (it holds the process id, whose digits a value could share); nothing else of it
    let named = dir.display().to_string();
    (status, out.replace(&named, "<dir>"), err.replace(&named, "<dir>"), dir)
}

#[test]
fn a_route_naming_an_undeclared_gateway_a_gateway_without_base_url_or_a_token_in_the_file_is_refused_at_start_by_key() {
    let dir = super::state::scratch("routes-refused-files");
    let tokens = token_file(&dir);
    let empty = dir.join("empty-token");
    std::fs::write(&empty, "").unwrap();
    let gw = "[gateways.gw]\nbase_url = \"http://127.0.0.1:8099\"\n";
    let route = |extra: &str| format!("{gw}[routes.r1]\ngateway = \"gw\"\nmodel = \"qwen-secret-alias\"\nkind = \"managed\"\n{extra}");
    let value = "tok-in-file-9f3b2c71";
    let cases: Vec<(String, &str)> = vec![
        (format!("{gw}[routes.r1]\ngateway = \"elsewhere-gw\"\nmodel = \"m\"\nkind = \"hosted\"\n"), "routes.r1.gateway"),
        ("[gateways.gw]\ntoken_file = \"/nonexistent/holder\"\n[routes.r1]\ngateway = \"gw\"\nmodel = \"m\"\nkind = \"hosted\"\n".to_owned(), "gateways.gw.base_url"),
        (format!("[gateways.gw]\nbase_url = \"http://127.0.0.1:8099\"\ntoken = \"{value}\"\n"), "gateways.gw.token"),
        (route(&format!("token = \"{value}\"\n")), "routes.r1.token"),
        (route(&format!("api_key = \"{value}\"\n")), "routes.r1.api_key"),
        (format!("{gw}[gateways.gw.headers]\nauthorization = \"{value}\"\n"), "gateways.gw.headers"),
        (route("wake_on_submit = true\nwake_daily_cap_micro_usd = 5000000\n"), "routes.r1.wake_on_submit"),
        (route("kind_of = \"x\"\n"), "routes.r1.kind_of"),
        (format!("{gw}[routes.r1]\ngateway = \"gw\"\nmodel = \"m\"\nkind = \"remote-secret-kind\"\n"), "routes.r1.kind"),
        (format!("{gw}[routes.r1]\ngateway = \"gw\"\nmodel = \"http://10.1.2.3:9/v1\"\nkind = \"hosted\"\n"), "routes.r1.model"),
        (format!("{gw}[routes.r1]\ngateway = \"gw\"\nkind = \"hosted\"\n"), "routes.r1.model"),
        (format!("{gw}[routes.r1]\nmodel = \"m\"\nkind = \"hosted\"\n"), "routes.r1.gateway"),
        (route("wake_cost_micro_usd = 7\n").replace("\"managed\"", "\"hosted\""), "routes.r1.wake_cost_micro_usd"),
        (route("price_in_micro_usd_per_mtok = \"cheap-secret\"\n"), "routes.r1.price_in_micro_usd_per_mtok"),
        (format!("{}[route_policy]\ndefault = \"no-such-secret-route\"\n", route("")), "route_policy.default"),
        (format!("{}[route_policy]\nclassifier = \"no-such-secret-route\"\n", route("")), "route_policy.classifier"),
        (format!("{}[route_policy]\nfallback = \"r1\"\n", route("")), "route_policy.fallback"),
        ("[gateways.gw]\nbase_url = \"ftp://secret-host\"\n".to_owned(), "gateways.gw.base_url"),
        (format!("[gateways.gw]\nbase_url = \"http://127.0.0.1:8099\"\ntoken_file = \"{}\"\n", dir.join("no-such-dir/token").display()), "gateways.gw.token_file"),
        (format!("[gateways.gw]\nbase_url = \"http://127.0.0.1:8099\"\ntoken_file = \"{}\"\n", empty.display()), "gateways.gw.token_file"),
        (format!("[gateway]\nbase_url = \"http://127.0.0.1:8099\"\ntoken_file = \"{}\"\n[gateways.default]\nbase_url = \"http://127.0.0.1:8098\"\n", tokens.display()), "gateways.default"),
        ("[gateways.\"bad name\"]\nbase_url = \"http://127.0.0.1:8099\"\n".to_owned(), "gateways"),
    ];
    for (n, (file, key)) in cases.iter().enumerate() {
        let (status, out, err, d) = refused(&format!("routes-refused-{n}"), file);
        assert_eq!(status, Some(2), "case {n} ({key}): the daemon started: {out} {err}");
        assert!(err.contains(key), "case {n}: the refusal does not name {key}: {err}");
        for v in [value, "8099", "8098", "elsewhere-gw", "qwen-secret-alias", "secret", "10.1.2.3", "no-such-dir", "empty-token", "/nonexistent"] {
            assert!(!err.contains(v) && !out.contains(v), "case {n} ({key}): {v} is in the refusal: {err}");
        }
        assert!(!d.join("state").exists(), "case {n} ({key}): nothing written");
    }
}

#[test]
fn a_gateway_table_alone_works_as_today_and_a_route_names_it_as_the_gateway_default() {
    let gw = todays_gateway();
    let dir = super::state::scratch("routes-legacy-files");
    let gateway = format!("[gateway]\nbase_url = \"{}\"\ntoken_file = \"{}\"\n", gw.base_url, token_file(&dir).display());
    // alone: the daemon starts, declares no route and reads no gateway
    let l = live("routes-legacy-alone", &[], Some(&gateway));
    assert_eq!(rows(&l, "/daemon/views/routes"), Vec::<Value>::new());
    assert!(gw.heads().is_empty(), "a daemon without a route reads no gateway");
    drop(l);
    // as today, its token file is read only by the model decider that takes it: a daemon without one starts with a
    // token file it could not read
    let unread = format!("[gateway]\nbase_url = \"{}\"\ntoken_file = \"{}\"\n", gw.base_url, dir.join("no-such-token").display());
    let l = live("routes-legacy-unread", &[], Some(&unread));
    assert_eq!(rows(&l, "/daemon/views/routes"), Vec::<Value>::new());
    drop(l);
    // with a route on the gateway `default`: the lone table is that gateway, with its address and its token
    let file = format!("{gateway}[routes.qwen]\ngateway = \"default\"\nmodel = \"qwen\"\nkind = \"managed\"\n");
    let l = live("routes-legacy-route", &[], Some(&file));
    let rows = rows(&l, "/daemon/views/routes");
    assert_eq!(without_read_at(&rows), vec![json!({"route": "qwen", "gateway": "default", "alias": "qwen", "kind": "managed", "wires": ["chat", "responses"], "status": "not-ready", "phase": "stopped"})]);
    assert!(!gw.heads().is_empty() && gw.heads().iter().all(|h| h.contains(&format!("Authorization: Bearer {TOKEN}\r\n"))), "{:?}", gw.heads());
}
