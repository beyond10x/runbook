use runbook::runtime::Confinement;
use runbook::state::{DaemonRow, DecisionState, Fact, Run, RunEvent, RunState, SessionState, State};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub fn scratch(tag: &str) -> PathBuf {
    let p = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("state-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// The facts of the journal in `dir` (a state directory), for a reader beside a daemon that may be writing:
/// only the lines that end in a newline. A last line without its newline is a write that is not over (the
/// daemon's own replay drops such a line the same way), so it is not a fact yet.
pub fn journal_facts(dir: &Path) -> Vec<serde_json::Value> {
    let bytes = std::fs::read(dir.join("journal.jsonl")).unwrap_or_default();
    let whole = bytes.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
    bytes[..whole].split(|b| *b == b'\n').filter(|l| !l.is_empty()).map(|l| serde_json::from_slice(l).unwrap()).collect()
}

fn run(id: &str) -> Run {
    Run {
        run_id: id.into(),
        runbook_id: "book".into(),
        runbook_version: None,
        intake: "text".into(),
        context_path: None,
        settings: BTreeMap::new(),
        confinement: Confinement::Linux,
        session_id: None,
        idempotency_key: Some(format!("key-{id}")),
        submitted_at: "2026-01-01T00:00:00.000Z".into(),
        started_at: None,
        ended_at: None,
        outcome: None,
        failure: None,
        state: RunState::Submitted,
    }
}

fn event(run: &str, sequence: u64, position: u64) -> RunEvent {
    RunEvent {
        event_id: format!("event-{position}"),
        run_id: run.into(),
        session_id: None,
        sequence,
        position,
        kind: "k".into(),
        record: json!({"n": position}),
        at: "2026-01-01T00:00:00.000Z".into(),
    }
}

fn some_facts() -> Vec<Fact> {
    vec![
        Fact::DaemonStarted(DaemonRow { daemon_id: "d1".into(), confinement_floor: Confinement::Linux, principal: "agent:d1".into(), started_at: "t".into() }),
        Fact::SessionOpened { session_id: "s1".into(), client: "me".into(), at: "t".into() },
        Fact::RunSubmitted(run("r1")),
        Fact::EventRecorded(event("r1", 1, 1)),
        Fact::RunStarted { run_id: "r1".into(), at: "t1".into() },
        Fact::RunSubmitted(run("r2")),
        Fact::EventRecorded(event("r2", 1, 2)),
        Fact::EventRecorded(event("r1", 2, 3)),
        Fact::RunWaiting { run_id: "r1".into() },
        Fact::RunEnded { run_id: "r1".into(), outcome: "done".into(), at: "t2".into() },
        Fact::SessionClosed { session_id: "s1".into(), at: "t3".into() },
    ]
}

#[test]
fn opening_a_state_directory_makes_the_layout() {
    let dir = scratch("layout");
    let state = State::open(&dir).unwrap();
    assert!(dir.join("runs").is_dir());
    assert!(dir.join("workspaces").is_dir());
    assert_eq!(state.run_dir("r1"), dir.join("runs/r1"));
    assert_eq!(state.workspace_dir("r1"), dir.join("workspaces/r1"));
}

#[test]
fn replaying_the_journal_gives_the_same_state() {
    let dir = scratch("replay");
    let mut a = State::open(&dir).unwrap();
    for f in some_facts() {
        a.commit(f).unwrap();
    }
    let mut before = serde_json::to_value(a.snapshot()).unwrap();
    assert_eq!(a.daemon().unwrap().daemon_id, "d1");
    drop(a);
    // The configuration row lives as long as the process: a replay does not bring it back.
    before["daemon"] = serde_json::Value::Null;
    let b = State::open(&dir).unwrap();
    assert_eq!(serde_json::to_value(b.snapshot()).unwrap(), before);
    assert_eq!(b.run("r1").unwrap().state, RunState::Ended);
    assert_eq!(b.run("r2").unwrap().state, RunState::Submitted);
    assert_eq!(b.session("s1").unwrap().state, SessionState::Closed);
    assert_eq!(b.last_position(), 3);
    assert_eq!(b.next_sequence("r1"), 3);
    assert_eq!(b.next_sequence("r2"), 2);
    assert_eq!(b.run_by_key("key-r2").unwrap().run_id, "r2");
}

#[test]
fn the_journal_is_one_json_object_per_line() {
    let dir = scratch("lines");
    let mut a = State::open(&dir).unwrap();
    for f in some_facts() {
        a.commit(f).unwrap();
    }
    let text = std::fs::read_to_string(dir.join("journal.jsonl")).unwrap();
    assert_eq!(text.lines().count(), some_facts().len());
    for l in text.lines() {
        let v: serde_json::Value = serde_json::from_str(l).unwrap();
        assert!(v["fact"].is_string(), "{l}");
    }
}

#[test]
fn a_reader_beside_a_writer_takes_the_whole_lines_and_not_a_last_line_without_its_newline() {
    let dir = scratch("reader-torn");
    let mut a = State::open(&dir).unwrap();
    for f in some_facts().into_iter().take(3) {
        a.commit(f).unwrap();
    }
    let path = dir.join("journal.jsonl");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("{\"fact\":\"run_started\",\"run_id\":\"r1\",\"at\":\"t");
    std::fs::write(&path, text).unwrap();
    assert_eq!(journal_facts(&dir).len(), 3);
}

#[test]
fn a_reader_that_follows_a_writing_state_meets_only_whole_lines() {
    let dir = scratch("reader-follows");
    let writer_dir = dir.clone();
    let writer = std::thread::spawn(move || {
        let mut s = State::open(&writer_dir).unwrap();
        for i in 0..3000 {
            s.commit(Fact::SessionOpened { session_id: format!("session-{i}"), client: "a client with a name long enough to make the line long".into(), at: "t".into() }).unwrap();
        }
    });
    let mut seen = 0;
    while !writer.is_finished() {
        let n = journal_facts(&dir).len();
        assert!(n >= seen, "the reader saw {n} facts after {seen}");
        seen = n;
    }
    writer.join().unwrap();
    assert_eq!(journal_facts(&dir).len(), 3000);
}

#[test]
fn a_torn_last_line_is_dropped_and_the_next_fact_starts_a_new_line() {
    let dir = scratch("torn");
    let mut a = State::open(&dir).unwrap();
    for f in some_facts().into_iter().take(3) {
        a.commit(f).unwrap();
    }
    drop(a);
    let path = dir.join("journal.jsonl");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("{\"fact\":\"run_started\",\"run_id\":\"r1\",\"at\":");
    std::fs::write(&path, text).unwrap();
    let mut b = State::open(&dir).unwrap();
    assert_eq!(b.run("r1").unwrap().state, RunState::Submitted);
    b.commit(Fact::RunStarted { run_id: "r1".into(), at: "t".into() }).unwrap();
    drop(b);
    let c = State::open(&dir).unwrap();
    assert_eq!(c.run("r1").unwrap().state, RunState::Running);
}

#[test]
fn a_bad_line_in_the_middle_is_an_error_that_names_the_line() {
    let dir = scratch("bad");
    let mut a = State::open(&dir).unwrap();
    a.commit(some_facts().remove(0)).unwrap();
    drop(a);
    let path = dir.join("journal.jsonl");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("not json\n");
    text.push_str(&serde_json::to_string(&some_facts()[1]).unwrap());
    text.push('\n');
    std::fs::write(&path, text).unwrap();
    let err = State::open(&dir).err().unwrap().to_string();
    assert!(err.contains("line 2"), "{err}");
}

#[test]
fn two_states_cannot_hold_one_directory() {
    let dir = scratch("lock");
    let _a = State::open(&dir).unwrap();
    let err = State::open(&dir).err().unwrap().to_string();
    assert!(err.contains("another daemon"), "{err}");
}

#[test]
fn a_decision_request_keeps_its_record_through_answer_and_withdrawal() {
    let dir = scratch("decisions");
    let mut a = State::open(&dir).unwrap();
    a.commit(Fact::RunSubmitted(run("r1"))).unwrap();
    for id in ["q1", "q2"] {
        a.commit(Fact::DecisionRequested(runbook::state::DecisionRequest {
            request_id: id.into(),
            run_id: "r1".into(),
            case_id: "c".into(),
            action: "a".into(),
            capability: "cap".into(),
            question: "?".into(),
            options: vec![],
            opened_at: "t".into(),
            state: DecisionState::Open,
            answer: None,
            answered_by: None,
            principal: None,
            answered_at: None,
        }))
        .unwrap();
    }
    a.commit(Fact::DecisionAnswered { request_id: "q1".into(), answer: "A".into(), answered_by: "operator".into(), principal: Some("agent:d1".into()), at: "t2".into() }).unwrap();
    a.commit(Fact::DecisionWithdrawn { request_id: "q2".into() }).unwrap();
    drop(a);
    let b = State::open(&dir).unwrap();
    let q1 = b.decision("q1").unwrap();
    assert_eq!((q1.state, q1.answer.as_deref(), q1.answered_by.as_deref()), (DecisionState::Answered, Some("A"), Some("operator")));
    assert_eq!(b.decision("q2").unwrap().state, DecisionState::Withdrawn);
    assert!(b.open_decisions_of("r1").is_empty());
    assert_eq!(b.decisions_of("r1").len(), 2);
}

#[test]
fn events_are_found_by_position_and_by_run() {
    let dir = scratch("events");
    let mut a = State::open(&dir).unwrap();
    a.commit(Fact::RunSubmitted(run("r1"))).unwrap();
    a.commit(Fact::RunSubmitted(run("r2"))).unwrap();
    for (r, s, p) in [("r1", 1, 1), ("r2", 1, 2), ("r1", 2, 3), ("r2", 2, 4)] {
        a.commit(Fact::EventRecorded(event(r, s, p))).unwrap();
    }
    let ps = |v: Vec<&RunEvent>| v.iter().map(|e| e.position).collect::<Vec<_>>();
    assert_eq!(ps(a.events_after(None, None, 0)), [1, 2, 3, 4]);
    assert_eq!(ps(a.events_after(None, None, 2)), [3, 4]);
    assert_eq!(ps(a.events_after(Some("r1"), None, 0)), [1, 3]);
    assert_eq!(ps(a.events_after(Some("r2"), None, 2)), [4]);
    assert_eq!(ps(a.events_after(None, None, 4)), Vec::<u64>::new());
}
