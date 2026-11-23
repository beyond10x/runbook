//! `story:submitrun-outcome-order`: the handler of `answer-decision` answers
//! in the order of `ess/domains/daemon.yaml` where two conditions hold.

use super::commands::config;
use super::state::scratch;
use runbook::commands::{AnswerDecision, Daemon};
use runbook::runtime::Confinement;
use runbook::state::{DecisionRequest, DecisionState, Fact, Run, RunState, State};
use runbook::stub::StubRuntime;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

fn request(id: &str, run: &str) -> DecisionRequest {
    DecisionRequest {
        request_id: id.into(),
        run_id: run.into(),
        case_id: "c".into(),
        action: "a".into(),
        capability: "k".into(),
        question: "q".into(),
        options: vec![],
        opened_at: "t".into(),
        state: DecisionState::Open,
        answer: None,
        answered_by: None,
        principal: None,
        answered_at: None,
    }
}

fn put(st: &mut State, id: &str, run: &str, end: Option<DecisionState>) {
    st.commit(Fact::DecisionRequested(request(id, run))).unwrap();
    match end {
        Some(DecisionState::Withdrawn) => st.commit(Fact::DecisionWithdrawn { request_id: id.into() }).unwrap(),
        Some(DecisionState::Answered) => st
            .commit(Fact::DecisionAnswered { request_id: id.into(), answer: "A".into(), answered_by: "x".into(), principal: None, at: "t".into() })
            .unwrap(),
        _ => {}
    }
}

fn submitted(id: &str) -> Run {
    Run {
        run_id: id.into(),
        runbook_id: "quick".into(),
        runbook_version: None,
        intake: "x".into(),
        context_path: None,
        settings: BTreeMap::new(),
        confinement: Confinement::Linux,
        confinement_source: None,
        terms: Default::default(),
        session_id: None,
        idempotency_key: None,
        submitted_at: "t".into(),
        started_at: None,
        ended_at: None,
        outcome: None,
        failure: None,
        state: RunState::Submitted,
    }
}

#[test]
fn answer_decision_answers_in_the_order_of_the_domain_file_where_two_conditions_hold() {
    let dir = scratch("adv-answer-order");
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    {
        let mut st = State::open(&Path::new(&dir).join("state")).unwrap();
        // run "ghost" does not exist; run "r1" exists and is not waiting (a Submitted run ends FailedToStart at start)
        st.commit(Fact::RunSubmitted(submitted("r1"))).unwrap();
        for run in ["ghost", "r1"] {
            put(&mut st, &format!("{run}-withdrawn"), run, Some(DecisionState::Withdrawn));
            put(&mut st, &format!("{run}-answered"), run, Some(DecisionState::Answered));
            put(&mut st, &format!("{run}-open"), run, None);
        }
    }
    let d = Daemon::start(config(&dir), Arc::new(StubRuntime::new())).unwrap();
    let ask = |run: &str, request: &str| {
        let o = d
            .answer_decision(AnswerDecision { run_id: run.into(), request_id: request.into(), answer: "A".into(), answered_by: "op".into() })
            .unwrap();
        (o.name, o.status)
    };
    let mut wrong = vec![];
    let mut check = |label: &str, got: (&'static str, u16), want: &str| {
        println!("{label}: {} (want {want})", got.0);
        if got.0 != want {
            wrong.push(format!("{label}: got {} want {want}", got.0));
        }
    };
    // no-request above everything: the request does not exist, nor does the run
    check("no-request > no-run", ask("ghost", "none"), "no-request");
    // not-of-run above withdrawn, already-answered, no-run: the request is of "ghost", the run named is another
    check("not-of-run > withdrawn", ask("r1", "ghost-withdrawn"), "not-of-run");
    check("not-of-run > already-answered", ask("r1", "ghost-answered"), "not-of-run");
    check("not-of-run > no-run", ask("elsewhere", "ghost-open"), "not-of-run");
    // withdrawn above already-answered cannot hold together (one state); above no-run and run-not-waiting
    check("withdrawn > no-run", ask("ghost", "ghost-withdrawn"), "withdrawn");
    check("withdrawn > run-not-waiting", ask("r1", "r1-withdrawn"), "withdrawn");
    // already-answered above no-run and run-not-waiting
    check("already-answered > no-run", ask("ghost", "ghost-answered"), "already-answered");
    check("already-answered > run-not-waiting", ask("r1", "r1-answered"), "already-answered");
    // no-run and run-not-waiting are alone with an open request
    check("no-run alone", ask("ghost", "ghost-open"), "no-run");
    check("run-not-waiting alone", ask("r1", "r1-open"), "run-not-waiting");
    // nothing changed: the open requests are still open, the answered one still holds its answer
    for id in ["ghost-open", "r1-open"] {
        assert_eq!(d.open_decisions_of_run(if id.starts_with("ghost") { "ghost" } else { "r1" }).iter().filter(|r| r["request_id"] == id).count(), 1, "{id} stays open");
    }
    d.stop();
    assert!(wrong.is_empty(), "{wrong:#?}");
}
