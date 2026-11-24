//! A stop of the daemon does not orphan a live run: on SIGTERM or SIGINT `runbook serve` drains. It refuses new
//! submits, keeps serving reads and decisions, lets every live run go on to its end and exits 0 when none is left;
//! a second signal, or `--drain-seconds`, ends it at once and the runs are orphaned with an event that says the
//! daemon was stopped while they ran. Each case starts `runbook serve --runtime loom` as a process on a free port,
//! over the fixture runbook of `engine.rs` (a writer program that sleeps the seconds of `pause.txt` in the
//! context, then a person step). No model call, no network beyond loopback.

use super::http::{send, Resp};
use super::state::scratch;
use serde_json::{json, Value};
use std::io::BufRead;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_runbook");
const RUNBOOK: &str = "engine-demo";
const JSON: (&str, &str) = ("Content-Type", "application/json");
const DRAIN_HEADER: &str = "x-runbook-draining";

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

impl Serve {
    /// `runbook serve --runtime loom` over a copy of the fixture runbook; `extra` are more serve options.
    fn start(tag: &str, extra: &[&str]) -> Serve {
        let dir = scratch(tag);
        let home = dir.join("home");
        copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine-runbook"), &home);
        let binding = home.join("bindings/engine-draft.yaml");
        let text = std::fs::read_to_string(&binding).unwrap();
        assert!(text.contains("principal: reviewer"));
        std::fs::write(&binding, text.replace("principal: reviewer", &format!("principal: {}", super::PRINCIPAL))).unwrap();
        std::fs::create_dir_all(dir.join("ctx/project")).unwrap();
        let mut cmd = Command::new(BIN);
        cmd.args(["serve", "--principal", super::PRINCIPAL, "--runtime", "loom", "--min-confinement", "local", "--mcp", "--listen", "127.0.0.1:0"])
            .arg("--runbook-home")
            .arg(&home)
            .arg("--context-root")
            .arg(dir.join("ctx"))
            .arg("--state-dir")
            .arg(dir.join("state"))
            .args(extra);
        cmd.env_remove("RUNBOOK_TOKEN").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::from(std::fs::File::create(dir.join("stderr.log")).unwrap()));
        let mut child = cmd.spawn().unwrap();
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
        std::fs::read_to_string(self.dir.join("state/journal.jsonl")).unwrap_or_default().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
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

    /// Submit a run whose writer sleeps `pause` seconds; the run is Running when this returns.
    fn submit(&self, pause: u32) -> String {
        let ctx = self.dir.join("ctx/project");
        std::fs::write(ctx.join("pause.txt"), format!("{pause}\n")).unwrap();
        let body = json!({"runbook_id": RUNBOOK, "intake": "a note on tea", "confinement": "local", "context_path": ctx});
        let r = self.post("submit-run", &body);
        assert_eq!((r.status, r.json()["outcome"].clone()), (202, json!("submitted")), "{}", r.text());
        let run = r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_string();
        until("the run to be Running", 40, || self.state_of(&run) == "Running");
        run
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

    fn serving(&self) -> bool {
        std::net::TcpStream::connect(self.addr).is_ok()
    }

    fn settle(&self, run: &str) -> String {
        let mut answered = std::collections::BTreeSet::new();
        until(&format!("run {run} to be over"), 50, || {
            if self.outcome_of(run).is_some() {
                return true;
            }
            if !self.serving() {
                return false;
            }
            for d in self.rows("open-decisions", &format!("run_id={run}")) {
                let id = d["request_id"].as_str().unwrap().to_string();
                if !answered.insert(id.clone()) {
                    continue;
                }
                let review = d["options"].as_array().unwrap().iter().any(|o| o["answer"].as_str().is_some_and(|a| a.contains("result")));
                let answer = if review { "performed" } else { "approve" };
                let r = self.post("answer-decision", &json!({"run_id": run, "request_id": id, "answer": answer, "answered_by": "tester"}));
                assert_eq!(r.status, 202, "{}", r.text());
            }
            false
        });
        self.outcome_of(run).unwrap()
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
    assert_eq!(code, Some(0), "{}", s.stderr());
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
    assert_eq!(code, Some(0), "{}", s.stderr());
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
