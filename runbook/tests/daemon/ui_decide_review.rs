//! Security review of slice 2 of `ui-embedded-in-the-binary` (answering a decision from the page): the
//! invariants the first test file of the slice does not measure.

use super::http::{send, Resp, Server};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_runbook");
const TOKEN: &str = "t0ken-for-the-decide-review-0123456789-abcdef";

/// `answered_by` is a free label (`docs/daemon-api.md`, "The web UI"). The daemon bounds the other labels
/// a caller sends (`client`, `idempotency_key`: `MAX_LABEL_CHARS`, 200 characters) and the page bounds this
/// one at 100, but the command takes it at any length, up to the body limit (about 6 MiB), writes it to the
/// journal and republishes it in the `DecisionAnswered` payload. A label the page limits and the daemon does
/// not is a limit that only a page keeps.
#[test]
fn answered_by_is_bounded_by_the_daemon_like_the_other_labels() {
    let s = Server::start("review-decide-label", None);
    let run = s.fx.submit("ask");
    s.fx.wait_open_decisions(&run, 1);
    let long = "x".repeat(201);
    let r = s.post("answer-decision", &json!({"run_id": run, "request_id": "q1", "answer": "A", "answered_by": long}));
    assert_eq!(r.status, 400, "a 201 character answered_by is taken: {}", &r.text()[..r.text().len().min(200)]);
    assert_eq!(s.fx.d().open_decisions_of_run(&run).len(), 1, "the refused answer left the request open");
}

/// What a caller without a token learns from the extra read of the page (`GET /daemon/views/configuration`
/// without credentials) when the daemon has a token: that a token is needed, in the body every refusal has.
#[test]
fn the_unauthenticated_configuration_read_of_a_daemon_with_a_token_reveals_only_that_a_token_is_needed() {
    let s = Server::start("review-decide-config", Some(TOKEN));
    let bare = s.get("/daemon/views/configuration");
    assert_eq!(bare.status, 401, "{}", bare.text());
    assert_eq!(bare.header("www-authenticate"), Some("Bearer"));
    let unknown = s.get("/no/such/route");
    assert_eq!(unknown.status, 401);
    assert_eq!(bare.text(), unknown.text(), "the refusal of the configuration read is the refusal of any route");
    let daemon_id = s.fx.d().daemon_configuration()[0]["daemon_id"].as_str().unwrap().to_string();
    for r in [&bare, &s.get_with("/daemon/views/configuration", &[("Authorization", "Bearer wrong")])] {
        assert_eq!(r.status, 401);
        assert!(!r.text().contains(&daemon_id) && !r.text().contains("confinement_floor") && !r.text().contains("started_at"), "{}", r.text());
    }
    // with the token it is the configuration row: that is what a caller without a token never gets
    let bearer = format!("Bearer {TOKEN}");
    let r = s.get_with("/daemon/views/configuration", &[("Authorization", &bearer)]);
    assert_eq!(r.status, 200);
    let mut keys: Vec<String> = r.json()["rows"][0].as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["confinement_floor", "daemon_id", "principal", "started_at"]);
}

/// A daemon without a token (loopback only) answers the same read with the configuration row: the whole
/// API is open to the host then, and the page asks for no token.
#[test]
fn the_unauthenticated_configuration_read_of_a_daemon_without_a_token_is_the_row_and_nothing_more() {
    let s = Server::start("review-decide-config-open", None);
    let r = s.get("/daemon/views/configuration");
    assert_eq!(r.status, 200, "{}", r.text());
    let mut keys: Vec<String> = r.json()["rows"][0].as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["confinement_floor", "daemon_id", "principal", "started_at"]);
}

struct Bin {
    child: Child,
    port: u16,
}

impl Bin {
    fn start(tag: &str, token: Option<&str>) -> Bin {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("empty")).unwrap();
        let mut cmd = Command::new(BIN);
        cmd.args(["serve", "--ui", "--listen", "127.0.0.1:0", "--principal", super::PRINCIPAL]).arg("--state-dir").arg(dir.join("state"));
        cmd.current_dir(dir.join("empty")).env("HOME", &dir).stdout(Stdio::piped()).stderr(Stdio::null());
        match token {
            Some(t) => cmd.env("RUNBOOK_TOKEN", t),
            None => cmd.env_remove("RUNBOOK_TOKEN"),
        };
        let mut child = cmd.spawn().unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.as_mut().unwrap()).read_line(&mut line).unwrap();
        let port: u16 = line.trim().rsplit(':').next().and_then(|p| p.parse().ok()).unwrap_or_else(|| panic!("no listening line: {line:?}"));
        Bin { child, port }
    }

    fn call(&self, method: &str, path: &str, headers: &[(&str, &str)], body: Option<Value>) -> Resp {
        let addr = SocketAddr::from(([127, 0, 0, 1], self.port));
        let mut h = headers.to_vec();
        if body.is_some() {
            h.push(("Content-Type", "application/json"));
        }
        send(addr, method, path, &h, body.map(|b| b.to_string()).as_deref().map(str::as_bytes))
    }

    fn rows(&self, view: &str, run: &str, headers: &[(&str, &str)]) -> Vec<Value> {
        let r = self.call("GET", &format!("/daemon/views/{view}?run_id={run}"), headers, None);
        assert_eq!(r.status, 200, "{}", r.text());
        r.json()["rows"].as_array().unwrap().clone()
    }
}

impl Drop for Bin {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// `stub-decision` is no route and no default: a run of it starts through `submit-run`, behind the same
/// guard as any command, and the `{run}` and `{intake}` of its texts are filled once, from the submitted
/// values, never expanded again.
#[test]
fn stub_decision_starts_through_submit_run_behind_the_guard_and_fills_its_texts_once() {
    let b = Bin::start("review-decide-stub", Some(TOKEN));
    let submit = json!({"runbook_id": "stub-decision", "intake": "{run}{intake}"});
    // no token: refused, nothing started
    assert_eq!(b.call("POST", "/daemon/commands/submit-run", &[], Some(submit.clone())).status, 401);
    // no route of that name, with a token or without one
    let bearer = format!("Bearer {TOKEN}");
    assert_eq!(b.call("GET", "/stub-decision", &[], None).status, 401);
    assert_eq!(b.call("GET", "/stub-decision", &[("Authorization", &bearer)], None).status, 404);
    // with the token: a run that waits, whose texts hold the intake as it was sent
    let r = b.call("POST", "/daemon/commands/submit-run", &[("Authorization", &bearer)], Some(submit));
    assert_eq!(r.status, 202, "{}", r.text());
    let run = r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_string();
    let end = Instant::now() + Duration::from_secs(20);
    while b.rows("open-decisions", &run, &[("Authorization", &bearer)]).len() != 2 {
        assert!(Instant::now() < end, "two open decisions");
        std::thread::sleep(Duration::from_millis(10));
    }
    for row in b.rows("open-decisions", &run, &[("Authorization", &bearer)]) {
        let id = row["request_id"].as_str().unwrap();
        assert!(id.starts_with(&run) && id.len() == run.len() + 3, "the request id is the run id and a suffix: {id}");
        let question = row["question"].as_str().unwrap();
        assert!(question == "{run}{intake}" || question == "Second look: {run}{intake}", "the intake was expanded again: {question}");
        for o in row["options"].as_array().unwrap() {
            let does = o["does"].as_str().unwrap();
            assert!(does == "yes: {run}{intake}" || does == "no: {run}{intake}", "{does}");
        }
    }
}
