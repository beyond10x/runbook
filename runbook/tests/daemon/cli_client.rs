//! The CLI as a client of the daemon (story `run-in-process-and-cli-as-daemon-client`, client part):
//! `runbook run --detach`, `runs list|show|events|cancel|decisions` and `decide`, each run as the
//! real binary against the daemon of a fixture served in this process on `127.0.0.1:0`.

use super::http::{send, Server};
use runbook::client::{exit, url};
use runbook::stub::{Script, Step};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_runbook");

/// `runbook <args>` with a clean environment (plus `env`), in `dir`.
fn cli_in(dir: &Path, args: &[&str], env: &[(&str, &str)], stdin: Option<&str>) -> Output {
    let mut c = Command::new(BIN);
    c.args(args).env_clear().env("HOME", dir).current_dir(dir).stdout(Stdio::piped()).stderr(Stdio::piped());
    crate::isolate(&mut c);
    for (k, v) in env {
        c.env(k, v);
    }
    c.stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() });
    let mut child = c.spawn().unwrap();
    if let Some(text) = stdin {
        child.stdin.take().unwrap().write_all(text.as_bytes()).unwrap();
    }
    child.wait_with_output().unwrap()
}

fn url_of(addr: SocketAddr) -> String {
    format!("http://{addr}")
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap()
}

struct Fixture {
    server: Server,
    url: String,
    dir: PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Fixture {
        let server = Server::start(&format!("cc-{tag}"), None);
        let stub = &server.fx.stub;
        stub.script(
            "emits",
            Script::new(vec![
                Step::Emit { kind: "run.opened".into(), body: json!({"n": 1}) },
                Step::Emit { kind: "run.middle".into(), body: json!({"n": 2}) },
                Step::Emit { kind: "run.closed".into(), body: json!({"n": 3}) },
                Step::End("passed".into()),
            ]),
        );
        let dir = server.fx.dir.clone();
        let url = url_of(server.addr);
        Fixture { server, url, dir }
    }

    fn cli(&self, args: &[&str]) -> Output {
        let mut all: Vec<&str> = args.to_vec();
        all.extend(["--daemon", &self.url]);
        cli_in(&self.dir, &all, &[], None)
    }

    /// The text `curl -N -H 'Accept: text/event-stream'` gives for the events of a run that has ended.
    fn curl_events(&self, run: &str) -> String {
        curl_events(self.server.addr, run)
    }
}

fn curl_events(addr: SocketAddr, run: &str) -> String {
    let r = send(addr, "GET", &format!("/daemon/views/events?run_id={run}"), &[("Accept", "text/event-stream")], None);
    assert_eq!(r.status, 200);
    r.text()
}

fn token_file(dir: &Path, name: &str, token: &str, mode: u32) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, format!("{token}\n")).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)).unwrap();
    p
}

#[test]
fn detach_submits_the_run_and_prints_its_id() {
    let f = Fixture::new("detach");
    std::fs::create_dir_all(f.dir.join("ctx/case")).unwrap();
    std::fs::write(f.dir.join("ctx/case/note.txt"), "x").unwrap();
    std::fs::write(f.dir.join("intake.md"), "the intake text\n").unwrap();
    let o = f.cli(&["run", "quick", "--intake", "intake.md", "--context", "ctx/case", "--set", "a=b", "--set", "c=d=e", "--detach"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let run = out(&o).trim().to_string();
    assert_eq!(out(&o).lines().count(), 1, "one line: the run id");
    let rows = f.server.view("run", &format!("run_id={run}"));
    assert_eq!(rows.len(), 1, "the daemon holds the run the CLI printed");
    assert_eq!(rows[0]["runbook_id"], "quick");
    assert_eq!(rows[0]["intake"], "the intake text\n");
    assert_eq!(rows[0]["settings"], json!({"a": "b", "c": "d=e"}));
    let want_ctx = std::fs::canonicalize(f.dir.join("ctx/case")).unwrap();
    assert_eq!(rows[0]["context_path"], json!(want_ctx.to_str().unwrap()), "a relative context is sent as an absolute path");
    assert!(rows[0].get("session_id").is_none(), "the CLI opens no session");
    let j = f.cli(&["run", "quick", "--intake", "intake.md", "--detach", "--json"]);
    assert_eq!(code(&j), 0, "{}", err(&j));
    let body: Value = serde_json::from_str(&out(&j)).unwrap();
    assert_eq!(body["outcome"], "submitted", "--json prints the route's body");
}

#[test]
fn detach_reads_the_daemon_from_the_environment_and_the_intake_from_standard_input() {
    let f = Fixture::new("detach-env");
    let o = cli_in(&f.dir, &["run", "quick", "--intake", "-", "--detach"], &[("RUNBOOK_DAEMON", &f.url)], Some("from stdin"));
    assert_eq!(code(&o), 0, "{}", err(&o));
    let run = out(&o).trim().to_string();
    assert_eq!(f.server.view("run", &format!("run_id={run}"))[0]["intake"], "from stdin");
}

#[test]
fn detach_without_a_daemon_exits_64_and_says_a_daemon_is_needed() {
    let dir = super::state::scratch("cc-nodaemon");
    std::fs::write(dir.join("i"), "x").unwrap();
    let o = cli_in(&dir, &["run", "quick", "--intake", "i", "--detach"], &[], None);
    assert_eq!(code(&o), 64);
    assert!(err(&o).contains("a daemon is needed"), "{}", err(&o));
    assert_eq!(out(&o), "");
    let o = cli_in(&dir, &["runs", "list"], &[("RUNBOOK_DAEMON", "")], None);
    assert_eq!(code(&o), 64, "an empty RUNBOOK_DAEMON names no daemon");
}

#[test]
fn run_without_detach_runs_in_process_and_names_the_options_it_lacks() {
    let dir = super::state::scratch("cc-inproc");
    std::fs::write(dir.join("i"), "x").unwrap();
    let o = cli_in(&dir, &["run", "sdc", "--intake", "i"], &[], None);
    assert_eq!(code(&o), 64);
    let text = err(&o);
    // the run, the directory of the runs and the settings have defaults now, the money options are optional; the
    // confinement has none
    assert!(text.contains("--confinement") && text.contains("confinement = ") && text.contains("runbook.toml"), "the first value that is not stated is not named with its line: {text}");
    assert!(!text.contains("--runs-dir") && !text.contains("--settings") && !text.contains("--spend-ledger"), "{text}");
    assert!(!text.contains("waits for"), "{text}");
    assert_eq!(out(&o), "");
}

#[test]
fn an_unknown_subcommand_and_a_missing_intake_exit_64_with_the_usage() {
    let dir = super::state::scratch("cc-usage");
    let o = cli_in(&dir, &["runs", "frobnicate"], &[], None);
    assert_eq!(code(&o), 64);
    assert!(err(&o).contains("Usage"), "{}", err(&o));
    let o = cli_in(&dir, &["frobnicate"], &[], None);
    assert_eq!(code(&o), 64);
    let o = cli_in(&dir, &["run", "sdc", "--detach", "--daemon", "http://127.0.0.1:1"], &[], None);
    assert_eq!(code(&o), 64);
    assert!(err(&o).contains("--intake") && err(&o).contains("Usage"), "{}", err(&o));
    std::fs::write(dir.join("i"), "x").unwrap();
    let o = cli_in(&dir, &["run", "sdc", "--intake", "i", "--set", "novalue", "--detach", "--daemon", "http://127.0.0.1:1"], &[], None);
    assert_eq!(code(&o), 64, "a --set without = is a usage error: {}", err(&o));
    let o = cli_in(&dir, &["run", "sdc", "--intake", "i", "--set", "k=1", "--set", "k=2", "--detach", "--daemon", "http://127.0.0.1:1"], &[], None);
    assert_eq!(code(&o), 64, "a key given twice: {}", err(&o));
    let o = cli_in(&dir, &["run", "sdc", "--intake", "missing-file", "--detach", "--daemon", "http://127.0.0.1:1"], &[], None);
    assert_eq!(code(&o), 66, "an intake file that does not exist: {}", err(&o));
}

#[test]
fn runs_list_and_show_print_the_routes_as_text_and_as_json() {
    let f = Fixture::new("list-show");
    let ask = f.server.fx.submit("ask");
    f.server.fx.wait_open_decisions(&ask, 1);
    let quick = f.server.fx.submit("quick");
    f.server.wait_state(&quick, "Ended");

    let o = f.cli(&["runs", "list"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let listed = out(&o);
    let lines: Vec<&str> = listed.lines().collect();
    assert_eq!(lines.len(), 2, "{}", out(&o));
    assert!(lines[0].starts_with(&quick) && lines[0].contains("Ended") && lines[0].contains("done"), "newest first: {}", lines[0]);
    assert!(lines[1].starts_with(&ask) && lines[1].contains("WaitingOnDecision"), "{}", lines[1]);

    let j = f.cli(&["runs", "list", "--json"]);
    let want = f.server.get("/daemon/views/runs").json();
    assert_eq!(serde_json::from_str::<Value>(&out(&j)).unwrap(), want, "--json is the route's body");

    let s = f.cli(&["runs", "show", &ask]);
    assert_eq!(code(&s), 0, "{}", err(&s));
    let text = out(&s);
    assert!(text.contains(&format!("run_id: {ask}")) && text.contains("state: WaitingOnDecision") && text.contains("intake: intake"), "{text}");
    assert!(text.contains("open decision q1: Approve?"), "the request id a person needs for `decide`: {text}");
    let sj = f.cli(&["runs", "show", &ask, "--json"]);
    let want = f.server.get(&format!("/daemon/views/run?run_id={ask}")).json();
    assert_eq!(serde_json::from_str::<Value>(&out(&sj)).unwrap(), want);
    let d = f.cli(&["runs", "decisions", &ask, "--json"]);
    let want = f.server.get(&format!("/daemon/views/open-decisions?run_id={ask}")).json();
    assert_eq!(serde_json::from_str::<Value>(&out(&d)).unwrap(), want);
    let dt = f.cli(&["runs", "decisions", &ask]);
    assert!(out(&dt).starts_with("q1: Approve?"), "{}", out(&dt));

    let missing = f.cli(&["runs", "show", "no-such-run"]);
    assert_eq!(code(&missing), exit::NOT_FOUND as i32, "the view answers 200 and no row: the CLI says no such run");
    assert!(err(&missing).contains("no such run"), "{}", err(&missing));
}

#[test]
fn decide_answers_a_request_and_a_second_answer_is_a_conflict() {
    let f = Fixture::new("decide");
    let run = f.server.fx.submit("ask");
    f.server.fx.wait_open_decisions(&run, 1);
    let o = f.cli(&["decide", &run, "q1", "A"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert_eq!(out(&o).trim(), "answered q1");
    f.server.wait_state(&run, "Ended");
    let rows = f.server.view("decisions", &format!("run_id={run}"));
    assert_eq!(rows[0]["answer"], "A");
    assert_eq!(rows[0]["state"], "Answered");

    let again = f.cli(&["decide", &run, "q1", "A"]);
    assert_eq!(code(&again), exit::CONFLICT as i32);
    assert!(err(&again).contains("sdc.daemon.DecisionAlreadyAnswered"), "the API's error code: {}", err(&again));
    assert_eq!(out(&again), "");

    let none = f.cli(&["decide", &run, "nope", "A"]);
    assert_eq!(code(&none), exit::NOT_FOUND as i32);
    assert!(err(&none).contains("sdc.daemon.NoSuchDecisionRequest"), "{}", err(&none));

    let run2 = f.server.fx.submit("ask-b");
    f.server.fx.wait_open_decisions(&run2, 1);
    let j = f.cli(&["decide", &run2, "q3", "B", "--by", "tester", "--json"]);
    assert_eq!(code(&j), 0, "{}", err(&j));
    let body: Value = serde_json::from_str(&out(&j)).unwrap();
    assert_eq!(body["outcome"], "answered");
    assert_eq!(f.server.view("decisions", &format!("run_id={run2}"))[0]["answered_by"], "tester");
}

#[test]
fn cancel_ends_a_live_run_and_maps_the_refusals_to_exit_codes() {
    let f = Fixture::new("cancel");
    let hold = f.server.fx.submit("hold");
    f.server.wait_state(&hold, "Running");
    let o = f.cli(&["runs", "cancel", &hold]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert_eq!(out(&o).trim(), format!("cancelled {hold}"));
    f.server.wait_state(&hold, "Cancelled");

    let again = f.cli(&["runs", "cancel", &hold]);
    assert_eq!(code(&again), exit::CONFLICT as i32);
    assert!(err(&again).contains("sdc.daemon.RunAlreadyEnded"), "{}", err(&again));

    let unknown = f.cli(&["runs", "cancel", "no-such-run"]);
    assert_eq!(code(&unknown), exit::NOT_FOUND as i32);
    assert!(err(&unknown).contains("sdc.daemon.NoSuchRun"), "{}", err(&unknown));
}

#[test]
fn a_refused_submit_exits_65_and_prints_the_code_and_message() {
    let f = Fixture::new("refused");
    std::fs::write(f.dir.join("i"), "x").unwrap();
    let o = f.cli(&["run", "no-such-runbook", "--intake", "i", "--detach"]);
    assert_eq!(code(&o), exit::REFUSED as i32);
    assert!(err(&o).contains("422") && err(&o).contains("sdc.daemon.NoSuchRunbook"), "{}", err(&o));
    let o = f.cli(&["run", "quick", "--intake", "i", "--context", "/", "--detach"]);
    assert_eq!(code(&o), exit::REFUSED as i32, "a context outside the roots: {}", err(&o));
    assert!(err(&o).contains("sdc.daemon.ContextOutsideRoots"), "{}", err(&o));
}

#[test]
fn runs_events_prints_the_lines_curl_gives() {
    let f = Fixture::new("events");
    let run = f.server.fx.submit("emits");
    f.server.wait_state(&run, "Ended");
    let curl = f.curl_events(&run);
    assert!(curl.starts_with("id: 1\nevent: runbook.submitted\ndata: {"), "{curl}");
    assert_eq!(curl.matches("event: ").count(), 4, "{curl}");

    let followed = f.cli(&["runs", "events", &run]);
    assert_eq!(code(&followed), 0, "{}", err(&followed));
    assert_eq!(out(&followed), curl, "the lines of curl -N, byte for byte");

    let page = f.cli(&["runs", "events", &run, "--no-follow"]);
    assert_eq!(code(&page), 0, "{}", err(&page));
    assert_eq!(out(&page), curl, "--no-follow prints what exists, in the same lines");

    let json = f.cli(&["runs", "events", &run, "--json"]);
    let rows: Vec<Value> = out(&json).lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let want = f.server.view("events", &format!("run_id={run}&after_position=0"));
    assert_eq!(rows, want, "--json: one row of the events route per line");

    let none = f.cli(&["runs", "events", "no-such-run"]);
    assert_eq!(code(&none), exit::NOT_FOUND as i32, "{}", err(&none));
}

#[test]
fn runs_events_follows_a_live_run_until_it_ends() {
    let f = Fixture::new("events-live");
    let run = f.server.fx.submit("ask");
    f.server.fx.wait_open_decisions(&run, 1);
    let mut child = Command::new(BIN)
        .args(["runs", "events", &run, "--daemon", &f.url])
        .env_clear()
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut so = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        so.read_to_string(&mut s).unwrap();
        s
    });
    std::thread::sleep(Duration::from_millis(300));
    assert!(child.try_wait().unwrap().is_none(), "the run waits for a decision: the CLI follows and does not end");
    let answered = f.cli(&["decide", &run, "q1", "A"]);
    assert_eq!(code(&answered), 0, "{}", err(&answered));
    let until = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break s;
        }
        assert!(Instant::now() < until, "the follow did not end with the run");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(status.success());
    let text = reader.join().unwrap();
    assert_eq!(text, f.curl_events(&run));
    assert!(text.contains("event: runbook.submitted"), "{text}");
}

/// A TCP proxy in front of the daemon that cuts the first event stream after its first event and
/// records the head of every request it sees.
struct CutOnce {
    addr: SocketAddr,
    heads: Arc<Mutex<Vec<String>>>,
}

fn cut_once(upstream: SocketAddr) -> CutOnce {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let heads = Arc::new(Mutex::new(Vec::new()));
    let cut = Arc::new(AtomicBool::new(false));
    let seen = heads.clone();
    std::thread::spawn(move || {
        for client in listener.incoming() {
            let Ok(mut client) = client else { return };
            let (seen, cut) = (seen.clone(), cut.clone());
            std::thread::spawn(move || {
                let mut head = Vec::new();
                let mut b = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") {
                    if client.read(&mut b).unwrap_or(0) == 0 {
                        return;
                    }
                    head.push(b[0]);
                }
                let text = String::from_utf8_lossy(&head).into_owned();
                let is_stream = text.contains("text/event-stream");
                seen.lock().unwrap().push(text.clone());
                let mut up = TcpStream::connect(upstream).unwrap();
                // a proxy in front of the daemon names the daemon in `Host`, as a reverse proxy does
                let forwarded: Vec<String> = text.split("\r\n").map(|l| if l.to_ascii_lowercase().starts_with("host:") { format!("Host: {upstream}") } else { l.to_string() }).collect();
                up.write_all(forwarded.join("\r\n").as_bytes()).unwrap();
                let first_stream = is_stream && !cut.swap(true, Ordering::SeqCst);
                let mut got = Vec::new();
                let mut buf = [0u8; 4096];
                loop {
                    let n = up.read(&mut buf).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    got.extend_from_slice(&buf[..n]);
                    if first_stream {
                        // The head, then the chunk of the first event ends with a blank line and the chunk's CRLF.
                        let body_at = got.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4);
                        if let Some(start) = body_at {
                            if let Some(end) = got[start..].windows(4).position(|w| w == b"\n\n\r\n") {
                                let _ = client.write_all(&got[..start + end + 4]);
                                let _ = client.shutdown(std::net::Shutdown::Both);
                                return;
                            }
                        }
                    } else {
                        let _ = client.write_all(&got);
                        got.clear();
                    }
                }
                let _ = client.write_all(&got);
            });
        }
    });
    CutOnce { addr, heads }
}

#[test]
fn runs_events_resumes_with_last_event_id_after_a_dropped_connection_and_prints_no_event_twice() {
    let f = Fixture::new("events-drop");
    let run = f.server.fx.submit("emits");
    f.server.wait_state(&run, "Ended");
    let proxy = cut_once(f.server.addr);
    let o = cli_in(&f.dir, &["runs", "events", &run, "--daemon", &url_of(proxy.addr)], &[], None);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let curl = f.curl_events(&run);
    assert_eq!(out(&o), curl, "every event once, in order, as curl gives them");
    let printed = out(&o);
    let ids: Vec<&str> = printed.lines().filter(|l| l.starts_with("id: ")).collect();
    assert_eq!(ids, ["id: 1", "id: 2", "id: 3", "id: 4"]);
    let heads = proxy.heads.lock().unwrap().clone();
    let streams: Vec<&String> = heads.iter().filter(|h| h.contains("text/event-stream")).collect();
    assert_eq!(streams.len(), 2, "one connection cut, one resumed: {heads:?}");
    assert!(!streams[0].to_lowercase().contains("last-event-id"), "{}", streams[0]);
    assert!(streams[1].to_lowercase().contains("last-event-id: 1\r\n"), "resumes after the last id it printed: {}", streams[1]);
}

#[test]
fn a_connection_that_cannot_be_made_exits_69() {
    let dir = super::state::scratch("cc-refused");
    // Held for the case: nothing listens there and no other server is handed the port.
    let (_held, port) = super::http::refusing_port();
    let o = cli_in(&dir, &["runs", "list", "--daemon", &format!("http://127.0.0.1:{port}")], &[], None);
    assert_eq!(code(&o), exit::UNREACHABLE as i32);
    assert!(err(&o).contains("connection refused"), "{}", err(&o));
    assert_eq!(out(&o), "");
}

#[test]
fn a_401_exits_77_and_no_token_is_printed() {
    let dir = super::state::scratch("cc-401");
    let server = Server::start("cc-401", Some("right-token-0123456789"));
    let wrong = token_file(&dir, "wrong", "wrong-token-9876543210", 0o600);
    let url = url_of(server.addr);
    for args in [vec!["runs", "list"], vec!["runs", "list", "--json"], vec!["runs", "show", "x"], vec!["decide", "r", "q", "A"]] {
        let mut all = args.clone();
        all.extend(["--daemon", &url, "--token-file", wrong.to_str().unwrap()]);
        let o = cli_in(&dir, &all, &[], None);
        assert_eq!(code(&o), exit::DENIED as i32, "{args:?}: {}", err(&o));
        for text in [out(&o), err(&o)] {
            assert!(!text.contains("wrong-token-9876543210") && !text.contains("right-token-0123456789"), "{args:?}: {text}");
        }
        assert!(err(&o).contains("unauthorized"), "the API's code: {}", err(&o));
    }
    let none = cli_in(&dir, &["runs", "list", "--daemon", &url], &[], None);
    assert_eq!(code(&none), exit::DENIED as i32);
    assert!(err(&none).contains("--token-file"), "says where a token comes from: {}", err(&none));
    let plain_env = cli_in(&dir, &["runs", "list", "--daemon", &url], &[("RUNBOOK_TOKEN", "right-token-0123456789")], None);
    assert_eq!(code(&plain_env), exit::DENIED as i32, "a token in a plain environment value is not read");
}

#[test]
fn a_token_file_works_from_the_flag_and_from_the_environment() {
    let dir = super::state::scratch("cc-token-ok");
    let server = Server::start("cc-token-ok", Some("right-token-0123456789"));
    let good = token_file(&dir, "good", "right-token-0123456789", 0o600);
    let url = url_of(server.addr);
    let a = cli_in(&dir, &["runs", "list", "--daemon", &url, "--token-file", good.to_str().unwrap()], &[], None);
    assert_eq!(code(&a), 0, "{}", err(&a));
    let b = cli_in(&dir, &["runs", "list"], &[("RUNBOOK_DAEMON", &url), ("RUNBOOK_TOKEN_FILE", good.to_str().unwrap())], None);
    assert_eq!(code(&b), 0, "{}", err(&b));
    let events = cli_in(&dir, &["runs", "events", "nothing", "--daemon", &url, "--token-file", good.to_str().unwrap()], &[], None);
    assert_eq!(code(&events), exit::NOT_FOUND as i32, "the stream route takes the token too: {}", err(&events));
}

#[test]
fn a_token_file_that_group_or_others_can_read_is_refused_and_never_echoed() {
    let dir = super::state::scratch("cc-token-mode");
    let server = Server::start("cc-token-mode", Some("right-token-0123456789"));
    let url = url_of(server.addr);
    for mode in [0o640, 0o604, 0o644, 0o660] {
        let p = token_file(&dir, &format!("t{mode:o}"), "right-token-0123456789", mode);
        let o = cli_in(&dir, &["runs", "list", "--daemon", &url, "--token-file", p.to_str().unwrap()], &[], None);
        assert_eq!(code(&o), exit::CONFIG as i32, "mode {mode:o}: {}", err(&o));
        assert!(err(&o).contains(&format!("{mode:04o}")) && err(&o).contains("chmod 600"), "{}", err(&o));
        assert!(!err(&o).contains("right-token"), "{}", err(&o));
    }
    let missing = cli_in(&dir, &["runs", "list", "--daemon", &url, "--token-file", "nowhere"], &[], None);
    assert_eq!(code(&missing), exit::CONFIG as i32, "{}", err(&missing));
    let empty = token_file(&dir, "empty", "", 0o600);
    let o = cli_in(&dir, &["runs", "list", "--daemon", &url, "--token-file", empty.to_str().unwrap()], &[], None);
    assert_eq!(code(&o), exit::CONFIG as i32, "an empty token: {}", err(&o));
}

#[test]
fn a_server_that_echoes_the_token_in_its_401_gets_no_echo_from_the_cli() {
    let dir = super::state::scratch("cc-echo");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut s) = conn else { return };
            let mut head = Vec::new();
            let mut b = [0u8; 1];
            while !head.ends_with(b"\r\n\r\n") {
                if s.read(&mut b).unwrap_or(0) == 0 {
                    break;
                }
                head.push(b[0]);
            }
            let text = String::from_utf8_lossy(&head).into_owned();
            let auth = text.lines().find(|l| l.to_lowercase().starts_with("authorization:")).unwrap_or("").to_string();
            let body = json!({"refused": "unauthorized", "message": format!("you sent {auth}")}).to_string();
            let _ = write!(s, "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        }
    });
    let tf = token_file(&dir, "echo", "echoed-token-abcdef123456", 0o600);
    for args in [vec!["runs", "list"], vec!["runs", "list", "--json"], vec!["runs", "events", "r"]] {
        let mut all = args.clone();
        let echo_url = url_of(addr);
        all.extend(["--daemon", &echo_url, "--token-file", tf.to_str().unwrap()]);
        let o = cli_in(&dir, &all, &[], None);
        assert_eq!(code(&o), exit::DENIED as i32, "{args:?}: {}", err(&o));
        for text in [out(&o), err(&o)] {
            assert!(!text.contains("echoed-token-abcdef123456"), "{args:?}: {text}");
        }
        assert!(err(&o).contains("you sent"), "the rest of the message stays: {}", err(&o));
    }
}

#[test]
fn https_and_urls_with_user_info_or_a_path_are_refused_with_exit_64() {
    let dir = super::state::scratch("cc-urls");
    for (u, says) in [
        ("https://example.invalid:7430", "https"),
        ("http://user:hunter2-pw@127.0.0.1:7430", "user info"),
        ("http://127.0.0.1:7430/base", "path"),
        ("ftp://127.0.0.1", "http://"),
        ("127.0.0.1:7430", "http://"),
        ("http://", "host"),
        ("http://127.0.0.1:99999", "port"),
    ] {
        let o = cli_in(&dir, &["runs", "list", "--daemon", u], &[], None);
        assert_eq!(code(&o), 64, "{u}: {}", err(&o));
        assert!(err(&o).contains(says), "{u}: {}", err(&o));
        assert!(!err(&o).contains("hunter2"), "the password is not echoed: {}", err(&o));
    }
}

#[test]
fn the_url_parser_tells_loopback_from_a_remote_host() {
    let t = url::parse("http://127.0.0.1:7430").unwrap();
    assert_eq!((t.host.as_str(), t.port, t.loopback), ("127.0.0.1", 7430, true));
    assert!(url::parse("http://localhost").unwrap().loopback);
    assert_eq!(url::parse("http://localhost").unwrap().port, 80);
    let v6 = url::parse("http://[::1]:9/").unwrap();
    assert_eq!((v6.host.as_str(), v6.port, v6.loopback), ("::1", 9, true));
    assert!(!url::parse("http://192.0.2.7:7430").unwrap().loopback);
    assert!(!url::parse("http://daemon.example:7430").unwrap().loopback);
    assert!(url::parse("http://a@b").is_err());
}

#[test]
fn the_exit_code_table_of_the_page_is_the_one_the_client_has() {
    let page = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/cli.md")).unwrap();
    for (code, name, _) in exit::TABLE {
        assert!(page.lines().any(|l| l.starts_with(&format!("| {code} |")) && l.contains(name)), "no row for {code} {name} in docs/cli.md");
    }
    let rows = page.lines().filter(|l| l.starts_with("| ") && l.split('|').nth(1).is_some_and(|c| c.trim().parse::<u8>().is_ok())).count();
    assert_eq!(rows, exit::TABLE.len(), "the page has a row the client does not have");
    let distinct: std::collections::BTreeSet<u8> = exit::TABLE.iter().map(|(c, _, _)| *c).collect();
    assert_eq!(distinct.len(), exit::TABLE.len(), "every code is distinct");
}
