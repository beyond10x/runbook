//! Conformance checks of the embedded UI and its browser login against the contract of
//! `docs/daemon-api.md`, "The web UI", and the security floor of `src/api/auth.rs`: what the cookie
//! holds, what a page of another origin can do to a daemon on loopback, the methods of the login
//! route, and the path, header and size rules the first suite does not measure.

use super::commands::Fx;
use super::http::Resp;
use runbook::api;
use runbook::ui_embed;
use serde_json::Value;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

const TOKEN: &str = "t0ken-for-the-review";
const COMMANDS: [&str; 5] = ["open-session", "close-session", "submit-run", "cancel-run", "answer-decision"];
const OPEN: &[u8] = br#"{"session_id":"review","client":"a page"}"#;

struct Srv {
    _fx: Fx,
    addr: SocketAddr,
    rt: Option<tokio::runtime::Runtime>,
}

impl Srv {
    fn start(tag: &str, token: Option<&str>, ui: bool) -> Srv {
        let fx = Fx::new(tag);
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        let listener = rt.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).unwrap();
        let addr = listener.local_addr().unwrap();
        let (daemon, token) = (fx.d().clone(), token.map(String::from));
        rt.spawn(async move {
            let _ = api::serve_ui(listener, daemon, token.as_deref(), ui, std::future::pending()).await;
        });
        Srv { _fx: fx, addr, rt: Some(rt) }
    }

    /// The `Host` a browser sends for this listener.
    fn host(&self) -> String {
        format!("127.0.0.1:{}", self.addr.port())
    }

    /// One request with the listener's own `Host`.
    fn req(&self, method: &str, path: &str, headers: &[(&str, &str)], body: Option<&[u8]>) -> Resp {
        self.req_host(&self.host(), method, path, headers, body)
    }

    /// One HTTP/1.1 request on a fresh connection with the `Host` given, read to the end. A
    /// connection the server closed without an answer has status 0.
    fn req_host(&self, host: &str, method: &str, path: &str, headers: &[(&str, &str)], body: Option<&[u8]>) -> Resp {
        let mut s = TcpStream::connect(self.addr).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        let mut head = format!("{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n");
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
        let _ = s.read_to_end(&mut raw);
        let Some(split) = raw.windows(4).position(|w| w == b"\r\n\r\n") else {
            return Resp { status: 0, headers: Vec::new(), body: raw };
        };
        let text = String::from_utf8_lossy(&raw[..split]).into_owned();
        let mut lines = text.lines();
        let status = lines.next().and_then(|l| l.split(' ').nth(1)).and_then(|c| c.parse().ok()).unwrap_or(0);
        let headers = lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect();
        Resp { status, headers, body: raw[split + 4..].to_vec() }
    }

    fn login(&self, body: &[u8], extra: &[(&str, &str)]) -> Resp {
        let mut h = vec![("Content-Type", "application/x-www-form-urlencoded")];
        h.extend_from_slice(extra);
        self.req("POST", "/ui/login", &h, Some(body))
    }

    /// The `name=value` pair of the cookie a right token is answered with.
    fn cookie(&self) -> String {
        let r = self.login(format!("token={TOKEN}").as_bytes(), &[]);
        assert_eq!(r.status, 303, "{}", r.text());
        r.header("set-cookie").expect("a cookie").split(';').next().unwrap().to_string()
    }

    fn sessions(&self, headers: &[(&str, &str)]) -> Vec<Value> {
        let r = self.req("GET", "/daemon/views/sessions", headers, None);
        assert_eq!(r.status, 200, "{}", r.text());
        r.json()["rows"].as_array().unwrap().clone()
    }
}

impl Drop for Srv {
    fn drop(&mut self) {
        if let Some(rt) = self.rt.take() {
            rt.shutdown_background();
        }
    }
}

fn unhex(s: &str) -> Option<String> {
    if !s.len().is_multiple_of(2) || !s.is_ascii() {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()).collect();
    String::from_utf8(bytes?).ok()
}

// ---- what the cookie holds ----

/// The contract: "a cookie never authorizes a command". A browser sends a cookie to every port of
/// the host that set it, so whatever the cookie holds is shown to every other server on that host;
/// what it holds must not be, or decode to, a value the daemon takes as a bearer token on a command.
#[test]
fn what_the_cookie_holds_is_not_a_bearer_token_for_a_command() {
    let s = Srv::start("uir-cookie-value", Some(TOKEN), true);
    let pair = s.cookie();
    let value = pair.split_once('=').unwrap().1.to_string();
    let mut candidates = vec![("the value of the cookie", value.clone())];
    if let Some(decoded) = unhex(&value) {
        candidates.push(("the value of the cookie read as hex", decoded));
    }
    for (what, candidate) in candidates {
        let r = s.req("POST", "/daemon/commands/open-session", &[("Authorization", &format!("Bearer {candidate}")), ("Content-Type", "application/json")], Some(OPEN));
        assert_eq!(r.status, 401, "{what} is taken as the bearer token of a command: {}", r.text());
    }
}

// ---- a page of another origin and a daemon on loopback without a token ----

/// A page of another site posts a body a browser sends without asking first (`text/plain`, no
/// preflight). The command must have no effect.
fn a_cross_origin_post_has_no_effect(tag: &str, ui: bool) {
    let s = Srv::start(tag, None, ui);
    let r = s.req("POST", "/daemon/commands/open-session", &[("Origin", "http://other-site.example"), ("Content-Type", "text/plain;charset=UTF-8")], Some(OPEN));
    let sessions = s.sessions(&[]);
    assert!(sessions.is_empty(), "a page of http://other-site.example opened a session on a daemon without a token (answer {}): {sessions:?}", r.status);
}

#[test]
fn with_ui_a_daemon_without_a_token_takes_no_command_from_a_page_of_another_origin() {
    a_cross_origin_post_has_no_effect("uir-csrf-ui", true);
}

#[test]
fn without_ui_a_daemon_without_a_token_takes_no_command_from_a_page_of_another_origin() {
    a_cross_origin_post_has_no_effect("uir-csrf-api", false);
}

/// DNS rebinding: a page of `rebound.example` whose name now resolves to 127.0.0.1 is of the same
/// origin as the daemon for the browser, so it reads every answer and sends any body; only the
/// `Host` header tells the daemon that the request was not meant for its listen address.
fn a_host_that_is_not_the_listen_address_is_refused(tag: &str, ui: bool) {
    let s = Srv::start(tag, None, ui);
    let host = format!("rebound.example:{}", s.addr.port());
    let origin = format!("http://{host}");
    let read = s.req_host(&host, "GET", "/daemon/views/runs", &[], None);
    let write = s.req_host(&host, "POST", "/daemon/commands/open-session", &[("Origin", &origin), ("Content-Type", "application/json")], Some(OPEN));
    let sessions = s.sessions(&[]);
    assert!(sessions.is_empty(), "a request for Host {host} opened a session on a daemon without a token (answer {}): {sessions:?}", write.status);
    assert_ne!(read.status, 200, "a request for Host {host} read a view of a daemon that listens on {}", s.host());
}

#[test]
fn with_ui_a_daemon_without_a_token_refuses_a_host_that_is_not_its_listen_address() {
    a_host_that_is_not_the_listen_address_is_refused("uir-rebind-ui", true);
}

#[test]
fn without_ui_a_daemon_without_a_token_refuses_a_host_that_is_not_its_listen_address() {
    a_host_that_is_not_the_listen_address_is_refused("uir-rebind-api", false);
}

// ---- the login route ----

/// The contract of the token: "every route, an unknown one included, answers 401" without it. The
/// login is outside the middleware for the one method that hands the token over.
#[test]
fn with_a_token_the_login_route_answers_401_to_every_method_but_its_post() {
    let s = Srv::start("uir-login-methods", Some(TOKEN), true);
    for method in ["GET", "HEAD", "PUT", "DELETE", "PATCH", "OPTIONS"] {
        let r = s.req(method, "/ui/login", &[], None);
        assert_eq!(r.status, 401, "{method} /ui/login without the token");
    }
}

#[test]
fn the_login_takes_a_form_of_its_own_origin_only_and_says_nothing_of_the_token() {
    let s = Srv::start("uir-login-origin", Some(TOKEN), true);
    let right = format!("token={TOKEN}");
    let own = format!("http://{}", s.host());
    // the form of the daemon's own page, as a browser sends it
    let good = s.login(right.as_bytes(), &[("Origin", &own)]);
    assert_eq!(good.status, 303, "{}", good.text());
    assert_eq!(good.header("location"), Some("/"));
    assert!(good.body.is_empty());
    let cookie = good.header("set-cookie").expect("a cookie").to_string();
    let attributes: Vec<String> = cookie.split(';').skip(1).map(|a| a.trim().to_ascii_lowercase()).collect();
    assert!(attributes.contains(&"httponly".to_string()) && attributes.contains(&"samesite=strict".to_string()) && attributes.contains(&"path=/".to_string()), "{cookie}");
    assert!(!attributes.iter().any(|a| a.starts_with("domain")), "the cookie is for this host only: {cookie}");
    // any other origin, with the right token
    let other_port = format!("http://127.0.0.1:{}", if s.addr.port() == 1 { 2 } else { 1 });
    for origin in ["null", "http://other-site.example", "https://127.0.0.1", "http://localhost", other_port.as_str(), "http://127.0.0.1.other-site.example"] {
        let r = s.login(right.as_bytes(), &[("Origin", origin)]);
        assert_eq!(r.status, 403, "Origin: {origin}");
        assert!(r.header("set-cookie").is_none(), "Origin: {origin}");
        assert!(!r.text().contains(TOKEN));
    }
    // the token in the URL is not read
    let in_url = s.req("POST", &format!("/ui/login?token={TOKEN}"), &[("Content-Type", "application/x-www-form-urlencoded")], Some(b""));
    assert_eq!(in_url.status, 401);
    assert!(in_url.header("set-cookie").is_none());
    // a wrong token: the same answer whatever it shares with the right one, and no word of the right one
    let (shorter, longer) = (TOKEN[..TOKEN.len() - 1].to_string(), format!("{TOKEN}x"));
    let answers: Vec<Resp> = ["x", shorter.as_str(), longer.as_str(), ""].iter().map(|t| s.login(format!("token={t}").as_bytes(), &[])).collect();
    for r in &answers {
        assert_eq!(r.status, 401);
        assert_eq!(r.body, answers[0].body);
        assert!(r.header("set-cookie").is_none() && r.header("location").is_none());
        assert!(!r.text().contains(TOKEN));
    }
    // no answer of the login opens a read to another origin
    for r in answers.iter().chain([&good]) {
        assert!(r.headers.iter().all(|(k, _)| !k.to_ascii_lowercase().starts_with("access-control-")), "{:?}", r.headers);
    }
}

#[test]
fn the_login_reads_4096_bytes_and_no_more() {
    let s = Srv::start("uir-login-size", Some(TOKEN), true);
    let body = |n: usize| format!("token={}", "a".repeat(n - "token=".len()));
    assert_eq!(s.login(body(4096).as_bytes(), &[]).status, 401);
    let over = s.login(body(4097).as_bytes(), &[]);
    assert_eq!(over.status, 413, "{}", over.text());
    assert!(over.header("set-cookie").is_none());
    // the right token behind padding that passes the limit is not read either
    let padded = format!("pad={}&token={TOKEN}", "a".repeat(4096));
    assert_eq!(s.login(padded.as_bytes(), &[]).status, 413);
}

#[test]
fn the_401_page_carries_a_policy_and_no_script_and_exists_with_ui_only() {
    let s = Srv::start("uir-401-page", Some(TOKEN), true);
    let first = s.req("GET", "/", &[("Accept", "text/html")], None);
    let refused = s.login(b"token=wrong", &[]);
    for page in [&first, &refused] {
        assert_eq!(page.status, 401);
        assert_eq!(page.header("content-type"), Some("text/html; charset=utf-8"));
        assert_eq!(page.header("x-content-type-options"), Some("nosniff"));
        assert_eq!(page.header("cache-control"), Some("no-store"));
        let csp = page.header("content-security-policy").expect("a policy on the 401 page");
        assert!(csp.contains("default-src 'none'") && csp.contains("form-action 'self'") && csp.contains("frame-ancestors 'none'"), "{csp}");
        assert!(!csp.contains("unsafe"), "{csp}");
        let text = page.text().to_ascii_lowercase();
        assert!(!text.contains("<script") && !text.contains("javascript:") && !text.contains("<style") && !text.contains("style="), "{text}");
        assert!(!text.split_whitespace().any(|word| word.starts_with("on") && word.contains('=')), "an event attribute in {text}");
        assert!(text.contains("method=\"post\"") && !text.contains("method=\"get\""), "{text}");
    }
    // the page is the answer of the navigation to / only: a file, a view and a stream keep the JSON refusal
    for path in ["/app.js", "/index.html", "/daemon/views/runs", "/nothing"] {
        let r = s.req("GET", path, &[("Accept", "text/html")], None);
        assert_eq!((r.status, r.header("content-type")), (401, Some("application/json")), "{path}");
    }
    // without --ui there is no such page and no login
    let plain = Srv::start("uir-401-plain", Some(TOKEN), false);
    let r = plain.req("GET", "/", &[("Accept", "text/html")], None);
    assert_eq!((r.status, r.header("content-type")), (401, Some("application/json")));
    let open = Srv::start("uir-401-open", None, false);
    assert_eq!(open.login(format!("token={TOKEN}").as_bytes(), &[]).status, 404);
    assert_eq!(open.req("GET", "/", &[("Accept", "text/html")], None).status, 404);
}

// ---- the cookie is for reads ----

#[test]
fn a_cookie_authorizes_get_and_head_and_nothing_else_on_any_route() {
    let s = Srv::start("uir-cookie-methods", Some(TOKEN), true);
    let pair = s.cookie();
    let cookie = ("Cookie", pair.as_str());
    let json = ("Content-Type", "application/json");
    for command in COMMANDS {
        let path = format!("/daemon/commands/{command}");
        for method in ["POST", "PUT", "PATCH", "DELETE", "OPTIONS"] {
            for extra in [("X-Review", "1"), ("X-HTTP-Method-Override", "GET"), ("X-HTTP-Method", "GET"), ("X-Method-Override", "GET")] {
                let r = s.req(method, &path, &[cookie, json, extra], Some(OPEN));
                assert_eq!(r.status, 401, "{method} {path} with the cookie and {}", extra.0);
            }
            let r = s.req(method, &format!("{path}?_method=GET"), &[cookie, json], Some(OPEN));
            assert_eq!(r.status, 401, "{method} {path}?_method=GET with the cookie");
        }
        // a read of a command's route is admitted by the cookie and is no command
        for method in ["GET", "HEAD"] {
            for extra in [("X-HTTP-Method-Override", "POST"), ("X-HTTP-Method", "POST"), ("X-Method-Override", "POST")] {
                let r = s.req(method, &format!("{path}?_method=POST"), &[cookie, json, extra], Some(OPEN));
                assert_eq!(r.status, 405, "{method} {path} with the cookie and {}", extra.0);
            }
        }
    }
    // (the methods of /ui/login have a case of their own above)
    for path in ["/", "/app.js", "/nothing", "/daemon/views/runs"] {
        for method in ["POST", "PUT", "PATCH", "DELETE", "OPTIONS"] {
            let r = s.req(method, path, &[cookie], Some(b""));
            assert_eq!(r.status, 401, "{method} {path} with the cookie");
        }
    }
    let bearer = format!("Bearer {TOKEN}");
    assert!(s.sessions(&[("Authorization", &bearer)]).is_empty(), "no request with the cookie was a command");
    // the reads
    assert_eq!(s.req("HEAD", "/daemon/views/runs", &[cookie], None).status, 200);
    assert_eq!(s.req("GET", "/daemon/views/runs", &[cookie], None).status, 200);
    // a header that is wrong is not rescued by a cookie that is right
    assert_eq!(s.req("GET", "/daemon/views/runs", &[cookie, ("Authorization", "Bearer wrong")], None).status, 401);
    // a cookie of another name, of no value, of a value that is no hex, and the token itself
    let (plain, odd) = (format!("runbook_ui={TOKEN}"), format!("{pair}x"));
    for bad in ["runbook_ui=", "runbook_ui=zz", "runbook_uix=00", "other=1", plain.as_str(), odd.as_str()] {
        assert_eq!(s.req("GET", "/daemon/views/runs", &[("Cookie", bad)], None).status, 401, "{bad}");
    }
}

// ---- the files ----

#[test]
fn no_form_of_a_path_reaches_anything_but_an_embedded_file() {
    let s = Srv::start("uir-paths", None, true);
    let long = format!("/{}", "a".repeat(6000));
    let long_up = format!("/{}Cargo.toml", "../".repeat(1500));
    for path in [
        "/%252e%252e/Cargo.toml",
        "/%252e%252e%252fCargo.toml",
        "/..%5cCargo.toml",
        "/..\\Cargo.toml",
        "/fonts\\inter-variable-latin.woff2",
        "/fonts%5cinter-variable-latin.woff2",
        "/fonts%2finter-variable-latin.woff2",
        "/etc/passwd",
        "//etc/passwd",
        "/%2fetc%2fpasswd",
        "/file:///etc/passwd",
        "/app.js%00.html",
        "/%00",
        "/index.html%00",
        "/index.html/..",
        "/index.html/.",
        "/app.js;x",
        "/.git/config",
        "/.gitignore",
        "/runs/",
        "/../runs/",
        "/build.rs",
        "/../runbook/Cargo.toml",
        long.as_str(),
        long_up.as_str(),
    ] {
        let r = s.req("GET", path, &[], None);
        let shown: String = path.chars().take(60).collect();
        assert!([400, 404, 414, 431].contains(&r.status), "{shown} answered {}", r.status);
        assert!(!r.text().contains("[package]") && !r.text().contains("root:"), "{shown}");
    }
    // an absolute form of the target names the same list
    let absolute = format!("http://{}/../Cargo.toml", s.host());
    assert_eq!(s.req("GET", &absolute, &[], None).status, 404);
}

#[test]
fn the_embedded_list_holds_no_dot_file_and_no_path_out_of_the_directory_and_every_file_says_nosniff() {
    let s = Srv::start("uir-list", None, true);
    for path in ui_embed::paths() {
        assert!(!path.starts_with('/') && !path.contains('\\') && !path.contains('\0'), "{path}");
        for part in path.split('/') {
            assert!(!part.is_empty() && !part.starts_with('.'), "{path}");
        }
        assert!(!path.starts_with("runs/") && !path.contains("node_modules"), "{path}");
        let r = s.req("GET", &format!("/{path}"), &[], None);
        assert_eq!(r.status, 200, "{path}");
        assert_eq!(r.header("x-content-type-options"), Some("nosniff"), "{path}");
        assert!(r.header("content-security-policy").is_some(), "{path}");
        assert!(r.headers.iter().all(|(k, _)| !k.to_ascii_lowercase().starts_with("access-control-")), "{path}: {:?}", r.headers);
        let kind = r.header("content-type").unwrap_or("");
        if !path.ends_with(".html") {
            assert!(!kind.starts_with("text/html"), "{path} is served as {kind}");
        }
    }
    // no answer of the API opens a read to another origin, asked for or not
    for (method, path) in [("GET", "/daemon/views/runs"), ("OPTIONS", "/daemon/views/runs"), ("OPTIONS", "/daemon/commands/submit-run")] {
        let r = s.req(method, path, &[("Origin", "http://other-site.example"), ("Access-Control-Request-Method", "POST"), ("Access-Control-Request-Headers", "content-type")], None);
        assert!(r.headers.iter().all(|(k, _)| !k.to_ascii_lowercase().starts_with("access-control-")), "{method} {path}: {:?}", r.headers);
    }
}
