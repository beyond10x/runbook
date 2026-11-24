//! Security review, pass 1, of the unit `daemon-page-starts-a-run`: conformance cases for the view
//! `runbooks` and the start command as the page sends it, against the daemon served in this process
//! with the UI, a token and the address of its listener (so the Host check of a loopback listener is on).
//! Invented names and texts only; no model call, no network beyond loopback.

use super::http::Resp;
use super::state::scratch;
use runbook::api::{self, auth};
use runbook::commands::{Config, Daemon};
use runbook::engine::Engine;
use runbook::runtime::{Confinement, Runtime};
use runbook::stub::{Script, Step, StubRuntime};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const TOKEN: &str = "security-start-0123456789-abcdefghijkl";
const JSON: (&str, &str) = ("Content-Type", "application/json");

struct Srv {
    addr: SocketAddr,
    dir: PathBuf,
    rt: Option<tokio::runtime::Runtime>,
    _daemon: Daemon,
}

impl Drop for Srv {
    fn drop(&mut self) {
        if let Some(rt) = self.rt.take() {
            rt.shutdown_background();
        }
    }
}

fn quick() -> Script {
    Script::new(vec![Step::End("done".into())])
}

fn stub() -> Arc<StubRuntime> {
    let stub = Arc::new(StubRuntime::new());
    stub.script("known-book", quick());
    stub
}

fn serve(tag: &str, runtime: Arc<dyn Runtime>, floor: Confinement, runbook_dir: Option<PathBuf>, token: Option<&str>, dir: PathBuf) -> Srv {
    let cfg = Config { state_dir: dir.join("state"), confinement_floor: floor, context_roots: vec![], runbook_dir, principal: super::PRINCIPAL.into() };
    let daemon = Daemon::start(cfg, runtime).unwrap_or_else(|e| panic!("{tag}: {e}"));
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let listener = rt.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).unwrap();
    let addr = listener.local_addr().unwrap();
    let (served, token) = (daemon.clone(), token.map(String::from));
    rt.spawn(async move {
        let _ = api::serve_all(listener, served, token.as_deref(), api::Streams::default(), true, None, std::future::pending()).await;
    });
    Srv { addr, dir, rt: Some(rt), _daemon: daemon }
}

impl Srv {
    fn own(&self) -> String {
        format!("127.0.0.1:{}", self.addr.port())
    }

    /// One request with the `Host` given, read to the end.
    fn call(&self, host: &str, method: &str, path: &str, headers: &[(&str, &str)], body: Option<&[u8]>) -> Resp {
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
        let split = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap_or_else(|| panic!("no response head in {:?}", String::from_utf8_lossy(&raw)));
        let text = String::from_utf8_lossy(&raw[..split]).into_owned();
        let mut lines = text.lines();
        let status = lines.next().and_then(|l| l.split(' ').nth(1)).and_then(|c| c.parse().ok()).unwrap_or(0);
        let headers: Vec<(String, String)> = lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect();
        let mut body = raw[split + 4..].to_vec();
        if headers.iter().any(|(k, v)| k.eq_ignore_ascii_case("transfer-encoding") && v.contains("chunked")) {
            body = dechunk(&body);
        }
        Resp { status, headers, body }
    }

    fn get(&self, path: &str, headers: &[(&str, &str)]) -> Resp {
        self.call(&self.own(), "GET", path, headers, None)
    }

    fn authed(&self, path: &str) -> Resp {
        self.get(path, &[("Authorization", &format!("Bearer {TOKEN}"))])
    }

    fn rows(&self, path: &str) -> Vec<Value> {
        let r = self.authed(path);
        assert_eq!(r.status, 200, "{path}: {}", r.text());
        r.json()["rows"].as_array().unwrap().clone()
    }

    /// `submit-run` with the token, the body given.
    fn submit(&self, body: &str) -> Resp {
        self.call(&self.own(), "POST", "/daemon/commands/submit-run", &[JSON, ("Authorization", &format!("Bearer {TOKEN}"))], Some(body.as_bytes()))
    }
}

fn dechunk(mut rest: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(eol) = rest.windows(2).position(|w| w == b"\r\n") {
        let n = usize::from_str_radix(std::str::from_utf8(&rest[..eol]).unwrap_or("0").trim(), 16).unwrap_or(0);
        if n == 0 || rest.len() < eol + 2 + n {
            break;
        }
        out.extend_from_slice(&rest[eol + 2..eol + 2 + n]);
        rest = &rest[(eol + 4 + n).min(rest.len())..];
    }
    out
}

/// What a client can tell two answers apart by: the status, the body and every header but the date.
fn shape(r: &Resp) -> (u16, String, BTreeMap<String, String>) {
    let headers = r.headers.iter().filter(|(k, _)| !k.eq_ignore_ascii_case("date")).map(|(k, v)| (k.to_ascii_lowercase(), v.clone())).collect();
    (r.status, r.text(), headers)
}

/// Every file under `dir`, by its path below it, with its bytes.
fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, at: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(at).into_iter().flatten().flatten() {
            let p = e.path();
            let rel = p.strip_prefix(root).unwrap().to_string_lossy().into_owned();
            if p.is_dir() {
                out.insert(format!("{rel}/"), Vec::new());
                walk(root, &p, out);
            } else {
                out.insert(rel, std::fs::read(&p).unwrap_or_default());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn books(dir: &Path) -> PathBuf {
    let books = dir.join("books");
    std::fs::create_dir_all(books.join("dir-book")).unwrap();
    books
}

// ---- invariant 1: one token check, and the Host and Origin guard, on the new view and the start ----

#[test]
fn without_the_token_no_answer_tells_whether_a_runbook_exists() {
    let dir = scratch("sec-start-token");
    let s = serve("token", stub(), Confinement::Linux, Some(books(&dir)), Some(TOKEN), dir);
    let own = s.own();
    let cookie = format!("{}={}", runbook::ui_embed::COOKIE, auth::cookie_value(TOKEN));
    let raw_cookie = format!("{}={TOKEN}", runbook::ui_embed::COOKIE);
    let basic = "Basic OnNlY3VyaXR5LXN0YXJ0LTAxMjM0NTY3ODktYWJjZGVmZ2hpamts";

    // the list: every way of asking that is not the token (or, for a read, the cookie of the login)
    let unknown = s.get("/daemon/views/no-such-view", &[]);
    assert_eq!(unknown.status, 401, "{}", unknown.text());
    let refusals: Vec<(&str, Resp)> = vec![
        ("no credentials", s.get("/daemon/views/runbooks", &[])),
        ("a wrong bearer", s.get("/daemon/views/runbooks", &[("Authorization", "Bearer not-the-token-of-this-daemon-0123456789")])),
        ("an empty bearer", s.get("/daemon/views/runbooks", &[("Authorization", "Bearer ")])),
        ("the token as Basic", s.get("/daemon/views/runbooks", &[("Authorization", basic)])),
        ("the token in the query", s.get(&format!("/daemon/views/runbooks?token={TOKEN}&access_token={TOKEN}"), &[])),
        ("the token itself as the cookie", s.get("/daemon/views/runbooks", &[("Cookie", &raw_cookie)])),
        ("the token in another header", s.get("/daemon/views/runbooks", &[("X-Token", TOKEN), ("Proxy-Authorization", &format!("Bearer {TOKEN}"))])),
        ("HEAD", s.call(&own, "HEAD", "/daemon/views/runbooks", &[], None)),
    ];
    for (what, r) in &refusals {
        assert_eq!(r.status, 401, "{what}: {}", r.text());
        assert!(!r.text().contains("known-book") && !r.text().contains("dir-book"), "{what}: the refusal names a runbook: {}", r.text());
        if *what != "HEAD" {
            assert_eq!(r.text(), unknown.text(), "{what}: the refusal differs from the one of a route that does not exist");
        }
    }
    // the cookie of the login is the documented way a page reads; the token opens the list too
    let read = s.get("/daemon/views/runbooks", &[("Cookie", &cookie)]);
    assert_eq!(read.status, 200, "{}", read.text());
    let names: Vec<String> = s.rows("/daemon/views/runbooks").iter().map(|r| r["runbook_id"].as_str().unwrap().to_string()).collect();
    assert_eq!(names, ["dir-book", "known-book"]);

    // the start: a runbook that exists and one that does not are refused alike without the token, with
    // a wrong one and with the read cookie, and nothing is submitted
    let body = |name: &str| json!({"runbook_id": name, "intake": "an invented intake", "confinement": "linux", "idempotency_key": format!("sec-{name}")}).to_string();
    let wrong = ("Authorization", "Bearer not-the-token-of-this-daemon-0123456789");
    let with_cookie = ("Cookie", cookie.as_str());
    let with_raw = ("Cookie", raw_cookie.as_str());
    for (what, extra) in [("no credentials", None), ("a wrong bearer", Some(wrong)), ("the read cookie", Some(with_cookie)), ("the token as a cookie", Some(with_raw))] {
        let mut headers = vec![JSON];
        headers.extend(extra);
        let post = |name: &str| s.call(&own, "POST", "/daemon/commands/submit-run", &headers, Some(body(name).as_bytes()));
        let (known, dir_book, absent, pathy) = (post("known-book"), post("dir-book"), post("no-such-book"), post("../books/dir-book"));
        assert_eq!(known.status, 401, "{what}: {}", known.text());
        for other in [&dir_book, &absent, &pathy] {
            assert_eq!(shape(other), shape(&known), "{what}: the answer tells a runbook that exists from one that does not");
        }
    }
    assert_eq!(s.rows("/daemon/views/runs"), Vec::<Value>::new(), "a refused start submitted a run");
}

#[test]
fn a_foreign_host_or_origin_is_refused_on_the_view_and_on_the_start_even_with_the_token() {
    let dir = scratch("sec-start-host");
    let s = serve("host", stub(), Confinement::Linux, Some(books(&dir)), Some(TOKEN), dir);
    let port = s.addr.port();
    let own = s.own();
    let bearer = format!("Bearer {TOKEN}");
    let start = json!({"runbook_id": "known-book", "intake": "an invented intake", "confinement": "linux"}).to_string();
    let hosts = [
        format!("127.0.0.1.attacker.example:{port}"),
        format!("localhost.attacker.example:{port}"),
        format!("rebound.example:{port}"),
        format!("127.0.0.1:{}", port.wrapping_add(1)),
        "127.0.0.1".to_string(),
        "localhost".to_string(),
        format!("127.1:{port}"),
        format!("0.0.0.0:{port}"),
        format!("2130706433:{port}"),
        format!("127.0.0.1:{port}@attacker.example"),
        format!("attacker.example#127.0.0.1:{port}"),
        format!("127.0.0.1:{port}, attacker.example"),
        String::new(),
    ];
    let origins = [
        "http://attacker.example".to_string(),
        "null".to_string(),
        format!("https://{own}"),
        format!("http://127.0.0.1.attacker.example:{port}"),
        format!("http://localhost:{port}"),
        format!("http://{own}.attacker.example"),
        format!("http://attacker.example@{own}"),
        format!("http://{own}/"),
        format!("HTTP://{own}:80"),
        format!("file://{own}"),
        String::new(),
    ];
    for (method, path, body) in [("GET", "/daemon/views/runbooks", None), ("POST", "/daemon/commands/submit-run", Some(start.as_bytes())), ("GET", "/start.js", None)] {
        for host in &hosts {
            let r = s.call(host, method, path, &[JSON, ("Authorization", &bearer)], body);
            assert!(r.status == 403 || r.status == 400, "{method} {path} with Host {host:?} answered {}: {}", r.status, r.text());
            assert!(!r.text().contains("known-book") && !r.text().contains("dir-book") && !r.text().contains("attacker"), "Host {host:?}: {}", r.text());
        }
        for origin in &origins {
            let r = s.call(&own, method, path, &[JSON, ("Authorization", &bearer), ("Origin", origin)], body);
            assert_eq!(r.status, 403, "{method} {path} with Origin {origin:?}: {}", r.text());
            assert!(!r.text().contains("known-book") && !r.text().contains("dir-book") && !r.text().contains("attacker"), "Origin {origin:?}: {}", r.text());
        }
        // two Origin headers, one of them the page's own
        let page = format!("http://{own}");
        let r = s.call(&own, method, path, &[JSON, ("Authorization", &bearer), ("Origin", &page), ("Origin", "http://attacker.example")], body);
        assert_eq!(r.status, 403, "{method} {path} with two Origin headers: {}", r.text());
        // the page's own origin passes
        let r = s.call(&own, method, path, &[JSON, ("Authorization", &bearer), ("Origin", &page)], if method == "POST" { Some(&b"not json"[..]) } else { None });
        assert!(r.status != 403 && r.status != 401, "{method} {path} from the page's own origin: {} {}", r.status, r.text());
    }
    assert_eq!(s.rows("/daemon/views/runs"), Vec::<Value>::new(), "a refused start submitted a run");
}

// ---- invariant 3: the floor is the daemon's, and a refused start writes nothing ----

#[test]
fn a_start_built_by_hand_below_the_floor_is_refused_and_writes_nothing() {
    let dir = scratch("sec-start-floor");
    let s = serve("floor", stub(), Confinement::Linux, Some(books(&dir)), Some(TOKEN), dir);
    let state = s.dir.join("state");
    let before = snapshot(&state);
    assert!(!before.is_empty(), "the state directory holds the journal of the start");
    let events_before = s.rows("/daemon/views/daemon-events?after_position=0");

    for book in ["known-book", "dir-book"] {
        let r = s.submit(&json!({"runbook_id": book, "intake": "an invented intake", "confinement": "local", "idempotency_key": format!("floor-{book}")}).to_string());
        assert_eq!(r.status, 409, "{book}: {}", r.text());
        assert_eq!(r.json()["outcome"], json!("below-floor"), "{}", r.text());
        assert!(r.header("location").is_none(), "a refused start names no run to look at");
    }
    // a level the daemon does not know is no level: refused before anything is looked up
    for level in [json!("LOCAL"), json!("Local"), json!(" local"), json!("local "), json!("none"), json!(""), json!("off"), json!("substrate"), json!("pod"), json!(0), json!(false), json!(["local"]), json!({"level": "local"})] {
        let r = s.submit(&json!({"runbook_id": "known-book", "intake": "an invented intake", "confinement": level}).to_string());
        assert_eq!(r.status, 400, "confinement {level}: {}", r.text());
    }
    // the member twice, the last one below the floor
    let twice = r#"{"runbook_id":"known-book","intake":"an invented intake","confinement":"linux","confinement":"local"}"#;
    let r = s.submit(twice);
    assert!(r.status == 400 || r.status == 409, "confinement twice: {} {}", r.status, r.text());

    assert_eq!(s.rows("/daemon/views/runs"), Vec::<Value>::new(), "a start below the floor made a run");
    assert_eq!(s.rows("/daemon/views/daemon-events?after_position=0"), events_before, "a start below the floor recorded an event");
    assert_eq!(snapshot(&state), before, "a start below the floor wrote into the state directory");

    // the refused start did not take its key: the same key at the floor starts the one run
    let r = s.submit(&json!({"runbook_id": "known-book", "intake": "an invented intake", "confinement": "linux", "idempotency_key": "floor-known-book"}).to_string());
    assert_eq!((r.status, r.json()["outcome"].clone()), (202, json!("submitted")), "{}", r.text());
    // and a start that names no level is never below the floor
    let r = s.submit(&json!({"runbook_id": "known-book", "intake": "an invented intake"}).to_string());
    assert_eq!(r.status, 202, "{}", r.text());
    let r = s.submit(&json!({"runbook_id": "known-book", "intake": "an invented intake", "confinement": null}).to_string());
    assert_eq!(r.status, 202, "{}", r.text());
    let runs = s.rows("/daemon/views/runs");
    assert_eq!(runs.len(), 3, "{runs:?}");
    assert!(runs.iter().all(|r| r["confinement"] == json!("linux")), "{runs:?}");
}

// ---- invariant 4: nothing a client types is a principal or an identity of the run ----

#[test]
fn nothing_typed_into_a_start_becomes_the_principal_or_the_identity_of_the_run() {
    let dir = scratch("sec-start-principal");
    let s = serve("principal", stub(), Confinement::Linux, None, Some(TOKEN), dir);
    let principal_before = s.rows("/daemon/views/configuration")[0]["principal"].clone();
    assert_eq!(principal_before, json!(super::PRINCIPAL));
    let daemon_id = s.rows("/daemon/views/configuration")[0]["daemon_id"].clone();
    let body = json!({
        "runbook_id": "known-book", "intake": "an invented intake", "confinement": "linux",
        "principal": "person:mallory", "answered_by": "person:mallory", "started_by": "person:mallory", "submitted_by": "person:mallory",
        "client": "person:mallory", "daemon_id": "mallory-daemon", "run_id": "mallory-run", "state": "Ended", "outcome": "mallory-outcome",
        "submitted_at": "1999-01-01T00:00:00Z", "confinement_floor": "local"
    });
    let r = s.submit(&body.to_string());
    // members the command does not have are refused or ignored; they are never taken
    assert!(r.status == 202 || r.status == 400, "{} {}", r.status, r.text());
    assert!(!r.text().contains("mallory"), "the answer carries what the client typed: {}", r.text());
    let config = s.rows("/daemon/views/configuration");
    assert_eq!(config[0]["principal"], principal_before);
    assert_eq!(config[0]["daemon_id"], daemon_id);
    assert_eq!(config[0]["confinement_floor"], json!("linux"));
    let runs = s.rows("/daemon/views/runs");
    if r.status == 202 {
        let run = r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_string();
        assert_ne!(run, "mallory-run");
        assert_eq!(runs.len(), 1);
        let detail = s.authed(&format!("/daemon/views/run?run_id={run}"));
        let events = s.authed(&format!("/daemon/views/events?run_id={run}&after_position=0"));
        for (what, text) in [("the run", detail.text()), ("the events of the run", events.text()), ("the run list", format!("{runs:?}"))] {
            assert!(!text.contains("mallory") && !text.contains("1999-01-01"), "{what} holds what the client typed: {text}");
        }
        assert_eq!(s.authed("/daemon/views/run?run_id=mallory-run").json()["rows"], json!([]));
    } else {
        assert_eq!(runs.len(), 0);
    }
    let journal: String = snapshot(&s.dir.join("state")).values().map(|b| String::from_utf8_lossy(b).into_owned()).collect();
    assert!(!journal.contains("mallory"), "the state directory holds what the client typed as an identity");
}

// ---- invariant 6: the view names runbooks and nothing of the host ----

fn plain(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('.') && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

#[test]
fn the_view_names_no_path_and_lists_exactly_what_a_start_accepts_for_odd_entries_of_the_runbook_directory() {
    let dir = scratch("sec-start-odd");
    let books = dir.join("books");
    let outside = dir.join("outside-secret-place");
    std::fs::create_dir_all(outside.join("inner-secret")).unwrap();
    std::fs::write(outside.join("secret.protocol.yaml"), "secret: outside-secret-text\n").unwrap();
    let mk = |name: &str| std::fs::create_dir_all(books.join(name)).unwrap();
    let touch = |name: &str| std::fs::write(books.join(name), "invented: runbook\n").unwrap();
    std::fs::create_dir_all(&books).unwrap();
    for d in ["ok-dir", ".git", ".hidden", "--help", "-rf", "a..b", "with space", "<img src=x onerror=alert(1)>", "new\nline", "tab\there", "semi;colon", "quote\"d", "nested/inner", "x.protocol.yaml.d", "dir.protocol.yaml", "UPPER", "ünïcode", "$(id)", "%2e%2e"] {
        mk(d);
    }
    for f in ["ok-file.protocol.yaml", ".dotted.protocol.yaml", ".protocol.yaml", "..protocol.yaml", "-o.protocol.yaml", "plain-file", "notes.txt", "sp ace.protocol.yaml", "<b>.protocol.yaml", "ok-dir.protocol.yaml"] {
        touch(f);
    }
    std::fs::create_dir_all(books.join(std::ffi::OsString::from_vec(vec![b'b', b'a', b'd', 0xff, 0xfe]))).unwrap();
    std::os::unix::fs::symlink(&outside, books.join("link-out")).unwrap();
    std::os::unix::fs::symlink(outside.join("secret.protocol.yaml"), books.join("link-file.protocol.yaml")).unwrap();
    std::os::unix::fs::symlink(books.join("nowhere"), books.join("broken")).unwrap();
    std::os::unix::fs::symlink("..", books.join("up")).unwrap();

    let s = serve("odd", stub(), Confinement::Linux, Some(books.clone()), Some(TOKEN), dir);
    let answer = s.authed("/daemon/views/runbooks");
    assert_eq!(answer.status, 200, "{}", answer.text());
    let text = answer.text();
    for leak in [s.dir.to_str().unwrap(), "outside-secret", "inner-secret", "secret.protocol", "/home/", "/books", "state-sec-start-odd"] {
        assert!(!text.contains(leak), "the view names {leak:?}: {text}");
    }
    let rows = answer.json()["rows"].as_array().unwrap().clone();
    let mut listed = BTreeSet::new();
    for row in &rows {
        let keys: BTreeSet<&str> = row.as_object().unwrap().keys().map(String::as_str).collect();
        assert!(keys.is_subset(&BTreeSet::from(["runbook_id", "runbook_version", "runtime"])), "a row holds more than a name, a version and the runtime: {row}");
        let name = row["runbook_id"].as_str().unwrap().to_string();
        assert!(plain(&name), "a listed name is not a plain name: {name:?}");
        assert!(listed.insert(name.clone()), "{name} is listed twice");
    }
    for must in ["ok-dir", "ok-file", "known-book", "--help", "-rf", "-o", "a..b", "UPPER", "dir.protocol.yaml", "x.protocol.yaml.d", "nested"] {
        assert!(listed.contains(must), "{must} is a plain name of the directory and is not listed: {listed:?}");
    }
    for never in [".git", ".hidden", ".dotted", "", ".", "..", "up/books", "with space", "broken", "plain-file", "notes.txt", "notes", "nested/inner", "inner"] {
        assert!(!listed.contains(never), "{never:?} is listed: {listed:?}");
    }

    // both ways: a listed name is one a start accepts, and every other way of naming an entry is `no-runbook`
    let mut asked: BTreeSet<String> = std::fs::read_dir(&books).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    asked.extend(listed.iter().cloned());
    for extra in [
        "../books/ok-dir", "ok-dir/", "./ok-dir", "ok-dir/../ok-dir", "ok-dir/.", "/ok-dir", "nested/inner", "..", ".", "", "up", "up/books/ok-dir", "link-out/inner-secret", "link-out/../outside-secret-place",
        "ok-dir\0", "ok-dir\n", " ok-dir", "OK-DIR", "ok-file.protocol.yaml", "link-file.protocol.yaml", "..%2fbooks%2fok-dir", "ok%2ddir",
    ] {
        asked.insert(extra.to_string());
    }
    asked.insert(s.dir.join("books/ok-dir").to_string_lossy().into_owned());
    asked.insert(s.dir.join("outside-secret-place").to_string_lossy().into_owned());
    let mut accepted = BTreeSet::new();
    for name in &asked {
        let r = s.submit(&json!({"runbook_id": name, "intake": "an invented intake", "confinement": "linux"}).to_string());
        let outcome = r.json()["outcome"].as_str().unwrap_or("").to_string();
        assert!(!r.text().contains(s.dir.to_str().unwrap()) || name.contains(s.dir.to_str().unwrap()), "{name:?}: the answer names a path of the host: {}", r.text());
        if listed.contains(name) {
            assert_eq!((r.status, outcome.as_str()), (202, "submitted"), "{name:?} is listed and the start is refused: {}", r.text());
            accepted.insert(name.clone());
        } else {
            assert_eq!((r.status, outcome.as_str()), (422, "no-runbook"), "{name:?} is not listed and the start is not refused as no-runbook: {}", r.text());
        }
    }
    let runs = s.rows("/daemon/views/runs");
    assert_eq!(runs.len(), accepted.len(), "one run for each accepted start and none for a refused one");
    assert!(runs.iter().all(|r| listed.contains(r["runbook_id"].as_str().unwrap())), "{runs:?}");
}

#[test]
fn a_runbook_home_with_odd_files_is_listed_by_name_and_revision_and_names_no_path() {
    let dir = scratch("sec-start-home");
    let home = dir.join("home");
    let outside = dir.join("outside-secret-place");
    let write = |path: PathBuf, text: &str| {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };
    let protocol = |id: &str, revision: &str| format!("format: protocol/1\nprotocol:\n  id: {id}\n  revision: {revision}\n");
    write(home.join("protocol/alpha.protocol.yaml"), &protocol("alpha", "3"));
    write(home.join("bindings/alpha.yaml"), "format: loom.bindings/1\n");
    for odd in [".dot", "sp ace", "<b>", "semi;colon"] {
        write(home.join(format!("protocol/{odd}.protocol.yaml")), &protocol("odd", "1"));
        write(home.join(format!("bindings/{odd}.yaml")), "format: loom.bindings/1\n");
    }
    // a revision that is no number, and one that is a path
    write(home.join("protocol/texty.protocol.yaml"), &protocol("texty", "\"/etc/hostname\""));
    write(home.join("bindings/texty.yaml"), "format: loom.bindings/1\n");
    // not YAML
    write(home.join("protocol/broken.protocol.yaml"), ": : :\n\t- [");
    write(home.join("bindings/broken.yaml"), "format: loom.bindings/1\n");
    // a directory where the file is expected
    std::fs::create_dir_all(home.join("protocol/dir.protocol.yaml")).unwrap();
    write(home.join("bindings/dir.yaml"), "format: loom.bindings/1\n");
    // a protocol whose file is a link out of the home
    write(outside.join("far.protocol.yaml"), &protocol("far", "77"));
    std::os::unix::fs::symlink(outside.join("far.protocol.yaml"), home.join("protocol/far.protocol.yaml")).unwrap();
    write(home.join("bindings/far.yaml"), "format: loom.bindings/1\n");

    let engine = Arc::new(Engine::new(home, dir.join("state")));
    let s = serve("home", engine, Confinement::Linux, None, Some(TOKEN), dir);
    let answer = s.authed("/daemon/views/runbooks");
    assert_eq!(answer.status, 200, "{}", answer.text());
    let text = answer.text();
    for leak in [s.dir.to_str().unwrap(), "outside-secret", "/etc/hostname", "protocol/", "bindings/", ".yaml", "/home/"] {
        assert!(!text.contains(leak), "the view names {leak:?}: {text}");
    }
    let rows = answer.json()["rows"].as_array().unwrap().clone();
    for row in &rows {
        let name = row["runbook_id"].as_str().unwrap();
        assert!(plain(name), "a listed name is not a plain name: {name:?}");
        assert_eq!(row["runtime"], json!("loom"));
        assert!(row["runbook_version"].as_str().is_some_and(|v| v.parse::<i64>().is_ok()), "the version of {name} is no revision: {row}");
    }
    let names: Vec<&str> = rows.iter().map(|r| r["runbook_id"].as_str().unwrap()).collect();
    assert!(names.contains(&"alpha"), "{names:?}");
    for never in [".dot", "sp ace", "<b>", "semi;colon", "texty", "broken", "dir"] {
        assert!(!names.contains(&never), "{never:?} is listed: {names:?}");
    }
}

// ---- invariant 10: the page's files and their content policy ----

#[test]
fn the_start_script_is_behind_the_token_and_carries_the_policy_of_the_page_which_allows_no_foreign_source() {
    let dir = scratch("sec-start-csp");
    let s = serve("csp", stub(), Confinement::Linux, None, Some(TOKEN), dir);
    let own = s.own();
    let bearer = format!("Bearer {TOKEN}");
    assert_eq!(s.get("/start.js", &[]).status, 401, "the start script is read without the token");
    let file = s.authed("/start.js");
    assert_eq!(file.status, 200);
    assert!(file.header("content-type").is_some_and(|t| t.starts_with("text/javascript")), "{:?}", file.header("content-type"));
    assert_eq!(file.header("x-content-type-options"), Some("nosniff"));
    assert!(file.text().contains("/daemon/commands/submit-run"));
    let index = s.authed("/");
    let csp = index.header("content-security-policy").expect("a policy on the page").to_string();
    assert_eq!(file.header("content-security-policy"), Some(csp.as_str()), "the start script has another policy than the page");
    let directive = |name: &str| csp.split(';').map(str::trim).find(|d| d.split(' ').next() == Some(name)).unwrap_or_else(|| panic!("no {name} in {csp}")).to_string();
    assert_eq!(directive("default-src"), "default-src 'none'");
    assert_eq!(directive("connect-src"), "connect-src 'self'");
    assert_eq!(directive("form-action"), "form-action 'self'");
    assert_eq!(directive("base-uri"), "base-uri 'none'");
    assert_eq!(directive("frame-ancestors"), "frame-ancestors 'none'");
    let script: Vec<String> = directive("script-src").split(' ').skip(1).map(String::from).collect();
    assert_eq!(script.len(), 2, "script-src is 'self' and the hash of the one inline script of index.html: {script:?}");
    assert_eq!(script[0], "'self'");
    assert!(script[1].starts_with("'sha256-") && script[1].ends_with('\''), "{script:?}");
    for loose in ["unsafe-eval", "http:", "https:", "ws:", "wss:", "*", "blob:", "strict-dynamic", "script-src-attr", "script-src-elem"] {
        assert!(!csp.contains(loose), "the policy has {loose}: {csp}");
    }
    assert!(!directive("script-src").contains("unsafe-inline"), "{csp}");
    // the file is read, never written or run by a request
    for method in ["POST", "PUT", "DELETE", "PATCH"] {
        let r = s.call(&own, method, "/start.js", &[JSON, ("Authorization", &bearer)], Some(&b"{}"[..]));
        assert_eq!(r.status, 405, "{method} /start.js: {}", r.text());
    }
    // nothing the page loads names an address of the network
    for path in ["/", "/app.js", "/start.js", "/decide.js", "/source.js"] {
        let body = s.authed(path).text();
        for needle in ["<script src=\"http", "<script src=\"//", "<link rel=\"stylesheet\" href=\"http", "from 'http", "from \"http", "import('http", "import(\"http", "cdn.", "unpkg", "jsdelivr", "googleapis"] {
            assert!(!body.contains(needle), "{path} holds {needle:?}");
        }
    }
}
