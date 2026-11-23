//! The level of a submit that names none: stated by the person who starts the daemon (`--default-confinement`, or
//! `confinement` of `runbook.toml`), never read from the floor, never below it.

use super::http::{exits_within, send};
use super::PRINCIPAL;
use runbook::serve::STUB_DECISION;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
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
    let (mut cmd, dir) = serve(tag, args, file);
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
    let (row, record) = submit_naming_none(&l);
    assert_eq!((row["confinement"].clone(), row["confinement_source"].clone()), (json!("linux"), json!("built-in")), "a floor of local gives no default of local: {row}");
    assert_eq!(record["confinement_source"], "built-in", "{record}");
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

