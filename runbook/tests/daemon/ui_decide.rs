//! Answering a decision from the page (story `ui-embedded-in-the-binary`, slice 2): the command is the
//! daemon's own `answer-decision`, behind the same guard as every route. A cookie alone never answers
//! one, a request from another origin never does, with a token or without one, and the page's file that
//! sends the command keeps the token out of every store of the browser. The stub runtime of `runbook
//! serve` has a preset that waits on two decisions, so the page can be driven against the binary.

use super::commands::Fx;
use super::http::{send, Resp};
use runbook::api;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_runbook");
const TOKEN: &str = "t0ken-for-the-decide-tests-0123456789-abcdef";

struct Srv {
    fx: Fx,
    addr: SocketAddr,
    rt: Option<tokio::runtime::Runtime>,
}

impl Srv {
    fn start(tag: &str, token: Option<&str>) -> Srv {
        let fx = Fx::new(tag);
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        let listener = rt.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).unwrap();
        let addr = listener.local_addr().unwrap();
        let (daemon, token) = (fx.d().clone(), token.map(String::from));
        rt.spawn(async move {
            let _ = api::serve_ui(listener, daemon, token.as_deref(), true, std::future::pending()).await;
        });
        Srv { fx, addr, rt: Some(rt) }
    }

    fn origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.addr.port())
    }

    fn cookie(&self) -> String {
        let r = send(self.addr, "POST", "/ui/login", &[("Content-Type", "application/x-www-form-urlencoded")], Some(format!("token={TOKEN}").as_bytes()));
        assert_eq!(r.status, 303, "{}", r.text());
        r.header("set-cookie").expect("a cookie").split(';').next().unwrap().to_string()
    }

    /// `answer-decision` for request `q1` of `run`, with the headers given.
    fn answer(&self, run: &str, headers: &[(&str, &str)]) -> Resp {
        let body = json!({"run_id": run, "request_id": "q1", "answer": "A", "answered_by": "a page"}).to_string();
        let mut h = vec![("Content-Type", "application/json")];
        h.extend_from_slice(headers);
        send(self.addr, "POST", "/daemon/commands/answer-decision", &h, Some(body.as_bytes()))
    }

    /// A run that waits on `q1`.
    fn waiting(&self) -> String {
        let run = self.fx.submit("ask");
        self.fx.wait_open_decisions(&run, 1);
        run
    }

    fn still_open(&self, run: &str) -> bool {
        self.fx.d().open_decisions_of_run(run).len() == 1 && self.fx.state_of(run) == "WaitingOnDecision"
    }
}

impl Drop for Srv {
    fn drop(&mut self) {
        if let Some(rt) = self.rt.take() {
            rt.shutdown_background();
        }
    }
}

#[test]
fn a_cookie_alone_does_not_answer_a_decision() {
    let s = Srv::start("decide-cookie", Some(TOKEN));
    let run = s.waiting();
    let cookie = s.cookie();
    let own = s.origin();
    for extra in [vec![("Cookie", cookie.as_str())], vec![("Cookie", cookie.as_str()), ("Origin", own.as_str())]] {
        let r = s.answer(&run, &extra);
        assert_eq!(r.status, 401, "{}", r.text());
    }
    // the cookie next to a wrong bearer is no better
    let r = s.answer(&run, &[("Cookie", &cookie), ("Authorization", "Bearer wrong")]);
    assert_eq!(r.status, 401, "{}", r.text());
    assert!(s.still_open(&run), "no request with the cookie alone answered the decision");
    // the bearer token answers, with the page's own origin
    let bearer = format!("Bearer {TOKEN}");
    let r = s.answer(&run, &[("Authorization", &bearer), ("Origin", &own)]);
    assert_eq!(r.status, 202, "{}", r.text());
    s.fx.wait_state(&run, "Ended");
    assert!(s.fx.d().open_decisions_of_run(&run).is_empty());
}

#[test]
fn a_decision_from_another_origin_is_refused_with_a_token_and_without_one() {
    let bearer = format!("Bearer {TOKEN}");
    for (tag, token) in [("decide-origin-token", Some(TOKEN)), ("decide-origin-open", None)] {
        let s = Srv::start(tag, token);
        let run = s.waiting();
        let auth: Vec<(&str, &str)> = token.map(|_| vec![("Authorization", bearer.as_str())]).unwrap_or_default();
        for origin in ["http://other-site.example", "http://rebound.example:80", "null", "https://127.0.0.1", "http://localhost.other.example"] {
            let mut h = auth.clone();
            h.push(("Origin", origin));
            let r = s.answer(&run, &h);
            assert_eq!(r.status, 403, "{tag}: Origin {origin}: {}", r.text());
            assert!(!r.text().contains("other-site") && !r.text().contains("rebound"), "the refusal names the origin: {}", r.text());
        }
        assert!(s.still_open(&run), "{tag}: no foreign request answered the decision");
        // the same request from the daemon's own origin is the page's, and it answers
        let mut h = auth.clone();
        let own = s.origin();
        h.push(("Origin", &own));
        let r = s.answer(&run, &h);
        assert_eq!(r.status, 202, "{tag}: {}", r.text());
    }
}

#[test]
fn a_decision_is_answered_by_json_only() {
    let s = Srv::start("decide-json", None);
    let run = s.waiting();
    let body = json!({"run_id": run, "request_id": "q1", "answer": "A", "answered_by": "a page"}).to_string();
    for kind in ["text/plain", "application/x-www-form-urlencoded", "multipart/form-data; boundary=x"] {
        let r = send(s.addr, "POST", "/daemon/commands/answer-decision", &[("Content-Type", kind)], Some(body.as_bytes()));
        assert_eq!(r.status, 415, "{kind}: {}", r.text());
    }
    assert!(s.still_open(&run));
}

/// The file the page sends commands from. It is plain text of the page: what it must not touch is
/// found by reading it, and a Playwright test (`ui/test/daemon-decide.spec.mjs`) looks at the browser.
fn decide_js() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/web/decide.js")).expect("ui/web/decide.js")
}

#[test]
fn the_page_keeps_the_token_in_memory_and_writes_no_markup() {
    let js = decide_js();
    for banned in ["localStorage", "sessionStorage", "indexedDB", "document.cookie", "history.", "location.", "innerHTML", "outerHTML", "insertAdjacentHTML", "document.write", "eval(", "new Function", "setAttribute('style'", "XMLHttpRequest"] {
        assert!(!js.contains(banned), "ui/web/decide.js uses {banned}");
    }
    // a command carries the bearer and JSON, and never the cookie
    assert!(js.contains("authorization") && js.contains("application/json") && js.contains("credentials: 'omit'"), "the command's headers");
    assert!(js.contains("/daemon/commands/answer-decision"));
    let source = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/web/source.js")).unwrap();
    assert!(source.contains("/daemon/views/open-decisions"), "the open decisions are read from the daemon's view");
    assert!(js.contains("textContent"), "daemon text reaches the page as a text node");
}

struct Daemon {
    child: Child,
    port: u16,
}

impl Daemon {
    fn start(tag: &str) -> Daemon {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("empty")).unwrap();
        let mut cmd = Command::new(BIN);
        cmd.args(["serve", "--ui", "--listen", "127.0.0.1:0", "--principal", super::PRINCIPAL]).arg("--state-dir").arg(dir.join("state"));
        cmd.current_dir(dir.join("empty")).env_remove("RUNBOOK_TOKEN").env("HOME", &dir).stdout(Stdio::piped()).stderr(Stdio::null());
        let mut child = cmd.spawn().unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.as_mut().unwrap()).read_line(&mut line).unwrap();
        let port: u16 = line.trim().rsplit(':').next().and_then(|p| p.parse().ok()).unwrap_or_else(|| panic!("no listening line: {line:?}"));
        Daemon { child, port }
    }

    fn call(&self, method: &str, path: &str, body: Option<Value>) -> Resp {
        let addr = SocketAddr::from(([127, 0, 0, 1], self.port));
        let headers: &[(&str, &str)] = if body.is_some() { &[("Content-Type", "application/json")] } else { &[] };
        send(addr, method, path, headers, body.map(|b| b.to_string()).as_deref().map(str::as_bytes))
    }

    fn rows(&self, view: &str, run: &str) -> Vec<Value> {
        let r = self.call("GET", &format!("/daemon/views/{view}?run_id={run}"), None);
        assert_eq!(r.status, 200, "{}", r.text());
        r.json()["rows"].as_array().unwrap().clone()
    }

    fn until(&self, what: &str, mut ok: impl FnMut() -> bool) {
        let end = Instant::now() + Duration::from_secs(20);
        while !ok() {
            assert!(Instant::now() < end, "{what}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn the_binary_has_a_stub_run_that_waits_on_two_decisions_and_goes_on_when_they_are_answered() {
    let d = Daemon::start("decide-bin");
    let intake = "Approve <b>this</b> plan";
    let r = d.call("POST", "/daemon/commands/submit-run", Some(json!({"runbook_id": "stub-decision", "intake": intake})));
    assert_eq!(r.status, 202, "{}", r.text());
    let run = r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_string();
    d.until("two open decisions", || d.rows("open-decisions", &run).len() == 2);
    let open = d.rows("open-decisions", &run);
    let ids: Vec<&str> = open.iter().map(|o| o["request_id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
    assert!(open.iter().all(|o| o["question"].as_str().unwrap().contains(intake)), "the question is made from the intake: {open:?}");
    let letters: Vec<&str> = open[0]["options"].as_array().unwrap().iter().map(|o| o["letter"].as_str().unwrap()).collect();
    assert_eq!(letters, ["A", "B"]);
    assert_eq!(d.rows("run", &run)[0]["state"], "WaitingOnDecision");

    for (i, id) in ids.iter().enumerate() {
        let r = d.call("POST", "/daemon/commands/answer-decision", Some(json!({"run_id": run, "request_id": id, "answer": "A", "answered_by": "the test"})));
        assert_eq!(r.status, 202, "{}", r.text());
        d.until("the open list shrinks", || d.rows("open-decisions", &run).len() == 1 - i);
    }
    d.until("the run ends", || d.rows("run", &run)[0]["state"] == "Ended");
    assert_eq!(d.rows("run", &run)[0]["outcome"], "approved");
    let events = d.call("GET", &format!("/daemon/views/events?run_id={run}&after_position=0"), None).json();
    let kinds: Vec<&str> = events["rows"].as_array().unwrap().iter().map(|e| e["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds.last(), Some(&"stub.answered"), "the run's events continue after the answers: {kinds:?}");
}

#[test]
fn an_intake_of_a_megabyte_gives_a_small_open_decisions_answer() {
    let d = Daemon::start("decide-big-intake");
    let intake = "é".repeat(524_288);
    assert_eq!(intake.len(), 1_048_576);
    let r = d.call("POST", "/daemon/commands/submit-run", Some(json!({"runbook_id": "stub-decision", "intake": intake})));
    assert_eq!(r.status, 202, "{}", r.text());
    let run = r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_string();
    d.until("two open decisions", || d.rows("open-decisions", &run).len() == 2);
    let body = d.call("GET", &format!("/daemon/views/open-decisions?run_id={run}"), None).text();
    assert!(body.len() < 16 * 1024, "the open-decisions answer is {} bytes for an intake of 1 MiB", body.len());
    let cut: String = intake.chars().take(200).collect();
    assert!(body.contains(&cut), "the stub keeps the first 200 characters of the intake");
    assert!(!body.contains(&"é".repeat(201)), "and no more than 200");
}
