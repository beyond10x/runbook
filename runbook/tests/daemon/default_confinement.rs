//! The level of a submit that names none: stated by the person who starts the daemon (`--default-confinement`, or
//! `confinement` of `runbook.toml`), never read from the floor, never below it.

use super::commands::config;
use super::http::{exits_within, send};
use super::state::scratch;
use super::PRINCIPAL;
use runbook::commands::{Daemon, Fault, Policy, SubmitRun};
use runbook::runtime::Confinement;
use runbook::serve::STUB_DECISION;
use runbook::stub::{Script, Step, StubRuntime};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

const BIN: &str = env!("CARGO_BIN_EXE_runbook");
const SUBMIT: &str = "/daemon/commands/submit-run";

pub(super) struct Live {
    pub(super) child: Child,
    pub(super) addr: SocketAddr,
    pub(super) dir: PathBuf,
}

impl Drop for Live {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// `runbook serve` on a loopback port of its own, with a `runbook.toml` of `file` when given, else an empty one.
fn serve(tag: &str, args: &[&str], file: Option<&str>) -> (Command, PathBuf) {
    let dir = super::state::scratch(tag);
    let config = dir.join("runbook.toml");
    std::fs::write(&config, file.unwrap_or("")).unwrap();
    let mut cmd = Command::new(BIN);
    cmd.arg("serve").args(["--principal", PRINCIPAL, "--listen", "127.0.0.1:0"]).arg("--state-dir").arg(dir.join("state")).args(args);
    cmd.env_remove("RUNBOOK_TOKEN").env("RUNBOOK_CONFIG", &config).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    (cmd, dir)
}

pub(super) fn live(tag: &str, args: &[&str], file: Option<&str>) -> Live {
    live_env(tag, args, file, &[])
}

/// `live` with the daemon's own environment changed: a variable set to a value, or removed when the value is none.
pub(super) fn live_env(tag: &str, args: &[&str], file: Option<&str>, env: &[(&str, Option<&str>)]) -> Live {
    let (mut cmd, dir) = serve(tag, args, file);
    for (name, value) in env {
        match value {
            Some(value) => cmd.env(name, value),
            None => cmd.env_remove(name),
        };
    }
    let mut child = cmd.spawn().unwrap();
    let out = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = BufReader::new(out).read_line(&mut line);
        let _ = tx.send(line);
    });
    let line = rx.recv_timeout(Duration::from_secs(10)).expect("no first line of output within 10 s");
    let addr = line.trim().strip_prefix("runbook: listening on http://").unwrap_or_else(|| panic!("the daemon did not start: {line:?}")).parse().unwrap();
    Live { child, addr, dir }
}

pub(super) fn post(l: &Live, body: &Value) -> (u16, Value) {
    let r = send(l.addr, "POST", SUBMIT, &[("Content-Type", "application/json")], Some(body.to_string().as_bytes()));
    (r.status, r.json())
}

pub(super) fn rows(l: &Live, path: &str) -> Vec<Value> {
    let r = send(l.addr, "GET", path, &[], None);
    assert_eq!(r.status, 200, "{path}: {}", r.text());
    r.json()["rows"].as_array().unwrap().clone()
}

/// Submit without `confinement`; the run's row and the record of its first event.
fn submit_naming_none(l: &Live) -> (Value, Value) {
    let (status, body) = post(l, &json!({"runbook_id": STUB_DECISION, "intake": "x"}));
    assert_eq!((status, body["outcome"].clone()), (202, json!("submitted")), "{body}");
    let run = body["published"][0]["payload"]["run_id"].as_str().unwrap().to_string();
    let row = rows(l, &format!("/daemon/views/run?run_id={run}"))[0].clone();
    let events = rows(l, &format!("/daemon/views/events?run_id={run}&after_position=0"));
    assert_eq!(events[0]["kind"], "runbook.submitted", "{events:?}");
    (row, events[0]["record"].clone())
}

#[test]
fn a_default_confinement_option_is_the_level_of_a_submit_that_names_none_and_the_record_says_so() {
    let l = live("dc-option", &["--min-confinement", "local", "--default-confinement", "local"], None);
    let (row, record) = submit_naming_none(&l);
    assert_eq!((row["confinement"].clone(), row["confinement_source"].clone()), (json!("local"), json!("daemon-default")), "{row}");
    assert_eq!((record["confinement"].clone(), record["confinement_source"].clone()), (json!("local"), json!("daemon-default")), "{record}");
    super::openapi::check_row("RunDetail", &row);
}

#[test]
fn the_confinement_of_runbook_toml_is_the_default_and_the_option_wins() {
    let file = "confinement = \"local\"\n";
    let l = live("dc-file", &["--min-confinement", "local"], Some(file));
    let (row, _) = submit_naming_none(&l);
    assert_eq!((row["confinement"].clone(), row["confinement_source"].clone()), (json!("local"), json!("daemon-default")), "the file's value: {row}");
    drop(l);
    let l = live("dc-file-option", &["--min-confinement", "local", "--default-confinement", "linux"], Some(file));
    let (row, _) = submit_naming_none(&l);
    assert_eq!((row["confinement"].clone(), row["confinement_source"].clone()), (json!("linux"), json!("daemon-default")), "the option wins over the file: {row}");
}

#[test]
fn a_default_below_the_floor_does_not_start_the_daemon_and_names_both_values() {
    for (args, file) in [(vec!["--default-confinement", "local"], None), (vec![], Some("confinement = \"local\"\n"))] {
        let (cmd, dir) = serve("dc-below", &args, file);
        let (status, out, err) = exits_within(cmd, Duration::from_secs(5));
        assert_eq!(status, Some(1), "{args:?}: {err}");
        assert!(err.starts_with("runbook serve: ") && err.contains("local") && err.contains("linux") && err.contains("--min-confinement"), "{err}");
        assert!(out.is_empty() && !dir.join("state").exists(), "nothing was announced or written: {out}");
    }
}

#[test]
fn the_floor_is_never_read_as_a_default() {
    let l = live("dc-floor", &["--min-confinement", "local"], None);
    let (status, body) = post(&l, &json!({"runbook_id": STUB_DECISION, "intake": "x"}));
    assert_eq!((status, body["refused"].clone()), (400, json!("bad request")), "a floor of local gives no default of local, and the binary has none: {body}");
    assert!(body["message"].as_str().unwrap_or("").contains("confinement"), "the refusal names the member: {body}");
    assert!(rows(&l, "/daemon/views/runs").is_empty(), "a refused submit writes no run");
    assert!(!l.dir.join("state/runs").read_dir().map(|mut d| d.next().is_some()).unwrap_or(false), "nor a run directory");
}

#[test]
fn a_submit_that_names_a_level_is_unchanged_by_the_default() {
    let l = live("dc-named", &["--min-confinement", "local", "--default-confinement", "linux"], None);
    let (status, body) = post(&l, &json!({"runbook_id": STUB_DECISION, "intake": "x", "confinement": "local"}));
    assert_eq!((status, body["outcome"].clone()), (202, json!("submitted")), "{body}");
    let run = body["published"][0]["payload"]["run_id"].as_str().unwrap();
    let row = rows(&l, &format!("/daemon/views/run?run_id={run}"))[0].clone();
    assert_eq!((row["confinement"].clone(), row["confinement_source"].clone()), (json!("local"), json!("submit")), "{row}");
    drop(l);
    let l = live("dc-named-low", &["--default-confinement", "linux"], None);
    let (status, body) = post(&l, &json!({"runbook_id": STUB_DECISION, "intake": "x", "confinement": "local"}));
    assert_eq!((status, body["outcome"].clone()), (409, json!("below-floor")), "{body}");
    assert!(rows(&l, "/daemon/views/runs").is_empty(), "a refused submit writes no run");
    assert!(!l.dir.join("state/runs").read_dir().map(|mut d| d.next().is_some()).unwrap_or(false), "nor a run directory");
}

fn stub_with_quick() -> Arc<StubRuntime> {
    let stub = Arc::new(StubRuntime::new());
    stub.script("quick", Script::new(vec![Step::End("done".into())]));
    stub.script("hold", Script::new(vec![Step::Sleep(60_000)]));
    stub
}

#[test]
fn a_daemon_with_no_default_refuses_a_submit_without_confinement_by_name() {
    for floor in [Confinement::Linux, Confinement::Local] {
        let dir = scratch(&format!("dc-nodefault-{}", floor.as_str()));
        std::fs::create_dir_all(dir.join("ctx")).unwrap();
        let mut cfg = config(&dir);
        cfg.confinement_floor = floor;
        let d = Daemon::start(cfg, stub_with_quick()).unwrap();
        let out = d.submit_run(SubmitRun { runbook_id: "quick".into(), intake: "x".into(), ..SubmitRun::default() });
        let refused_by_name = match &out {
            Err(Fault::Invalid(message)) => message.contains("confinement"),
            Ok(o) => o.is_refusal(),
            Err(_) => false,
        };
        let runs = d.runs().len();
        d.stop();
        assert!(refused_by_name && runs == 0, "floor {}: a daemon with neither --default-confinement nor a file value took a submit without `confinement` and made {runs} run(s): {out:?}", floor.as_str());
    }
}

#[test]
fn the_default_never_lowers_a_named_level_and_the_floor_holds_on_every_path() {
    // a default below the floor does not start the daemon, and nothing is written
    let dir = scratch("dc-floor-a");
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    let why = Daemon::start_with(config(&dir), Policy { default_confinement: Some(Confinement::Local), ..Policy::default() }, stub_with_quick()).err().map(|e| e.to_string()).unwrap();
    assert!(why.contains("local") && why.contains("linux"), "{why}");
    assert!(!dir.join("state").exists(), "nothing written by a daemon that did not start");

    // floor local, default local: a submit without a level runs at local and says where it came from; a named level wins and is never lowered
    let dir = scratch("dc-floor-b");
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    let mut cfg = config(&dir);
    cfg.confinement_floor = Confinement::Local;
    let policy = Policy { default_confinement: Some(Confinement::Local), ..Policy::default() };
    let d = Daemon::start_with(cfg, policy, stub_with_quick()).unwrap();
    let none = d.submit_run(SubmitRun { runbook_id: "hold".into(), intake: "x".into(), ..SubmitRun::default() }).unwrap().run_id.unwrap();
    let named = d.submit_run(SubmitRun { runbook_id: "hold".into(), intake: "x".into(), confinement: Some(Confinement::Linux), ..SubmitRun::default() }).unwrap().run_id.unwrap();
    let (a, b) = (d.run_detail(&none)[0].clone(), d.run_detail(&named)[0].clone());
    assert_eq!((a["confinement"].clone(), a["confinement_source"].clone()), (json!("local"), json!("daemon-default")), "{a}");
    assert_eq!((b["confinement"].clone(), b["confinement_source"].clone()), (json!("linux"), json!("submit")), "{b}");
    d.stop();
    drop(d);

    // after a restart under a higher floor: a submit that names a level below it, or none (the default is below it), starts nothing
    let mut cfg = config(&dir);
    cfg.confinement_floor = Confinement::Linux;
    let d = Daemon::start_with(cfg.clone(), Policy { default_confinement: Some(Confinement::Linux), ..Policy::default() }, stub_with_quick()).unwrap();
    let before = d.runs().len();
    let below = d.submit_run(SubmitRun { runbook_id: "hold".into(), intake: "x".into(), confinement: Some(Confinement::Local), ..SubmitRun::default() }).unwrap();
    assert_eq!((below.name, below.is_refusal()), ("below-floor", true), "{below:?}");
    assert_eq!(d.runs().len(), before, "a refused submit writes no run");
    assert!(d.events_of_daemon_after(0).iter().filter(|e| e["kind"] == "runbook.submitted" && e["record"]["confinement"] == "local").count() == 1, "only the first daemon's submit is recorded at local");
    let plain = d.submit_run(SubmitRun { runbook_id: "hold".into(), intake: "x".into(), ..SubmitRun::default() }).unwrap().run_id.unwrap();
    assert_eq!(d.run_detail(&plain)[0]["confinement"], "linux", "the default is the one this daemon was started with");
    d.stop();
}

#[test]
fn the_policy_comment_and_the_documents_say_what_the_code_does_about_a_missing_default() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let commands = std::fs::read_to_string(root.join("runbook/src/commands.rs")).unwrap();
    assert!(!commands.contains("none: linux."), "runbook/src/commands.rs: the comment of `Policy::default_confinement` says that a daemon without a default takes linux");
    for doc in ["docs/daemon-api.md", "docs/mcp.md", "docs/engine.md", "README.md"] {
        let text = std::fs::read_to_string(root.join(doc)).unwrap();
        for stale in ["else `linux`", "at `linux` when the daemon has none", "`built-in`"] {
            assert!(!text.contains(stale), "{doc} still says {stale}");
        }
    }
}

