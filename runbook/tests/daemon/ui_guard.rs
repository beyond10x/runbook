//! The one guard in front of every route (`src/api/auth.rs`): a foreign `Host` or `Origin` is 403, a
//! command or an MCP call that is not `application/json` is 415, on a command, a view, an event
//! stream, `/mcp`, a UI file and the login, with the UI or without it; and the build refuses a
//! symbolic link in `ui/web/`.

use super::commands::Fx;
use super::http::Resp;
use runbook::api;
use runbook::mcp;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::process::Command;
use std::time::Duration;

const TOKEN: &str = "t0ken-for-the-guard-tests";
const OPEN: &[u8] = br#"{"session_id":"guard","client":"a test"}"#;
const PING: &[u8] = br#"{"jsonrpc":"2.0","id":1,"method":"ping","params":{}}"#;

struct Srv {
    _fx: Fx,
    addr: SocketAddr,
    rt: Option<tokio::runtime::Runtime>,
}

impl Srv {
    fn start(tag: &str, token: Option<&str>, ui: bool) -> Srv {
        Srv::on("127.0.0.1:0", tag, token, ui)
    }

    fn on(bind: &str, tag: &str, token: Option<&str>, ui: bool) -> Srv {
        let fx = Fx::new(tag);
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        let listener = rt.block_on(tokio::net::TcpListener::bind(bind)).unwrap();
        let addr = SocketAddr::from(([127, 0, 0, 1], listener.local_addr().unwrap().port()));
        let (daemon, token) = (fx.d().clone(), token.map(String::from));
        rt.spawn(async move {
            let extra = mcp::router(daemon.clone());
            let _ = api::serve_all(listener, daemon, token.as_deref(), api::Streams::default(), ui, Some(extra), std::future::pending()).await;
        });
        Srv { _fx: fx, addr, rt: Some(rt) }
    }

    fn own(&self) -> String {
        format!("127.0.0.1:{}", self.addr.port())
    }

    /// One request with the `Host` given (`None`: HTTP/1.0 without a `Host`); the answer is read to
    /// the end of its head, and to the end of the body unless `stream` is set (an event stream has none).
    fn call(&self, host: Option<&str>, method: &str, path: &str, headers: &[(&str, &str)], body: Option<&[u8]>, stream: bool) -> Resp {
        let mut s = TcpStream::connect(self.addr).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        let mut head = match host {
            Some(h) => format!("{method} {path} HTTP/1.1\r\nHost: {h}\r\nConnection: close\r\n"),
            None => format!("{method} {path} HTTP/1.0\r\n"),
        };
        for (k, v) in headers {
            head.push_str(&format!("{k}: {v}\r\n"));
        }
        if let Some(b) = body {
            head.push_str(&format!("Content-Length: {}\r\n", b.len()));
        }
        head.push_str("\r\n");
        let _ = s.write_all(head.as_bytes());
        if let Some(b) = body {
            let _ = s.write_all(b);
        }
        let mut raw = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            match s.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => raw.extend_from_slice(&buf[..n]),
            }
            if stream && raw.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        let split = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap_or_else(|| panic!("no response head in {:?}", String::from_utf8_lossy(&raw)));
        let text = String::from_utf8_lossy(&raw[..split]).into_owned();
        let mut lines = text.lines();
        let status = lines.next().and_then(|l| l.split(' ').nth(1)).and_then(|c| c.parse().ok()).unwrap_or(0);
        let headers = lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect();
        Resp { status, headers, body: raw[split + 4..].to_vec() }
    }

    fn req(&self, route: &Route, host: &str, origin: Option<&str>, auth: bool) -> Resp {
        let mut h: Vec<(&str, &str)> = route.headers.to_vec();
        if let Some(o) = origin {
            h.push(("Origin", o));
        }
        let bearer = format!("Bearer {TOKEN}");
        if auth {
            h.push(("Authorization", &bearer));
        }
        self.call(Some(host), route.method, route.path, &h, route.body, route.stream)
    }
}

impl Drop for Srv {
    fn drop(&mut self) {
        if let Some(rt) = self.rt.take() {
            rt.shutdown_background();
        }
    }
}

struct Route {
    name: &'static str,
    method: &'static str,
    path: &'static str,
    headers: &'static [(&'static str, &'static str)],
    body: Option<&'static [u8]>,
    stream: bool,
    ui_only: bool,
}

const JSON: (&str, &str) = ("Content-Type", "application/json");

const ROUTES: [Route; 6] = [
    Route { name: "a command", method: "POST", path: "/daemon/commands/open-session", headers: &[JSON], body: Some(OPEN), stream: false, ui_only: false },
    Route { name: "a view", method: "GET", path: "/daemon/views/runs", headers: &[], body: None, stream: false, ui_only: false },
    Route { name: "an event stream", method: "GET", path: "/daemon/views/daemon-events", headers: &[("Accept", "text/event-stream")], body: None, stream: true, ui_only: false },
    Route { name: "/mcp", method: "POST", path: "/mcp", headers: &[JSON, ("Accept", "application/json, text/event-stream"), ("MCP-Protocol-Version", "2025-06-18")], body: Some(PING), stream: false, ui_only: false },
    Route { name: "a UI file", method: "GET", path: "/app.js", headers: &[], body: None, stream: false, ui_only: true },
    Route { name: "the login form", method: "POST", path: "/ui/login", headers: &[("Content-Type", "application/x-www-form-urlencoded")], body: Some(b"token=wrong"), stream: false, ui_only: true },
];

/// The status a request of `route` that the guard lets through gets: not 403 and not 415.
fn passes(r: &Resp) -> bool {
    r.status != 403 && r.status != 415
}

/// A foreign `Host` or `Origin` is 403 on every route, with a token and without one, before the
/// token is looked at; the same-origin request and the one without `Origin` pass.
fn host_and_origin(ui: bool, token: Option<&str>, tag: &str) {
    let s = Srv::start(tag, token, ui);
    let port = s.addr.port();
    let own = s.own();
    let rebound = format!("rebound.example:{port}");
    let rebound_origin = format!("http://{rebound}");
    for route in ROUTES.iter().filter(|r| ui || !r.ui_only) {
        let name = route.name;
        for auth in [false, true] {
            let ctx = format!("{name}, ui {ui}, token {}, bearer {auth}", token.is_some());
            // a Host that is not the listener's: 403 even when its Origin is its own
            for (host, origin) in [(rebound.as_str(), None), (rebound.as_str(), Some(rebound_origin.as_str()))] {
                let r = s.req(route, host, origin, auth);
                assert_eq!(r.status, 403, "{ctx}: Host {host} Origin {origin:?}: {}", r.text());
                assert!(!r.text().contains("rebound"), "{ctx}: the refusal names the host: {}", r.text());
            }
            // an Origin that is not the Host
            for origin in ["http://other-site.example", "null", "https://127.0.0.1", "http://localhost", "http://127.0.0.1.other-site.example", &format!("http://127.0.0.1:{}", port.wrapping_add(1)), &format!("http://{own}/path"), &format!("http://user@{own}")] {
                let r = s.req(route, &own, Some(origin), auth);
                assert_eq!(r.status, 403, "{ctx}: Origin {origin}: {}", r.text());
                assert!(!r.text().contains("other-site") && !r.text().contains("null"), "{ctx}: the refusal names the origin: {}", r.text());
            }
        }
        // the page's own origin, by either name of the listener, and no Origin at all
        let bearer = token.is_some();
        for host in [own.clone(), format!("localhost:{port}"), format!("[::1]:{port}")] {
            let origin = format!("http://{host}");
            for origin in [Some(origin.as_str()), None] {
                let r = s.req(route, &host, origin, bearer);
                assert!(passes(&r), "{name}: Host {host} Origin {origin:?} answered {}: {}", r.status, r.text());
                assert!(r.status < 500, "{name}: {}", r.status);
            }
        }
    }
}

#[test]
fn every_route_refuses_a_foreign_host_and_origin_with_ui_and_a_token() {
    host_and_origin(true, Some(TOKEN), "ug-ui-token");
}

#[test]
fn every_route_refuses_a_foreign_host_and_origin_with_ui_and_no_token() {
    host_and_origin(true, None, "ug-ui-open");
}

#[test]
fn every_route_refuses_a_foreign_host_and_origin_without_ui_with_a_token() {
    host_and_origin(false, Some(TOKEN), "ug-api-token");
}

#[test]
fn every_route_refuses_a_foreign_host_and_origin_without_ui_and_no_token() {
    host_and_origin(false, None, "ug-api-open");
}

#[test]
fn a_refusal_for_host_or_origin_has_one_body_whatever_was_sent() {
    let s = Srv::start("ug-body", Some(TOKEN), true);
    let own = s.own();
    let a = s.call(Some("a.example"), "GET", "/daemon/views/runs", &[], None, false);
    let b = s.call(Some("b.example:81"), "POST", "/mcp", &[JSON], Some(PING), false);
    let c = s.call(Some(&own), "GET", "/app.js", &[("Origin", "http://c.example")], None, false);
    let d = s.call(None, "GET", "/daemon/views/runs", &[], None, false);
    for r in [&a, &b, &c, &d] {
        assert_eq!(r.status, 403, "{}", r.text());
        assert_eq!(r.body, a.body);
        assert_eq!(r.header("content-type"), Some("application/json"));
    }
    let v = a.json();
    assert_eq!(v["refused"], "forbidden");
    assert!(v["message"].is_string());
}

#[test]
fn on_loopback_the_host_is_one_of_the_names_of_the_listener_with_its_port() {
    let s = Srv::start("ug-host-forms", None, false);
    let port = s.addr.port();
    let ok = [format!("127.0.0.1:{port}"), format!("localhost:{port}"), format!("LOCALHOST:{port}"), format!("[::1]:{port}")];
    for host in &ok {
        assert_eq!(s.call(Some(host), "GET", "/daemon/views/runs", &[], None, false).status, 200, "{host}");
    }
    let wrong_port = port.wrapping_add(1);
    let bad = [
        "127.0.0.1".to_string(),
        "localhost".to_string(),
        format!("127.0.0.1:{wrong_port}"),
        format!("localhost:{wrong_port}"),
        format!("127.0.0.2:{port}"),
        format!("0.0.0.0:{port}"),
        format!("localhost.:{port}"),
        format!("localhost.example:{port}"),
        format!("evil.example:{port}"),
        format!("127.0.0.1:{port}@evil.example"),
        format!("user@127.0.0.1:{port}"),
        format!("127.0.0.1:{port}:{port}"),
        format!("127.0.0.1:{port}/"),
        format!("[::1]:{wrong_port}"),
        String::new(),
        " ".to_string(),
    ];
    for host in &bad {
        let r = s.call(Some(host), "GET", "/daemon/views/runs", &[], None, false);
        assert_eq!(r.status, 403, "Host {host:?}: {}", r.text());
    }
    assert_eq!(s.call(None, "GET", "/daemon/views/runs", &[], None, false).status, 403, "no Host at all");
}

#[test]
fn on_a_listener_that_is_not_loopback_there_is_no_host_check_and_the_origin_still_must_be_the_host() {
    let s = Srv::on("0.0.0.0:0", "ug-wide", Some(TOKEN), false);
    let bearer = format!("Bearer {TOKEN}");
    let h = |origin: Option<&'static str>| {
        let mut v = vec![("Authorization", bearer.as_str())];
        if let Some(o) = origin {
            v.push(("Origin", o));
        }
        s.call(Some("daemon.internal.example:7430"), "GET", "/daemon/views/runs", &v, None, false).status
    };
    assert_eq!(h(None), 200, "a Host of the machine's own name is fine behind a token");
    assert_eq!(h(Some("http://daemon.internal.example:7430")), 200);
    assert_eq!(h(Some("http://other-site.example")), 403);
    assert_eq!(h(Some("null")), 403);
    assert_eq!(s.call(Some("daemon.internal.example:7430"), "GET", "/daemon/views/runs", &[], None, false).status, 401, "the token rule is as before");
}

#[test]
fn commands_and_mcp_take_application_json_only() {
    let s = Srv::start("ug-415", Some(TOKEN), false);
    let own = s.own();
    let bearer = format!("Bearer {TOKEN}");
    for (path, body) in [("/daemon/commands/open-session", OPEN), ("/mcp", PING)] {
        let send = |content_type: Option<&str>| {
            let mut h = vec![("Authorization", bearer.as_str()), ("Accept", "application/json, text/event-stream"), ("MCP-Protocol-Version", "2025-06-18")];
            if let Some(c) = content_type {
                h.push(("Content-Type", c));
            }
            s.call(Some(&own), "POST", path, &h, Some(body), false)
        };
        for bad in [None, Some("text/plain"), Some("text/plain;charset=UTF-8"), Some("application/x-www-form-urlencoded"), Some("multipart/form-data; boundary=x"), Some("application/jsonx"), Some("application/json-seq"), Some("text/json"), Some("")] {
            let r = send(bad);
            assert_eq!(r.status, 415, "{path} with Content-Type {bad:?}: {}", r.text());
            assert!(!r.text().contains("text/plain"), "{}", r.text());
        }
        for good in ["application/json", "application/json; charset=utf-8", "Application/JSON", " application/json "] {
            let r = send(Some(good));
            assert!(r.status != 415 && r.status != 403, "{path} with Content-Type {good:?} answered {}: {}", r.status, r.text());
        }
    }
    // the token is asked first: a body of the wrong type without it is 401, and 415 does not tell a stranger anything
    let r = s.call(Some(&own), "POST", "/daemon/commands/open-session", &[("Content-Type", "text/plain")], Some(OPEN), false);
    assert_eq!(r.status, 401);
    // a read of the MCP path is no command: 405, not 415
    let r = s.call(Some(&own), "GET", "/mcp", &[("Authorization", &bearer)], None, false);
    assert_eq!(r.status, 405);
    // and a refused command had no effect
    let sessions = s.call(Some(&own), "GET", "/daemon/views/sessions", &[("Authorization", &bearer)], None, false).json();
    assert_eq!(sessions["rows"].as_array().unwrap().len(), 1, "only the commands of a right type opened the one session: {sessions}");
}

#[test]
fn a_cross_site_text_plain_post_has_no_effect_with_or_without_the_ui() {
    for (ui, tag) in [(false, "ug-csrf-api"), (true, "ug-csrf-ui")] {
        let s = Srv::start(tag, None, ui);
        let own = s.own();
        let r = s.call(Some(&own), "POST", "/daemon/commands/open-session", &[("Origin", "http://other-site.example"), ("Content-Type", "text/plain;charset=UTF-8")], Some(OPEN), false);
        assert_eq!(r.status, 403, "ui {ui}: {}", r.text());
        // the same body without an Origin, from a client that is not a page, is refused for its type
        let r = s.call(Some(&own), "POST", "/daemon/commands/open-session", &[("Content-Type", "text/plain;charset=UTF-8")], Some(OPEN), false);
        assert_eq!(r.status, 415, "ui {ui}: {}", r.text());
        let sessions = s.call(Some(&own), "GET", "/daemon/views/sessions", &[], None, false).json();
        assert!(sessions["rows"].as_array().unwrap().is_empty(), "ui {ui}: {sessions}");
    }
}

#[test]
fn the_cli_client_and_the_mcp_client_send_what_the_guard_asks_for() {
    // the clients send `Content-Type: application/json` with a body and no `Origin`: their own suites
    // (cli_client.rs, mcp.rs) run against a daemon that has this guard; here the head they send is read
    for file in ["src/client/http.rs", "src/mcp/remote.rs"] {
        let text = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(file)).unwrap();
        assert!(text.contains("Content-Type: application/json"), "{file} does not send the content type of a command");
        assert!(!text.contains("Origin:"), "{file} sends an Origin");
    }
}

// ---- the build refuses a link in ui/web/ ----

fn build_script(fx: &Fx) -> std::path::PathBuf {
    let out = fx.dir.join("build-script");
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("build.rs");
    let status = Command::new("rustc").args(["--edition", "2021", "-C", "opt-level=0", "-C", "debuginfo=0", "-o"]).arg(&out).arg(&source).env_remove("RUSTC_WRAPPER").status().expect("rustc runs");
    assert!(status.success(), "build.rs does not compile on its own");
    out
}

/// Run `build.rs` over `<dir>/ui/web` and say whether it succeeded, with what it printed to stderr.
fn run_build(fx: &Fx, script: &std::path::Path, name: &str) -> (bool, String) {
    let manifest = fx.dir.join(name).join("runbook");
    let out_dir = fx.dir.join(name).join("out");
    std::fs::create_dir_all(&manifest).unwrap();
    std::fs::create_dir_all(&out_dir).unwrap();
    // the script also reads the Loom commit from the lock file next to the manifest
    std::fs::copy(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.lock"), manifest.join("Cargo.lock")).unwrap();
    let r = Command::new(script).env("CARGO_MANIFEST_DIR", &manifest).env("OUT_DIR", &out_dir).output().unwrap();
    (r.status.success(), String::from_utf8_lossy(&r.stderr).into_owned())
}

#[test]
fn the_build_refuses_a_symbolic_link_in_ui_web_to_a_file_or_a_directory_and_names_it() {
    use std::os::unix::fs::symlink;
    let fx = Fx::new("ug-build-links");
    let script = build_script(&fx);
    // a tree without a link builds
    let web = |name: &str| {
        let w = fx.dir.join(name).join("ui/web");
        std::fs::create_dir_all(w.join("fonts")).unwrap();
        std::fs::write(w.join("index.html"), "<html></html>").unwrap();
        std::fs::write(w.join("fonts/a.woff2"), "x").unwrap();
        w
    };
    web("plain");
    let (ok, err) = run_build(&fx, &script, "plain");
    assert!(ok, "a tree of plain files and directories fails the build: {err}");
    // a link to a file outside the directory
    let outside = fx.dir.join("secret.txt");
    std::fs::write(&outside, "secret").unwrap();
    let w = web("file-link");
    symlink(&outside, w.join("leak.txt")).unwrap();
    let (ok, err) = run_build(&fx, &script, "file-link");
    assert!(!ok && err.contains("leak.txt") && err.contains("symbolic link"), "a link to a file passed or is not named: {err}");
    // a link to a file inside the directory
    let w = web("inner-link");
    symlink(w.join("index.html"), w.join("again.html")).unwrap();
    let (ok, err) = run_build(&fx, &script, "inner-link");
    assert!(!ok && err.contains("again.html"), "{err}");
    // a link to a directory, one level down
    let w = web("dir-link");
    symlink(&fx.dir, w.join("fonts/up")).unwrap();
    let (ok, err) = run_build(&fx, &script, "dir-link");
    assert!(!ok && err.contains("up") && err.contains("symbolic link"), "a link to a directory passed or is not named: {err}");
    // a link that points nowhere
    let w = web("dangling");
    symlink(fx.dir.join("nothing"), w.join("gone.js")).unwrap();
    let (ok, err) = run_build(&fx, &script, "dangling");
    assert!(!ok && err.contains("gone.js"), "{err}");
}
