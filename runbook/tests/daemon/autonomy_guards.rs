//! The guards of the autonomy level and the decider kind of a run, built in process: what the daemon records,
//! what it refuses before it writes, and who an answer may be recorded under. The runtime here is a script of its
//! own that asks and tries to answer as a decider would; the engine and the decider of the binary are the cases of
//! `autonomy.rs` (the binary) and of `src/loom_run/arbiter.rs`.

use super::commands::config;
use runbook::commands::{AnswerDecision, CancelRun, Daemon, Fault, Policy, SubmitRun};
use runbook::runtime::{AnswerFailure, Autonomy, DecisionRequest, EventSink, Finish, Recovery, RunbookInfo, Runtime, RuntimeFailure, StartRequest};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Asks one request for each try, then tries to answer each as `(by, principal)` and keeps what the daemon said.
struct Trying {
    tries: Vec<(&'static str, &'static str, &'static str)>,
    taken: Mutex<Vec<bool>>,
    started: Mutex<Vec<StartRequest>>,
}

impl Trying {
    fn new(tries: Vec<(&'static str, &'static str, &'static str)>) -> Arc<Trying> {
        Arc::new(Trying { tries, taken: Mutex::new(Vec::new()), started: Mutex::new(Vec::new()) })
    }
}

struct Rt(Arc<Trying>);

impl Runtime for Rt {
    fn recover(&self, _run_id: &str) -> Recovery {
        Recovery::Gone
    }
    fn knows_runbook(&self, _runbook: &str, _version: Option<&str>) -> bool {
        true
    }
    fn runbooks(&self) -> Vec<RunbookInfo> {
        Vec::new()
    }
    fn kind(&self) -> &'static str {
        "stub"
    }
    fn start(&self, request: StartRequest, sink: &dyn EventSink, cancel: &AtomicBool) -> Result<Finish, RuntimeFailure> {
        self.0.started.lock().unwrap().push(request);
        sink.started();
        for (id, _, _) in &self.0.tries {
            sink.decision_requested(DecisionRequest { request_id: (*id).into(), case_id: "c".into(), action: "notes.stage".into(), capability: "notes.stage".into(), question: "q".into(), options: vec![] });
        }
        for (id, by, principal) in &self.0.tries {
            let taken = sink.decision_answered(id, "approve", by, principal);
            self.0.taken.lock().unwrap().push(taken);
        }
        while !cancel.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(Finish::Cancelled)
    }
    fn answer(&self, _run_id: &str, _request_id: &str, _answer: &str, _principal: &str) -> Result<(), AnswerFailure> {
        Ok(())
    }
    fn cancel(&self, _run_id: &str) {}
}

fn policy(autonomy: Option<Autonomy>, decider: Option<&str>, kinds: &[&str]) -> Policy {
    Policy { autonomy, decider: decider.map(str::to_owned), kinds: kinds.iter().map(|k| (*k).to_owned()).collect(), ..Policy::default() }
}

fn start(dir: &std::path::Path, runtime: &Arc<Trying>, policy: Policy) -> Daemon {
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    Daemon::start_with(config(dir), policy, Arc::new(Rt(runtime.clone()))).unwrap()
}

fn submit(d: &Daemon, autonomy: Option<Autonomy>, decider: Option<&str>) -> Result<String, Fault> {
    d.submit_run(SubmitRun { runbook_id: "any".into(), intake: "x".into(), autonomy, decider: decider.map(str::to_owned), ..SubmitRun::default() }).map(|o| {
        assert_eq!(o.body()["outcome"], "submitted", "{}", o.body());
        o.run_id.unwrap()
    })
}

fn until(what: &str, mut done: impl FnMut() -> bool) {
    let limit = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(Instant::now() < limit, "gave up waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn decisions(d: &Daemon, run: &str) -> Vec<Value> {
    d.decisions_of_run(run)
}

#[test]
fn an_answer_of_a_decider_is_recorded_under_its_own_name_and_never_a_person_or_a_label_a_client_typed() {
    let dir = super::state::scratch("auto-labels");
    let rt = Trying::new(vec![("q1", "rule:approve", "decider:approve"), ("q2", "person:alice", "person:alice"), ("q3", "operator", "operator"), ("q4", "decider:model", "decider:model"), ("q5", "rule:", "x"), ("q6", "decider:", "x")]);
    let d = start(&dir, &rt, policy(Some(Autonomy::A5), Some("rule"), &["rule"]));
    let run = submit(&d, None, None).unwrap();
    until("the six tries", || rt.taken.lock().unwrap().len() == 6);
    assert_eq!(*rt.taken.lock().unwrap(), [true, false, false, true, false, false], "only a name of a rule or of a decider is taken");
    let rows = decisions(&d, &run);
    let row = |id: &str| rows.iter().find(|r| r["request_id"] == id).unwrap().clone();
    assert_eq!((row("q1")["state"].clone(), row("q1")["answered_by"].clone(), row("q1")["principal"].clone()), (json!("Answered"), json!("rule:approve"), json!("decider:approve")));
    assert_eq!((row("q4")["answered_by"].clone(), row("q4")["principal"].clone()), (json!("decider:model"), json!("decider:model")));
    for id in ["q2", "q3", "q5", "q6"] {
        assert_eq!((row(id)["state"].clone(), row(id)["answered_by"].clone()), (json!("Open"), Value::Null), "{id} stays open for a client");
    }
    // the client's answer is the label it typed and the daemon's principal, and a request a decider answered takes no second answer
    for label in ["rule:approve", "decider:model", "rule:"] {
        match d.answer_decision(AnswerDecision { run_id: run.clone(), request_id: "q2".into(), answer: "approve".into(), answered_by: label.into() }) {
            Err(Fault::Invalid(why)) => assert!(why.contains("rule:") && why.contains("decider:"), "{why}"),
            other => panic!("a client typed the label {label:?} of a decider and it was taken: {other:?}"),
        }
    }
    assert_eq!(decisions(&d, &run).into_iter().find(|r| r["request_id"] == "q2").unwrap()["state"], "Open", "a refused label answers nothing");
    let client = d.answer_decision(AnswerDecision { run_id: run.clone(), request_id: "q2".into(), answer: "approve".into(), answered_by: "operator".into() }).unwrap();
    assert_eq!(client.body()["outcome"], "answered", "{}", client.body());
    let row2 = decisions(&d, &run).into_iter().find(|r| r["request_id"] == "q2").unwrap();
    assert_eq!((row2["answered_by"].clone(), row2["principal"].clone()), (json!("operator"), json!(super::PRINCIPAL)), "a client's answer carries the daemon's principal and the label it typed");
    let again = d.answer_decision(AnswerDecision { run_id: run.clone(), request_id: "q1".into(), answer: "deny".into(), answered_by: "op".into() }).unwrap();
    assert_eq!(again.body()["outcome"], "already-answered", "{}", again.body());
    d.cancel_run(CancelRun { run_id: run }).unwrap();
    d.stop();
}

#[test]
fn a_submit_takes_the_level_and_the_kind_of_the_daemon_and_names_win_and_the_start_event_records_both() {
    let dir = super::state::scratch("auto-defaults");
    let rt = Trying::new(vec![]);
    let d = start(&dir, &rt, policy(Some(Autonomy::A3), Some("rule"), &["model", "rule"]));
    let row = |run: &str| d.run_detail(run)[0].clone();
    let plain = submit(&d, None, None).unwrap();
    assert_eq!((row(&plain)["autonomy"].clone(), row(&plain)["decider"].clone()), (json!("A3"), json!("rule")), "{}", row(&plain));
    let named = submit(&d, Some(Autonomy::A5), Some("model")).unwrap();
    assert_eq!((row(&named)["autonomy"].clone(), row(&named)["decider"].clone()), (json!("A5"), json!("model")), "the submit wins");
    let lower = submit(&d, Some(Autonomy::A0), None).unwrap();
    assert_eq!((row(&lower)["autonomy"].clone(), row(&lower)["decider"].clone()), (json!("A0"), json!("rule")));
    let record = d.events_after(&named, 0)[0]["record"].clone();
    assert_eq!((record["autonomy"].clone(), record["decider"].clone()), (json!("A5"), json!("model")), "the start event names level and kind: {record}");
    until("the runtime has the three requests", || rt.started.lock().unwrap().len() == 3);
    let given: Vec<_> = rt.started.lock().unwrap().iter().map(|r| (r.terms.autonomy, r.terms.decider.clone())).collect();
    assert!(given.contains(&(Some(Autonomy::A5), Some("model".to_owned()))) && given.contains(&(Some(Autonomy::A3), Some("rule".to_owned()))), "{given:?}");
    for run in [plain, named, lower] {
        d.cancel_run(CancelRun { run_id: run }).unwrap();
    }
    d.stop();
}

#[test]
fn a_daemon_with_no_level_and_a_submit_with_none_start_a_run_that_every_decision_waits_in() {
    let dir = super::state::scratch("auto-none");
    let rt = Trying::new(vec![]);
    let d = start(&dir, &rt, policy(None, None, &[]));
    let run = submit(&d, None, None).unwrap();
    let row = d.run_detail(&run)[0].clone();
    assert!(row.get("autonomy").is_none() && row.get("decider").is_none(), "no level, no kind: {row}");
    until("the start", || rt.started.lock().unwrap().len() == 1);
    let terms = rt.started.lock().unwrap()[0].terms.clone();
    assert_eq!((terms.autonomy, terms.decider), (None, None));
    d.cancel_run(CancelRun { run_id: run }).unwrap();
    d.stop();
}

#[test]
fn a_kind_the_operator_did_not_configure_is_refused_before_a_run_directory_or_a_record_exists() {
    let dir = super::state::scratch("auto-unconfigured");
    let rt = Trying::new(vec![]);
    let d = start(&dir, &rt, policy(None, None, &["rule"]));
    let refused = |autonomy: Option<Autonomy>, decider: Option<&str>| match submit(&d, autonomy, decider) {
        Err(Fault::Invalid(message)) => message,
        other => panic!("not refused: {other:?}"),
    };
    let message = refused(Some(Autonomy::A5), Some("model"));
    assert!(message.contains("model") && message.contains("rule") && message.contains("configured"), "{message}");
    assert!(!message.contains("scripted"), "only the configured kinds: {message}");
    let message = refused(None, Some("../bin/anything"));
    assert!(message.contains("rule"), "{message}");
    let message = refused(Some(Autonomy::A3), None);
    assert!(message.contains("level") && message.contains("rule"), "a level with no kind to take it: {message}");
    assert!(d.runs().is_empty(), "no run");
    assert!(!dir.join("state/runs").read_dir().map(|mut r| r.next().is_some()).unwrap_or(false), "no run directory");
    assert!(d.events_of_daemon_after(0).iter().all(|e| e["kind"] != "runbook.submitted"), "no record of a submit");
    assert!(rt.started.lock().unwrap().is_empty(), "the runtime was not asked");
    d.stop();
}

#[test]
fn the_level_and_the_kind_of_a_run_are_read_from_its_record_after_a_restart_not_from_the_daemons_policy() {
    let dir = super::state::scratch("auto-restart");
    let rt = Trying::new(vec![]);
    let d = start(&dir, &rt, policy(Some(Autonomy::A5), Some("rule"), &["rule", "model"]));
    let run = submit(&d, None, Some("model")).unwrap();
    d.stop();
    drop(d);
    let d = start(&dir, &rt, policy(Some(Autonomy::A0), Some("rule"), &["rule"]));
    let row = d.run_detail(&run)[0].clone();
    assert_eq!((row["autonomy"].clone(), row["decider"].clone()), (json!("A5"), json!("model")), "{row}");
    d.stop();
}

#[test]
fn a_default_level_needs_a_kind_to_take_it_or_the_daemon_does_not_start() {
    let dir = super::state::scratch("auto-nokind");
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    let rt = Trying::new(vec![]);
    let started = |p: Policy| Daemon::start_with(config(&dir), p, Arc::new(Rt(rt.clone()))).err().map(|e| e.to_string());
    let why = started(policy(Some(Autonomy::A3), None, &["rule"])).unwrap();
    assert!(why.contains("A3") && why.contains("decider"), "{why}");
    let why = started(policy(None, Some("model"), &["rule"])).unwrap();
    assert!(why.contains("model") && why.contains("rule"), "the default kind is one that is configured: {why}");
    assert!(!dir.join("state").exists(), "nothing was written");
    assert!(started(policy(Some(Autonomy::A3), Some("rule"), &["rule"])).is_none());
}
