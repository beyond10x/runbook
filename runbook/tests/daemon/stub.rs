use runbook::runtime::{AnswerFailure, Confinement, DecisionRequest, EventSink, Finish, Runtime, StartRequest};
use runbook::stub::{Ask, Script, Step, StubRuntime};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, PartialEq)]
pub enum Seen {
    Started,
    Record(String, Value),
    Asked(String),
    Resumed,
}

#[derive(Default, Clone)]
pub struct Recorder(pub Arc<Mutex<Vec<Seen>>>);

impl EventSink for Recorder {
    fn started(&self) {
        self.0.lock().unwrap().push(Seen::Started);
    }
    fn record(&self, kind: &str, record: Value) {
        self.0.lock().unwrap().push(Seen::Record(kind.to_string(), record));
    }
    fn decision_requested(&self, request: DecisionRequest) {
        self.0.lock().unwrap().push(Seen::Asked(request.request_id));
    }
    fn resumed(&self) {
        self.0.lock().unwrap().push(Seen::Resumed);
    }
}

impl Recorder {
    fn seen(&self) -> Vec<String> {
        self.0.lock().unwrap().iter().map(|s| format!("{s:?}")).collect()
    }
    fn wait_for(&self, want: usize) {
        for _ in 0..2000 {
            if self.0.lock().unwrap().len() >= want {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("timed out; saw {:?}", self.seen());
    }
}

pub fn request(run: &str, runbook: &str) -> StartRequest {
    StartRequest {
        run_id: run.to_string(),
        runbook: runbook.to_string(),
        version: None,
        intake: "text".to_string(),
        context_dir: None,
        settings: BTreeMap::new(),
        confinement: Confinement::Linux,
        state_dir: "state".into(),
        workspace_dir: "workspace".into(),
    }
}

pub fn ask(id: &str) -> Ask {
    Ask {
        request_id: id.to_string(),
        case_id: "case-1".to_string(),
        action: "intake.approve".to_string(),
        capability: "approve".to_string(),
        question: "Approve?".to_string(),
        options: vec![json!({"letter": "A", "does": "yes", "cost": "none", "consequence": {"admits": [], "blocks": []}})],
    }
}

fn stub(steps: Vec<Step>) -> Arc<StubRuntime> {
    let s = StubRuntime::new();
    s.script("book", Script::new(steps));
    Arc::new(s)
}

#[test]
fn a_script_emits_records_in_order_and_ends_with_its_outcome() {
    let rt = stub(vec![
        Step::Emit { kind: "run.opened".into(), body: json!({"n": 1}) },
        Step::Sleep(1),
        Step::Emit { kind: "run.closed".into(), body: json!({"n": 2}) },
        Step::End("done".into()),
    ]);
    let sink = Recorder::default();
    let end = rt.start(request("r1", "book"), &sink, &AtomicBool::new(false));
    assert_eq!(end, Ok(Finish::Outcome("done".into())));
    assert_eq!(
        *sink.0.lock().unwrap(),
        vec![
            Seen::Started,
            Seen::Record("run.opened".into(), json!({"n": 1})),
            Seen::Record("run.closed".into(), json!({"n": 2})),
        ]
    );
}

#[test]
fn a_run_that_fails_to_start_is_an_error_and_never_reports_started() {
    let rt = stub(vec![Step::FailToStart("no capacity".into())]);
    let sink = Recorder::default();
    let end = rt.start(request("r1", "book"), &sink, &AtomicBool::new(false));
    assert_eq!(end.unwrap_err().reason, "no capacity");
    assert!(sink.0.lock().unwrap().is_empty());
}

#[test]
fn the_table_chooses_the_script_by_runbook_name_and_version() {
    let s = StubRuntime::new();
    s.script("book", Script::new(vec![]).version("2"));
    assert!(s.knows_runbook("book", None));
    assert!(s.knows_runbook("book", Some("2")));
    assert!(!s.knows_runbook("book", Some("3")));
    assert!(!s.knows_runbook("other", None));
}

#[test]
fn a_run_without_a_script_for_its_name_fails_to_start() {
    let rt = stub(vec![]);
    let err = rt.start(request("r1", "other"), &Recorder::default(), &AtomicBool::new(false)).unwrap_err();
    assert!(err.reason.contains("other"), "{}", err.reason);
}

#[test]
fn the_cancel_flag_is_honoured_between_steps() {
    let rt = stub(vec![
        Step::Emit { kind: "a".into(), body: json!(1) },
        Step::Sleep(60_000),
        Step::Emit { kind: "b".into(), body: json!(2) },
    ]);
    let sink = Recorder::default();
    let cancel = Arc::new(AtomicBool::new(false));
    let handle = {
        let (rt, sink, cancel) = (rt.clone(), sink.clone(), cancel.clone());
        std::thread::spawn(move || rt.start(request("r1", "book"), &sink, &cancel))
    };
    sink.wait_for(2);
    cancel.store(true, Ordering::SeqCst);
    assert_eq!(handle.join().unwrap(), Ok(Finish::Cancelled));
    assert!(!sink.seen().iter().any(|s| s.contains("\"b\"")));
}

#[test]
fn a_flag_set_before_the_start_stops_the_run_before_its_first_step() {
    let rt = stub(vec![Step::Emit { kind: "a".into(), body: json!(1) }]);
    let sink = Recorder::default();
    let end = rt.start(request("r1", "book"), &sink, &AtomicBool::new(true));
    assert_eq!(end, Ok(Finish::Cancelled));
    assert!(!sink.seen().iter().any(|s| s.contains("Record")));
}

#[test]
fn a_decision_waits_for_every_answer_and_then_resumes() {
    let rt = stub(vec![Step::Ask(vec![ask("q1"), ask("q2")]), Step::End("approved".into())]);
    let sink = Recorder::default();
    let handle = {
        let (rt, sink) = (rt.clone(), sink.clone());
        std::thread::spawn(move || rt.start(request("r1", "book"), &sink, &AtomicBool::new(false)))
    };
    sink.wait_for(3);
    assert_eq!(sink.seen(), ["Started", "Asked(\"q1\")", "Asked(\"q2\")"]);
    assert_eq!(rt.answer("r1", "q1", "A", "operator"), Ok(()));
    std::thread::sleep(Duration::from_millis(30));
    assert!(!sink.seen().contains(&"Resumed".to_string()), "resumed with one request open");
    assert_eq!(rt.answer("r1", "q2", "A", "operator"), Ok(()));
    assert_eq!(handle.join().unwrap(), Ok(Finish::Outcome("approved".into())));
    assert_eq!(sink.seen().last().unwrap(), "Resumed");
}

#[test]
fn an_answer_for_a_run_the_stub_does_not_hold_is_refused() {
    let rt = stub(vec![]);
    assert_eq!(rt.answer("gone", "q1", "A", "operator"), Err(AnswerFailure::UnknownRun));
}

#[test]
fn a_cancel_ends_a_run_that_waits_on_a_decision() {
    let rt = stub(vec![Step::Ask(vec![ask("q1")]), Step::End("never".into())]);
    let sink = Recorder::default();
    let cancel = Arc::new(AtomicBool::new(false));
    let handle = {
        let (rt, sink, cancel) = (rt.clone(), sink.clone(), cancel.clone());
        std::thread::spawn(move || rt.start(request("r1", "book"), &sink, &cancel))
    };
    sink.wait_for(2);
    cancel.store(true, Ordering::SeqCst);
    rt.cancel("r1");
    assert_eq!(handle.join().unwrap(), Ok(Finish::Cancelled));
}
