//! A stop of the daemon does not orphan a live run: on SIGTERM or SIGINT `runbook serve` drains. It refuses new
//! submits, keeps serving reads and decisions, lets every live run go on to its end and exits 0 when none is left;
//! a second signal, or `--drain-seconds`, ends it at once and the runs are orphaned with an event that says the
//! daemon was stopped while they ran. Each case starts `runbook serve --runtime loom` as a process on a free port,
//! over the fixture runbook of `engine.rs` (a writer program that sleeps the seconds of `pause.txt` in the
//! context, then a person step). No model call, no network beyond loopback.

use super::http::{send, try_send, Resp};
use super::state::{journal_facts, scratch};
use serde_json::{json, Value};
use std::io::{BufRead, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_runbook");
const RUNBOOK: &str = "engine-demo";
const JSON: (&str, &str) = ("Content-Type", "application/json");
const DRAIN_HEADER: &str = "x-runbook-draining";
/// The exit code of a stop that left a live run orphaned (`serve::EXIT_ORPHANED`, `docs/daemon-api.md`).
const EXIT_ORPHANED: i32 = 75;
const STOPPED: &str = "the daemon was stopped while the run was live";

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

fn until(what: &str, secs: u64, mut done: impl FnMut() -> bool) {
    let limit = Instant::now() + Duration::from_secs(secs);
    while !done() {
        assert!(Instant::now() < limit, "gave up waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

struct Serve {
    child: Child,
    addr: SocketAddr,
    dir: PathBuf,
}

impl Drop for Serve {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A scratch directory holding a copy of the fixture runbook (`home`), a context root and the state directory
/// of one daemon; a second `launch` over the same directory is a restart.
fn prepare(tag: &str) -> PathBuf {
    let dir = scratch(tag);
    let home = dir.join("home");
    copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine-runbook"), &home);
    let binding = home.join("bindings/engine-draft.yaml");
    let text = std::fs::read_to_string(&binding).unwrap();
    assert!(text.contains("principal: reviewer"));
    std::fs::write(&binding, text.replace("principal: reviewer", &format!("principal: {}", super::PRINCIPAL))).unwrap();
    std::fs::create_dir_all(dir.join("ctx/project")).unwrap();
    dir
}

impl Serve {
    /// `runbook serve --runtime loom` over a copy of the fixture runbook; `extra` are more serve options.
    fn start(tag: &str, extra: &[&str]) -> Serve {
        Serve::launch(prepare(tag), extra)
    }

    fn launch(dir: PathBuf, extra: &[&str]) -> Serve {
        Serve::launch_with(dir, extra, false)
    }

    /// `closed_stderr`: standard error is a pipe whose reader is gone once the daemon listens (a `| tee` that ended).
    fn launch_with(dir: PathBuf, extra: &[&str], closed_stderr: bool) -> Serve {
        let mut cmd = Command::new(BIN);
        cmd.args(["serve", "--principal", super::PRINCIPAL, "--runtime", "loom", "--min-confinement", "local", "--mcp", "--listen", "127.0.0.1:0"])
            .arg("--runbook-home")
            .arg(dir.join("home"))
            .arg("--context-root")
            .arg(dir.join("ctx"))
            .arg("--state-dir")
            .arg(dir.join("state"))
            .args(extra);
        let err = if closed_stderr { Stdio::piped() } else { Stdio::from(std::fs::File::create(dir.join("stderr.log")).unwrap()) };
        cmd.env_remove("RUNBOOK_TOKEN").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(err);
        let mut child = cmd.spawn().unwrap();
        let err_reader = child.stderr.take();
        let out = child.stdout.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut lines = std::io::BufReader::new(out).lines();
            if let Some(Ok(first)) = lines.next() {
                let _ = tx.send(first);
            }
            for _ in lines {}
        });
        let line = rx.recv_timeout(Duration::from_secs(20)).expect("no first line of output within 20 s");
        let addr = line.strip_prefix("runbook: listening on http://").unwrap_or_else(|| panic!("{line}")).parse().unwrap();
        drop(err_reader);
        Serve { child, addr, dir }
    }

    fn signal(&self, name: &str) {
        let ok = Command::new("kill").args([&format!("-{name}"), &self.child.id().to_string()]).status().unwrap();
        assert!(ok.success());
    }

    /// The process's exit code, waited for at most `limit`; the time it took.
    fn exit_within(&mut self, limit: Duration) -> (Option<i32>, Duration) {
        let t = Instant::now();
        loop {
            if let Some(s) = self.child.try_wait().unwrap() {
                return (s.code(), t.elapsed());
            }
            assert!(t.elapsed() < limit, "the daemon was still running {limit:?} after the signal: {}", self.stderr());
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn stderr(&self) -> String {
        std::fs::read_to_string(self.dir.join("stderr.log")).unwrap_or_default()
    }

    fn journal(&self) -> Vec<Value> {
        journal_facts(&self.dir.join("state"))
    }

    fn facts(&self, name: &str) -> Vec<Value> {
        self.journal().into_iter().filter(|f| f["fact"] == name).collect()
    }

    fn wait_draining(&self) {
        until("the journal to record daemon.draining", 5, || !self.facts("daemon.draining").is_empty());
    }

    fn get(&self, path: &str) -> Resp {
        send(self.addr, "GET", path, &[], None)
    }

    fn post(&self, command: &str, body: &Value) -> Resp {
        send(self.addr, "POST", &format!("/daemon/commands/{command}"), &[JSON], Some(body.to_string().as_bytes()))
    }

    fn rows(&self, view: &str, query: &str) -> Vec<Value> {
        let r = self.get(&format!("/daemon/views/{view}?{query}"));
        assert_eq!(r.status, 200, "{view}: {}", r.text());
        r.json()["rows"].as_array().unwrap().clone()
    }

    fn submit_body(&self, pause: u32) -> Value {
        let ctx = self.dir.join("ctx/project");
        std::fs::write(ctx.join("pause.txt"), format!("{pause}\n")).unwrap();
        json!({"runbook_id": RUNBOOK, "intake": "a note on tea", "confinement": "local", "context_path": ctx})
    }

    /// Submit a run whose writer sleeps `pause` seconds; the runtime has taken it up when this returns.
    fn submit(&self, pause: u32) -> String {
        let r = self.post("submit-run", &self.submit_body(pause));
        assert_eq!((r.status, r.json()["outcome"].clone()), (202, json!("submitted")), "{}", r.text());
        let run = r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_string();
        until("the run to be taken up", 40, || self.state_of(&run) != "Submitted");
        run
    }

    fn wait_waiting(&self, run: &str) {
        until("the run to wait on a decision", 40, || self.state_of(run) == "WaitingOnDecision");
    }

    fn state_of(&self, run: &str) -> String {
        self.rows("run", &format!("run_id={run}"))[0]["state"].as_str().unwrap().to_string()
    }

    /// Answer what the run asks until it is over: the person step with `performed`, every other decision with `approve`.
    /// The outcome the journal holds for the run, once it ended. The daemon exits when its last run ends,
    /// so the end is read from the journal and not from a view.
    fn outcome_of(&self, run: &str) -> Option<String> {
        self.facts("run_ended").into_iter().find(|f| f["run_id"] == run).map(|f| f["outcome"].as_str().unwrap().to_string())
    }

    /// One request to a daemon that leaves when its last run ends. `None`: no answer came, and the journal
    /// already holds `daemon.drained`, so the daemon had finished its drain and closed its listener. No
    /// answer from a daemon that had not drained is a defect and fails the case.
    fn ask(&self, method: &str, path: &str, headers: &[(&str, &str)], body: Option<&[u8]>) -> Option<Resp> {
        match try_send(self.addr, method, path, headers, body) {
            Ok(r) => Some(r),
            Err(e) => {
                assert!(!self.facts("daemon.drained").is_empty(), "no answer from a daemon that had not drained: {e}: {}", self.stderr());
                None
            }
        }
    }

    fn settle(&self, run: &str) -> String {
        self.settle_all(&[run.to_string()]).remove(0)
    }

    /// Answer every open decision of `runs` until each has ended; the outcomes in the order of `runs`.
    fn settle_all(&self, runs: &[String]) -> Vec<String> {
        let mut answered = std::collections::BTreeSet::new();
        until("every run to be over", 50, || {
            if runs.iter().all(|r| self.outcome_of(r).is_some()) {
                return true;
            }
            for run in runs {
                if self.outcome_of(run).is_some() {
                    continue;
                }
                let Some(open) = self.ask("GET", &format!("/daemon/views/open-decisions?run_id={run}"), &[], None) else {
                    return false;
                };
                assert_eq!(open.status, 200, "open-decisions: {}", open.text());
                for d in open.json()["rows"].as_array().unwrap() {
                    let id = d["request_id"].as_str().unwrap().to_string();
                    if !answered.insert(id.clone()) {
                        continue;
                    }
                    let review = d["options"].as_array().unwrap().iter().any(|o| o["answer"].as_str().is_some_and(|a| a.contains("result")));
                    let answer = if review { "performed" } else { "approve" };
                    let body = json!({"run_id": run, "request_id": id, "answer": answer, "answered_by": "tester"}).to_string();
                    let Some(r) = self.ask("POST", "/daemon/commands/answer-decision", &[JSON], Some(body.as_bytes())) else {
                        return false;
                    };
                    assert_eq!(r.status, 202, "{}", r.text());
                }
            }
            false
        });
        runs.iter().map(|r| self.outcome_of(r).unwrap()).collect()
    }

    fn mcp_submit(&self) -> Value {
        let params = json!({"name": "submit_run", "arguments": {"runbook_id": RUNBOOK, "intake": "x", "confinement": "local"},
            "_meta": {"io.modelcontextprotocol/protocolVersion": "2026-07-28", "io.modelcontextprotocol/clientInfo": {"name": "test", "version": "0"}, "io.modelcontextprotocol/clientCapabilities": {}}});
        let body = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": params});
        let headers = [JSON, ("MCP-Protocol-Version", "2026-07-28"), ("Mcp-Method", "tools/call"), ("Mcp-Name", "submit_run")];
        let r = send(self.addr, "POST", "/mcp", &headers, Some(body.to_string().as_bytes()));
        assert_eq!(r.status, 200, "{}", r.text());
        r.json()["result"].clone()
    }
}

fn mcp_call(addr: SocketAddr, method: &str, name: Option<&str>, params: Value) -> Resp {
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
    let mut headers = vec![JSON, ("MCP-Protocol-Version", "2026-07-28"), ("Mcp-Method", method)];
    if let Some(n) = name {
        headers.push(("Mcp-Name", n));
    }
    send(addr, "POST", "/mcp", &headers, Some(body.to_string().as_bytes()))
}

/// The head of a response to a GET that never ends (an event stream): status line and headers only.
fn stream_head(addr: SocketAddr, path: &str, extra: &[(&str, &str)]) -> Vec<(String, String)> {
    let mut tcp = TcpStream::connect(addr).unwrap();
    tcp.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let mut head = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nAccept: text/event-stream\r\n");
    for (k, v) in extra {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    tcp.write_all(head.as_bytes()).unwrap();
    let mut raw = Vec::new();
    let mut buf = [0u8; 512];
    loop {
        let n = tcp.read(&mut buf).unwrap();
        assert!(n > 0, "closed before the head: {:?}", String::from_utf8_lossy(&raw));
        raw.extend_from_slice(&buf[..n]);
        if raw.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    let text = String::from_utf8_lossy(&raw).into_owned();
    text.lines().skip(1).take_while(|l| !l.is_empty()).filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect()
}

fn header<'a>(h: &'a [(String, String)], name: &str) -> Option<&'a str> {
    h.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}

/// The reasons of every `runbook.run-orphaned` event the journal holds.
fn reasons(s: &Serve) -> Vec<String> {
    s.journal().into_iter().filter(|f| f["fact"] == "event_recorded" && f["kind"] == "runbook.run-orphaned").map(|f| f["record"]["reason"].as_str().unwrap().to_string()).collect()
}

#[test]
fn a_stop_during_a_live_run_lets_it_end_with_its_outcome_and_refuses_a_submit_meanwhile() {
    let t = Instant::now();
    let mut s = Serve::start("drain-live", &[]);
    let run = s.submit(2);
    assert_eq!(s.get("/daemon/views/runs").header(DRAIN_HEADER), None, "not draining yet");
    s.signal("TERM");
    s.wait_draining();
    assert!(s.child.try_wait().unwrap().is_none(), "the daemon went on serving: {}", s.stderr());

    // Reads are served and say the daemon is draining, with the count of live runs.
    let runs = s.get("/daemon/views/runs");
    assert_eq!(runs.status, 200, "{}", runs.text());
    assert_eq!(runs.header(DRAIN_HEADER), Some("1"), "{:?}", runs.headers);
    assert_eq!(runs.json()["rows"].as_array().unwrap().len(), 1);

    // A submit is refused and says why; so is the MCP tool, with the same body.
    let refused = s.post("submit-run", &json!({"runbook_id": RUNBOOK, "intake": "x", "confinement": "local"}));
    assert_eq!(refused.status, 503, "{}", refused.text());
    let body = refused.json();
    assert_eq!((body["refused"].as_str(), body["live_runs"].as_u64()), (Some("draining"), Some(1)), "{body}");
    assert!(body["message"].as_str().unwrap().contains("draining"), "{body}");
    let tool = s.mcp_submit();
    assert_eq!((tool["isError"].clone(), tool["_meta"]["runbook/http-status"].clone()), (json!(true), json!(503)), "{tool}");
    assert_eq!(tool["structuredContent"], body, "the tool gives the route's body");
    assert_eq!(s.rows("runs", "").len(), 1, "the refused submits made no run");

    // Decisions are taken during the drain; the run ends with its own outcome.
    assert_eq!(s.settle(&run), "delivered");
    let (code, took) = s.exit_within(Duration::from_secs(15));
    assert_eq!(code, Some(0), "{}", s.stderr());
    println!("exit {took:?} after the run ended; whole case {:?}", t.elapsed());

    let journal = s.journal();
    let at = |name: &str| journal.iter().position(|f| f["fact"] == name).unwrap_or_else(|| panic!("no {name} in the journal"));
    assert!(at("daemon.draining") < at("daemon.drained"));
    assert_eq!(s.facts("daemon.draining")[0]["run_ids"], json!([run]));
    assert!(journal.iter().all(|f| f["outcome"] != "orphaned"), "no run was orphaned");
}

#[test]
fn a_second_signal_ends_the_daemon_at_once_and_the_live_run_is_orphaned_as_stopped_while_it_ran() {
    let mut s = Serve::start("drain-second", &[]);
    let run = s.submit(30);
    s.signal("TERM");
    s.wait_draining();
    assert!(s.child.try_wait().unwrap().is_none(), "the first signal ended it: {}", s.stderr());
    std::thread::sleep(Duration::from_millis(300));
    assert!(s.child.try_wait().unwrap().is_none(), "the drain ended while a run lived");
    s.signal("INT");
    let (code, took) = s.exit_within(Duration::from_secs(10));
    println!("exit {took:?} after the second signal");
    assert_eq!(code, Some(EXIT_ORPHANED), "{}", s.stderr());
    assert!(s.facts("daemon.drained").is_empty(), "a drain that was cut off is not drained");

    // The journal has the run ended as orphaned, with the reason the stop is the cause.
    let ended: Vec<Value> = s.facts("run_ended");
    assert_eq!((ended.len(), ended[0]["run_id"].as_str(), ended[0]["outcome"].as_str()), (1, Some(run.as_str()), Some("orphaned")), "{ended:?}");
    let record = s.journal().into_iter().find(|f| f["fact"] == "event_recorded" && f["kind"] == "runbook.run-orphaned").expect("an orphaned event");
    let reason = record["record"]["reason"].as_str().unwrap().to_string();
    assert!(reason.contains("stopped while") && !reason.contains("started again"), "{reason}");
}

#[test]
fn a_stop_with_no_live_run_exits_at_once() {
    let mut s = Serve::start("drain-idle", &[]);
    s.signal("TERM");
    let (code, took) = s.exit_within(Duration::from_secs(5));
    println!("exit {took:?} after the signal");
    assert_eq!(code, Some(0), "{}", s.stderr());
    assert_eq!(s.facts("daemon.draining")[0]["run_ids"], json!([]));
    assert_eq!(s.facts("daemon.drained").len(), 1);
}

#[test]
fn a_drain_bound_ends_a_drain_that_does_not_finish_and_the_run_is_orphaned_as_stopped_while_it_ran() {
    let mut s = Serve::start("drain-bound", &["--drain-seconds", "1"]);
    let run = s.submit(30);
    s.signal("TERM");
    s.wait_draining();
    let (code, took) = s.exit_within(Duration::from_secs(10));
    println!("exit {took:?} after the signal, bound 1 s");
    assert_eq!(code, Some(EXIT_ORPHANED), "{}", s.stderr());
    assert!(took >= Duration::from_millis(500), "the bound was not waited: {took:?}");
    assert!(s.facts("daemon.drained").is_empty());
    let ended = s.facts("run_ended");
    assert_eq!((ended.len(), ended[0]["run_id"].as_str(), ended[0]["outcome"].as_str()), (1, Some(run.as_str()), Some("orphaned")), "{ended:?}");
    let record = s.journal().into_iter().find(|f| f["fact"] == "event_recorded" && f["kind"] == "runbook.run-orphaned").expect("an orphaned event");
    assert!(record["record"]["reason"].as_str().unwrap().contains("stopped while"), "{record}");
}

#[test]
fn the_serve_option_names_the_bound_in_seconds() {
    let out = Command::new(BIN).args(["serve", "--help"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("--drain-seconds <SECONDS>"), "{text}");
}
#[test]
fn three_runs_at_once_all_end_with_their_outcome_and_the_daemon_exits_0_after_the_last() {
    let mut s = Serve::start("drain-three", &[]);
    let runs: Vec<String> = [1, 2, 3].iter().map(|p| s.submit(*p)).collect();
    s.signal("TERM");
    s.wait_draining();
    let draining = &s.facts("daemon.draining")[0];
    assert_eq!(draining["run_ids"].as_array().unwrap().len(), 3, "{draining}");
    let outcomes = s.settle_all(&runs);
    assert_eq!(outcomes, vec!["delivered"; 3], "{}", s.stderr());
    let (code, _) = s.exit_within(Duration::from_secs(15));
    assert_eq!(code, Some(0), "{}", s.stderr());
    let journal = s.journal();
    let drained = journal.iter().position(|f| f["fact"] == "daemon.drained").expect("daemon.drained");
    let last_end = journal.iter().rposition(|f| f["fact"] == "run_ended").unwrap();
    assert!(drained > last_end, "drained came before the last run ended");
    assert_eq!(s.facts("daemon.draining").len() + s.facts("daemon.drained").len(), 2);
    assert!(reasons(&s).is_empty(), "{:?}", reasons(&s));
}

#[test]
fn a_submit_racing_the_signal_leaves_no_run_submitted_after_draining_and_none_unended_at_drained() {
    let mut s = Serve::start("drain-race", &[]);
    let accepted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));
    let (addr, body) = (s.addr, s.submit_body(0));
    let acc = accepted.clone();
    let submitter = std::thread::spawn(move || {
        for _ in 0..200 {
            // the daemon going away makes `send` panic: that ends the thread
            let r = send(addr, "POST", "/daemon/commands/submit-run", &[JSON], Some(body.to_string().as_bytes()));
            if r.status == 202 {
                acc.lock().unwrap().push(r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_string());
            } else {
                assert_eq!(r.status, 503, "{}", r.text());
                break;
            }
        }
    });
    until("a first run accepted", 20, || !accepted.lock().unwrap().is_empty());
    std::thread::sleep(Duration::from_millis(30));
    s.signal("TERM");
    s.wait_draining();
    let _ = submitter.join();
    let runs = accepted.lock().unwrap().clone();
    let outcomes = s.settle_all(&runs);
    assert!(outcomes.iter().all(|o| o == "delivered"), "{outcomes:?}");
    let (code, _) = s.exit_within(Duration::from_secs(20));
    assert_eq!(code, Some(0), "{}", s.stderr());
    let journal = s.journal();
    let draining = journal.iter().position(|f| f["fact"] == "daemon.draining").unwrap();
    let late: Vec<&Value> = journal.iter().skip(draining).filter(|f| f["fact"] == "run_submitted").collect();
    assert!(late.is_empty(), "a run was submitted after daemon.draining: {late:?}");
    let submitted: Vec<String> = s.facts("run_submitted").iter().map(|f| f["run_id"].as_str().unwrap().to_string()).collect();
    assert_eq!(submitted.len(), runs.len(), "accepted {} journal {}", runs.len(), submitted.len());
    assert!(draining < journal.iter().position(|f| f["fact"] == "daemon.drained").unwrap());
}

#[test]
fn drain_seconds_zero_orphans_the_live_run_at_once_as_stopped() {
    let mut s = Serve::start("drain-zero", &["--drain-seconds", "0"]);
    let run = s.submit(30);
    s.signal("TERM");
    let (code, took) = s.exit_within(Duration::from_secs(10));
    println!("exit {took:?}");
    assert_eq!(code, Some(EXIT_ORPHANED), "{}", s.stderr());
    assert_eq!(s.outcome_of(&run).as_deref(), Some("orphaned"));
    assert_eq!(reasons(&s), vec![STOPPED.to_string()]);
    assert!(s.facts("daemon.drained").is_empty());
}

#[test]
fn drain_seconds_zero_with_no_live_run_drains_and_exits_0() {
    let mut s = Serve::start("drain-zero-idle", &["--drain-seconds", "0"]);
    s.signal("TERM");
    let (code, _) = s.exit_within(Duration::from_secs(5));
    assert_eq!(code, Some(0), "{}", s.stderr());
    assert_eq!(s.facts("daemon.drained").len(), 1);
}

#[test]
fn a_very_large_drain_seconds_does_not_crash_a_stop_with_no_live_run() {
    let mut s = Serve::start("drain-huge", &["--drain-seconds", "18446744073709551615"]);
    s.signal("TERM");
    let (code, _) = s.exit_within(Duration::from_secs(5));
    assert_eq!(code, Some(0), "{}", s.stderr());
    assert_eq!(s.facts("daemon.drained").len(), 1, "{}", s.stderr());
}

#[test]
fn sigint_then_sigterm_orphans_as_stopped_and_writes_each_drain_fact_once() {
    let mut s = Serve::start("drain-int-term", &[]);
    let run = s.submit(30);
    s.signal("INT");
    s.wait_draining();
    std::thread::sleep(Duration::from_millis(200));
    assert!(s.child.try_wait().unwrap().is_none());
    s.signal("TERM");
    let (code, took) = s.exit_within(Duration::from_secs(10));
    println!("exit {took:?}");
    assert_eq!(code, Some(EXIT_ORPHANED), "{}", s.stderr());
    assert_eq!(s.outcome_of(&run).as_deref(), Some("orphaned"));
    assert_eq!(reasons(&s), vec![STOPPED.to_string()]);
    assert_eq!((s.facts("daemon.draining").len(), s.facts("daemon.drained").len()), (1, 0));
}

#[test]
fn two_sigterm_orphan_as_stopped_and_a_third_signal_changes_nothing() {
    let mut s = Serve::start("drain-term-term", &[]);
    let run = s.submit(30);
    s.signal("TERM");
    s.wait_draining();
    std::thread::sleep(Duration::from_millis(200));
    s.signal("TERM");
    let (code, _) = s.exit_within(Duration::from_secs(10));
    assert_eq!(code, Some(EXIT_ORPHANED), "{}", s.stderr());
    assert_eq!(s.outcome_of(&run).as_deref(), Some("orphaned"));
    assert_eq!(reasons(&s), vec![STOPPED.to_string()]);
    assert_eq!(s.facts("daemon.draining").len(), 1);
}

#[test]
fn a_cut_off_that_orphans_a_live_run_exits_with_the_code_that_says_so_and_a_clean_drain_exits_0() {
    let mut s = Serve::start("drain-exit-code", &["--drain-seconds", "1"]);
    let run = s.submit(30);
    s.signal("TERM");
    let (code, _) = s.exit_within(Duration::from_secs(10));
    assert_eq!(s.outcome_of(&run).as_deref(), Some("orphaned"));
    assert_eq!(code, Some(EXIT_ORPHANED), "a run was orphaned: {}", s.stderr());
    assert!(![0, 1, 2, 64].contains(&EXIT_ORPHANED), "the code is one no other `runbook serve` exit uses");

    // The bound is set and the run ends inside it: nothing is left, so the exit is 0.
    let mut s = Serve::start("drain-exit-clean", &["--drain-seconds", "20"]);
    let run = s.submit(1);
    s.signal("TERM");
    assert_eq!(s.settle(&run), "delivered");
    let (code, _) = s.exit_within(Duration::from_secs(15));
    assert_eq!(code, Some(0), "{}", s.stderr());
}

#[test]
fn second_signal_withdraws_the_open_decision_of_a_run_that_waits() {
    let mut s = Serve::start("drain-withdraw", &[]);
    let run = s.submit(0);
    s.wait_waiting(&run);
    let open = s.rows("open-decisions", &format!("run_id={run}"));
    assert!(!open.is_empty());
    s.signal("TERM");
    s.wait_draining();
    std::thread::sleep(Duration::from_millis(200));
    assert!(s.child.try_wait().unwrap().is_none(), "the drain ended with a run that waits");
    s.signal("TERM");
    let (code, _) = s.exit_within(Duration::from_secs(10));
    assert_eq!(code, Some(EXIT_ORPHANED), "{}", s.stderr());
    assert_eq!(s.outcome_of(&run).as_deref(), Some("orphaned"));
    assert_eq!(reasons(&s), vec![STOPPED.to_string()]);
    let withdrawn = s.facts("decision_withdrawn").len();
    assert_eq!(withdrawn, open.len(), "withdrawn {withdrawn} of {}", open.len());
}

#[test]
fn a_restart_after_a_kill_in_the_drain_reads_the_journal_orphans_with_the_start_reason_and_takes_submits() {
    let dir = prepare("drain-kill9");
    let mut s = Serve::launch(dir.clone(), &[]);
    let run = s.submit(30);
    s.signal("TERM");
    s.wait_draining();
    s.signal("KILL");
    let (code, _) = s.exit_within(Duration::from_secs(5));
    assert_eq!(code, None, "killed by a signal");
    assert!(s.facts("daemon.drained").is_empty());
    drop(s);
    let s2 = Serve::launch(dir, &[]);
    assert_eq!(s2.state_of(&run), "Ended");
    assert_eq!(s2.rows("run", &format!("run_id={run}"))[0]["outcome"], json!("orphaned"));
    assert_eq!(reasons(&s2), vec!["the daemon started again and the runtime holds no live run of this id".to_string()]);
    let r = s2.post("submit-run", &s2.submit_body(0));
    assert_eq!(r.status, 202, "a restarted daemon refuses a submit: {}", r.text());
    assert_eq!(r.header(DRAIN_HEADER), None);
}

#[test]
fn a_restart_after_a_cut_off_adds_no_second_orphan_event_and_takes_submits() {
    let dir = prepare("drain-cutoff-restart");
    let mut s = Serve::launch(dir.clone(), &["--drain-seconds", "1"]);
    let run = s.submit(30);
    s.signal("TERM");
    let _ = s.exit_within(Duration::from_secs(10));
    assert_eq!(s.outcome_of(&run).as_deref(), Some("orphaned"));
    drop(s);
    let s2 = Serve::launch(dir, &[]);
    assert_eq!(reasons(&s2), vec![STOPPED.to_string()]);
    let r = s2.post("submit-run", &s2.submit_body(0));
    assert_eq!(r.status, 202, "{}", r.text());
    assert_eq!(s2.get("/daemon/views/runs").header(DRAIN_HEADER), None);
}

#[test]
fn every_answer_during_the_drain_carries_the_header() {
    let s = Serve::start("drain-headers", &["--ui"]);
    let run = s.submit(30);
    s.signal("TERM");
    s.wait_draining();
    let mut missing: Vec<String> = vec![];
    let mut check = |what: &str, h: Option<&str>| {
        if h != Some("1") {
            missing.push(format!("{what}: {h:?}"));
        }
    };
    let r = s.get("/no/such/route");
    check("404 fallback", r.header(DRAIN_HEADER));
    let r = send(s.addr, "DELETE", "/daemon/views/runs", &[], None);
    check(&format!("{} method", r.status), r.header(DRAIN_HEADER));
    let r = s.post("cancel-run", &json!({"run_id": "no-such-run"}));
    check(&format!("{} command", r.status), r.header(DRAIN_HEADER));
    let r = s.get("/daemon/views/run?run_id=x");
    check(&format!("{} view", r.status), r.header(DRAIN_HEADER));
    let r = mcp_call(s.addr, "tools/list", None, json!({}));
    check(&format!("{} mcp", r.status), r.header(DRAIN_HEADER));
    let h = stream_head(s.addr, &format!("/daemon/views/events?run_id={run}"), &[]);
    check("event stream", header(&h, DRAIN_HEADER));
    let r = s.get("/");
    check(&format!("{} ui", r.status), r.header(DRAIN_HEADER));
    assert!(missing.is_empty(), "the header X-Runbook-Draining: 1 is missing on: {missing:?}");
}

#[test]
fn a_401_during_the_drain_carries_the_header() {
    let dir = prepare("drain-token");
    std::fs::write(dir.join("token"), "secret-token-secret-token-0123456789\n").unwrap();
    std::fs::set_permissions(dir.join("token"), std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();
    let tf = dir.join("token");
    let mut s = Serve::launch(dir, &["--token-file", tf.to_str().unwrap()]);
    let auth = [("Authorization", "Bearer secret-token-secret-token-0123456789")];
    let r = send(s.addr, "GET", "/daemon/views/runs", &auth, None);
    assert_eq!(r.status, 200);
    let ctx = s.dir.join("ctx/project");
    std::fs::write(ctx.join("pause.txt"), "30\n").unwrap();
    let body = json!({"runbook_id": RUNBOOK, "intake": "x", "confinement": "local", "context_path": ctx});
    let r = send(s.addr, "POST", "/daemon/commands/submit-run", &[JSON, auth[0]], Some(body.to_string().as_bytes()));
    assert_eq!(r.status, 202, "{}", r.text());
    s.signal("TERM");
    s.wait_draining();
    let r = send(s.addr, "GET", "/daemon/views/runs", &[], None);
    assert_eq!(r.status, 401);
    assert_eq!(r.header(DRAIN_HEADER), Some("1"), "{:?}", r.headers);
    // The guard's own refusal of a foreign `Origin` is 403 and carries it too.
    let r = send(s.addr, "GET", "/daemon/views/runs", &[auth[0], ("Origin", "http://foreign.example")], None);
    assert_eq!(r.status, 403, "{}", r.text());
    assert_eq!(r.header(DRAIN_HEADER), Some("1"), "{:?}", r.headers);
    s.signal("TERM");
    let _ = s.exit_within(Duration::from_secs(10));
}

#[test]
fn the_header_counts_live_runs_and_a_cancel_of_the_last_one_ends_the_drain() {
    let mut s = Serve::start("drain-count", &[]);
    let a = s.submit(30);
    let b = s.submit(30);
    s.signal("TERM");
    s.wait_draining();
    assert_eq!(s.get("/daemon/views/runs").header(DRAIN_HEADER), Some("2"));
    let r = s.post("cancel-run", &json!({"run_id": a}));
    assert_eq!(r.status, 202, "{}", r.text());
    assert_eq!(r.header(DRAIN_HEADER), Some("1"), "the answer of a cancel: {:?}", r.headers);
    until("the count to fall to 1", 5, || s.get("/daemon/views/runs").header(DRAIN_HEADER) == Some("1"));
    assert!(s.child.try_wait().unwrap().is_none());
    let r = s.post("cancel-run", &json!({"run_id": b}));
    assert_eq!(r.status, 202, "{}", r.text());
    let (code, _) = s.exit_within(Duration::from_secs(15));
    assert_eq!(code, Some(0), "{}", s.stderr());
    assert_eq!(s.facts("daemon.drained").len(), 1);
    assert!(reasons(&s).is_empty());
}

#[test]
fn which_drain_seconds_values_let_an_idle_stop_drain_and_exit_0() {
    let mut bad: Vec<String> = vec![];
    for v in ["86400", "1000000000000", "1000000000000000000", "9223372036854775807", "18446744073709551615"] {
        let mut s = Serve::start(&format!("drain-sec-{v}"), &["--drain-seconds", v]);
        s.signal("TERM");
        let (code, _) = s.exit_within(Duration::from_secs(5));
        if code != Some(0) || s.facts("daemon.drained").len() != 1 {
            bad.push(format!("{v}: exit {code:?}, drained {}", s.facts("daemon.drained").len()));
        }
    }
    assert!(bad.is_empty(), "an idle stop did not drain and exit 0 with: {bad:?}");
}

#[test]
fn an_open_event_stream_stays_open_in_the_drain_and_does_not_hold_the_exit_back() {
    let mut s = Serve::start("drain-stream", &[]);
    let run = s.submit(30);
    let mut tcp = TcpStream::connect(s.addr).unwrap();
    tcp.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let head = format!("GET /daemon/views/events?run_id={run} HTTP/1.1\r\nHost: {}\r\nAccept: text/event-stream\r\n\r\n", s.addr);
    tcp.write_all(head.as_bytes()).unwrap();
    let mut buf = [0u8; 4096];
    assert!(tcp.read(&mut buf).unwrap() > 0);
    s.signal("TERM");
    s.wait_draining();
    std::thread::sleep(Duration::from_millis(300));
    assert!(s.child.try_wait().unwrap().is_none());
    let r = s.post("submit-run", &s.submit_body(0));
    assert_eq!((r.status, r.header(DRAIN_HEADER)), (503, Some("1")), "{}", r.text());
    let c = s.post("cancel-run", &json!({"run_id": run}));
    assert_eq!(c.status, 202, "{}", c.text());
    // the client does not read: the stream is not closed by the client and the daemon still exits
    let (code, took) = s.exit_within(Duration::from_secs(6));
    println!("exit {took:?}");
    assert_eq!(code, Some(0), "{}", s.stderr());
}

#[test]
fn every_submit_during_the_drain_is_503_whatever_else_is_wrong_with_it() {
    let s = Serve::start("drain-503-first", &[]);
    let _run = s.submit(30);
    s.signal("TERM");
    s.wait_draining();
    let unknown = s.post("submit-run", &json!({"runbook_id": "no-such-runbook", "intake": "x", "confinement": "local"}));
    let outside = s.post("submit-run", &json!({"runbook_id": RUNBOOK, "intake": "x", "confinement": "local", "context_path": "/nonexistent-dir"}));
    let statuses = (unknown.status, outside.status);
    assert_eq!(statuses, (503, 503), "{} | {}", unknown.text(), outside.text());
}

#[test]
fn a_closed_standard_error_does_not_end_the_drain() {
    let mut s = Serve::launch_with(prepare("drain-closed-stderr"), &[], true);
    let run = s.submit(2);
    s.signal("TERM");
    std::thread::sleep(Duration::from_millis(500));
    let status = s.child.try_wait().unwrap();
    assert!(status.is_none(), "the daemon exited with a run live: {status:?}, journal facts {:?}", s.journal().iter().map(|f| f["fact"].as_str().unwrap().to_string()).collect::<Vec<_>>());
    assert_eq!(s.facts("daemon.draining").len(), 1);
    assert_eq!(s.settle_all(&[run]), vec!["delivered"]);
    let (code, _) = s.exit_within(Duration::from_secs(15));
    assert_eq!(code, Some(0));
    assert_eq!(s.facts("daemon.drained").len(), 1);
}

#[test]
fn a_huge_drain_seconds_still_lets_a_live_run_end_with_its_outcome() {
    for v in ["1000000000000000000", "18446744073709551615"] {
        let mut s = Serve::start(&format!("drain-huge-live-{v}"), &["--drain-seconds", v]);
        let run = s.submit(1);
        s.signal("TERM");
        std::thread::sleep(Duration::from_millis(500));
        assert!(s.child.try_wait().unwrap().is_none(), "--drain-seconds {v}: the daemon exited with a run live: {}", s.stderr());
        assert_eq!(s.settle_all(&[run]), vec!["delivered"], "--drain-seconds {v}: {}", s.stderr());
        let (code, _) = s.exit_within(Duration::from_secs(15));
        assert_eq!(code, Some(0), "--drain-seconds {v}: {}", s.stderr());
    }
}

#[test]
fn a_stop_that_panics_orphans_the_live_runs_as_stopped_and_reports_that_it_failed() {
    let fx = super::commands::Fx::new("drain-panic");
    let run = fx.submit("hold");
    fx.wait_state(&run, "Running");
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let end = rt.block_on(runbook::serve::guarded_stop(fx.d(), async { panic!("the stop future failed") }));
    assert_eq!((end.orphaned, end.failed), (1, true), "{end:?}");
    assert_eq!(fx.state_of(&run), "Ended");
    let journal = std::fs::read_to_string(fx.dir.join("state/journal.jsonl")).unwrap();
    let facts: Vec<Value> = journal.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let reasons: Vec<&str> = facts.iter().filter(|f| f["fact"] == "event_recorded" && f["kind"] == "runbook.run-orphaned").map(|f| f["record"]["reason"].as_str().unwrap()).collect();
    assert_eq!(reasons, vec![STOPPED]);

    // A stop that ends without a panic reports what it orphaned and says it did not fail.
    let fx = super::commands::Fx::new("drain-no-panic");
    let end = rt.block_on(runbook::serve::guarded_stop(fx.d(), async { 3 }));
    assert_eq!((end.orphaned, end.failed), (3, false), "{end:?}");
}
