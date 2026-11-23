use super::state::scratch;
use super::stub::ask;
use runbook::commands::{AnswerDecision, CancelRun, CloseSession, Config, Daemon, Fault, OpenSession, Outcome, SubmitRun, MAX_INTAKE_BYTES, OUTCOMES};
use runbook::runtime::{AnswerFailure, Confinement, EventSink, Finish, Recovery, RunbookInfo, Runtime, RuntimeFailure, StartRequest};
use runbook::state::{DaemonRow, DecisionRequest, DecisionState, Fact, Run, RunState, State};
use runbook::stub::{Script, Step, StubRuntime};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct Fx {
    pub dir: PathBuf,
    pub stub: Arc<StubRuntime>,
    d: Option<Daemon>,
}

pub fn config(dir: &Path) -> Config {
    Config {
        state_dir: dir.join("state"),
        confinement_floor: Confinement::Linux,
        context_roots: vec![dir.join("ctx")],
        runbook_dir: Some(dir.join("runbooks")),
        principal: super::PRINCIPAL.into(),
    }
}

impl Fx {
    pub fn new(tag: &str) -> Fx {
        let dir = scratch(tag);
        std::fs::create_dir_all(dir.join("ctx")).unwrap();
        std::fs::create_dir_all(dir.join("runbooks")).unwrap();
        let stub = Arc::new(StubRuntime::new());
        stub.script("quick", Script::new(vec![Step::End("done".into())]));
        stub.script("hold", Script::new(vec![Step::Sleep(60_000)]));
        stub.script("ask", Script::new(vec![Step::Ask(vec![ask("q1")]), Step::End("approved".into())]));
        stub.script("ask2", Script::new(vec![Step::Ask(vec![ask("q1"), ask("q2")]), Step::End("approved".into())]));
        stub.script("ask-b", Script::new(vec![Step::Ask(vec![ask("q3")]), Step::End("approved".into())]));
        let d = Daemon::start(config(&dir), stub.clone()).unwrap();
        Fx { dir, stub, d: Some(d) }
    }

    pub fn d(&self) -> &Daemon {
        self.d.as_ref().unwrap()
    }

    /// Stop the daemon and start it again on the same directory.
    pub fn restart(&mut self) {
        self.d.take().unwrap().stop();
        self.d = Some(Daemon::start(config(&self.dir), self.stub.clone()).unwrap());
    }

    /// Stop the daemon, let `edit` change the journal file, and start it again on the same directory.
    pub fn restart_editing(&mut self, edit: impl FnOnce(&Path)) {
        self.d.take().unwrap().stop();
        edit(&self.dir.join("state/journal.jsonl"));
        self.d = Some(Daemon::start(config(&self.dir), self.stub.clone()).unwrap());
    }

    /// Stop the daemon and start it again on the same directory over another runtime.
    pub fn restart_with(&mut self, runtime: Arc<dyn Runtime>) {
        self.d.take().unwrap().stop();
        self.d = Some(Daemon::start(config(&self.dir), runtime).unwrap());
    }

    pub fn submit(&self, book: &str) -> String {
        self.submit_with(SubmitRun { runbook_id: book.into(), intake: "intake".into(), ..SubmitRun::default() })
    }

    pub fn submit_with(&self, input: SubmitRun) -> String {
        let o = self.d().submit_run(input).unwrap();
        assert_eq!(o.name, "submitted", "{o:?}");
        o.run_id.unwrap()
    }

    pub fn state_of(&self, run: &str) -> String {
        self.d().run_detail(run)[0]["state"].as_str().unwrap().to_string()
    }

    pub fn wait_state(&self, run: &str, want: &str) {
        let until = Instant::now() + Duration::from_secs(20);
        while self.state_of(run) != want {
            assert!(Instant::now() < until, "run {run} is {} and not {want}", self.state_of(run));
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    pub fn wait_open_decisions(&self, run: &str, n: usize) {
        let until = Instant::now() + Duration::from_secs(20);
        while self.d().open_decisions_of_run(run).len() != n {
            assert!(Instant::now() < until, "run {run} has {} open decisions", self.d().open_decisions_of_run(run).len());
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    pub fn answer(&self, run: &str, request: &str) -> Outcome {
        self.d().answer_decision(AnswerDecision { run_id: run.into(), request_id: request.into(), answer: "A".into(), answered_by: "operator".into() }).unwrap()
    }

    pub fn cancel(&self, run: &str) -> Outcome {
        self.d().cancel_run(CancelRun { run_id: run.into() }).unwrap()
    }
}

impl Drop for Fx {
    fn drop(&mut self) {
        if let Some(d) = &self.d {
            d.stop();
        }
    }
}

fn kinds(rows: &[Value]) -> Vec<String> {
    rows.iter().map(|r| r["kind"].as_str().unwrap().to_string()).collect()
}

#[test]
fn a_submit_makes_a_run_that_the_runtime_takes_to_its_outcome() {
    let fx = Fx::new("submit");
    fx.stub.script(
        "book",
        Script::new(vec![
            Step::Emit { kind: "run.opened".into(), body: json!({"a": 1}) },
            Step::Emit { kind: "run.closed".into(), body: json!({"b": 2}) },
            Step::End("passed".into()),
        ]),
    );
    let o = fx.d().submit_run(SubmitRun { runbook_id: "book".into(), intake: "the intake".into(), ..SubmitRun::default() }).unwrap();
    assert_eq!((o.name, o.status, o.is_refusal()), ("submitted", 202, false));
    assert_eq!(o.published[0].0, "sdc.daemon.RunSubmitted");
    let run = o.run_id.clone().unwrap();
    assert_eq!(o.published[0].1["run_id"], json!(run));
    let id = uuid_text(&run);
    assert_eq!(id.as_bytes()[14], b'7', "a UUID version 7: {run}");
    fx.wait_state(&run, "Ended");
    let row = &fx.d().run_detail(&run)[0];
    assert_eq!(row["outcome"], "passed");
    assert_eq!(row["intake"], "the intake");
    assert_eq!(row["confinement"], "linux", "the default of a submit");
    assert!(row["started_at"].is_string() && row["ended_at"].is_string());
    let runs = fx.d().runs();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0]["run_id"], json!(run));
    assert!(runs[0].get("intake").is_none(), "the list row has no intake");
    let events = fx.d().events_after(&run, 0);
    assert_eq!(kinds(&events), ["runbook.submitted", "run.opened", "run.closed"]);
    assert_eq!(events[1]["record"], json!({"a": 1}));
    let seq: Vec<_> = events.iter().map(|e| e["sequence"].as_u64().unwrap()).collect();
    assert_eq!(seq, [1, 2, 3]);
}

fn uuid_text(s: &str) -> String {
    assert_eq!(s.len(), 36, "{s}");
    s.to_string()
}

#[test]
fn the_record_and_the_workspace_are_children_of_the_state_directory() {
    let fx = Fx::new("dirs");
    let run = fx.submit("quick");
    let state = fx.dir.join("state");
    assert!(state.join("runs").join(&run).is_dir());
    assert!(state.join("workspaces").join(&run).is_dir());
    assert_eq!(state.join("workspaces").join(&run).parent().unwrap(), state.join("workspaces"));
}

#[test]
fn a_second_submit_with_the_same_key_returns_the_first_run_and_makes_none() {
    let fx = Fx::new("idem");
    let one = |key: &str| SubmitRun { runbook_id: "quick".into(), intake: "x".into(), idempotency_key: Some(key.into()), ..SubmitRun::default() };
    let first = fx.d().submit_run(one("k1")).unwrap();
    let again = fx.d().submit_run(one("k1")).unwrap();
    assert_eq!((first.name, again.name, again.status), ("submitted", "already-submitted", 202));
    assert_eq!(again.run_id, first.run_id);
    assert_eq!(again.published[0].0, "sdc.daemon.SubmitReplayed");
    assert_eq!(again.published[0].1, json!({"idempotency_key": "k1", "run_id": first.run_id}), "the answer names the first run as data");
    assert_eq!(again.body()["published"][0]["payload"]["run_id"], json!(first.run_id));
    assert_eq!(fx.d().runs().len(), 1);
    let other = fx.d().submit_run(one("k2")).unwrap();
    assert_eq!(other.name, "submitted");
    assert_ne!(other.run_id, first.run_id);
    assert_eq!(fx.d().runs().len(), 2);
    let long = fx.d().submit_run(one(&"k".repeat(201)));
    assert!(matches!(long, Err(Fault::Invalid(_))), "{long:?}");
    assert_eq!(fx.d().submit_run(one(&"k".repeat(200))).unwrap().name, "submitted");
}

#[test]
fn the_idempotency_key_holds_after_a_restart() {
    let mut fx = Fx::new("idem-restart");
    let input = SubmitRun { runbook_id: "quick".into(), intake: "x".into(), idempotency_key: Some("k".into()), ..SubmitRun::default() };
    let run = fx.d().submit_run(input.clone()).unwrap().run_id.unwrap();
    fx.wait_state(&run, "Ended");
    fx.restart();
    let again = fx.d().submit_run(input).unwrap();
    assert_eq!((again.name, again.run_id), ("already-submitted", Some(run)));
}

fn intake_of(bytes: usize, unit: &str) -> String {
    assert_eq!(bytes % unit.len(), 0);
    unit.repeat(bytes / unit.len())
}

#[test]
fn the_intake_limit_is_counted_in_bytes() {
    let fx = Fx::new("intake");
    let go = |intake: String| fx.d().submit_run(SubmitRun { runbook_id: "quick".into(), intake, ..SubmitRun::default() }).unwrap();
    assert_eq!(MAX_INTAKE_BYTES, 1_048_576);
    assert_eq!(go("a".repeat(1_048_576)).name, "submitted");
    let refused = go("a".repeat(1_048_577));
    assert_eq!((refused.name, refused.status, refused.error), ("intake-too-large", 422, Some("IntakeTooLarge")));
    // Two bytes a character: exactly the limit is taken, one character more is refused although the
    // character count (524,289) is far below the limit of the specification.
    assert_eq!(go(intake_of(1_048_576, "é")).name, "submitted");
    let refused = go(intake_of(1_048_578, "é"));
    assert_eq!(refused.name, "intake-too-large");
    // Three bytes a character.
    assert_eq!(go(intake_of(1_048_575, "€")).name, "submitted");
    assert_eq!(go(intake_of(1_048_578, "€")).name, "intake-too-large");
    // A refusal makes no run: three were taken above.
    assert_eq!(fx.d().runs().len(), 3);
}

fn with_context(path: &Path) -> SubmitRun {
    SubmitRun { runbook_id: "quick".into(), intake: "x".into(), context_path: Some(path.to_string_lossy().into_owned()), ..SubmitRun::default() }
}

#[test]
fn a_context_under_a_root_is_copied_into_the_workspace() {
    let fx = Fx::new("ctx-in");
    let proj = fx.dir.join("ctx/proj");
    std::fs::create_dir_all(proj.join("sub")).unwrap();
    std::fs::write(proj.join("a.txt"), "alpha").unwrap();
    std::fs::write(proj.join("sub/b.txt"), "beta").unwrap();
    let secret = fx.dir.join("secret.txt");
    std::fs::write(&secret, "outside").unwrap();
    std::os::unix::fs::symlink(&secret, proj.join("link.txt")).unwrap();
    let run = fx.submit_with(with_context(&proj));
    fx.wait_state(&run, "Ended");
    let ws = fx.dir.join("state/workspaces").join(&run);
    assert_eq!(std::fs::read_to_string(ws.join("a.txt")).unwrap(), "alpha");
    assert_eq!(std::fs::read_to_string(ws.join("sub/b.txt")).unwrap(), "beta");
    assert!(!ws.join("link.txt").exists(), "a symbolic link is not carried into a run");
    assert_eq!(fx.d().run_detail(&run)[0]["context_path"], json!(proj.to_string_lossy()));
}

#[test]
fn a_context_outside_the_roots_is_refused_also_through_a_link_and_through_dots() {
    let fx = Fx::new("ctx-out");
    let outside = fx.dir.join("outside");
    std::fs::create_dir_all(outside.join("deep")).unwrap();
    std::fs::create_dir_all(fx.dir.join("ctx/inner")).unwrap();
    std::fs::create_dir_all(fx.dir.join("ctx/other")).unwrap();
    std::os::unix::fs::symlink(&outside, fx.dir.join("ctx/escape")).unwrap();
    std::fs::write(fx.dir.join("ctx/file.txt"), "f").unwrap();
    let refused = |p: &Path| {
        let o = fx.d().submit_run(with_context(p)).unwrap();
        (o.name, o.status, o.error)
    };
    let want = ("context-outside-roots", 422, Some("ContextOutsideRoots"));
    assert_eq!(refused(&outside), want, "a directory beside the root");
    assert_eq!(refused(&fx.dir), want, "the parent of the root");
    assert_eq!(refused(&fx.dir.join("ctx/escape")), want, "a link under the root that leads out");
    assert_eq!(refused(&fx.dir.join("ctx/escape/deep")), want, "below that link");
    assert_eq!(refused(&fx.dir.join("ctx/inner/../../outside")), want, "dots that leave the root");
    assert_eq!(refused(&fx.dir.join("ctx/../outside/deep")), want, "dots through the root");
    assert_eq!(refused(&fx.dir.join("ctx/missing")), want, "a path that does not exist");
    assert_eq!(refused(&fx.dir.join("ctx/file.txt")), want, "a file is no directory");
    assert_eq!(refused(Path::new("ctx/inner")), want, "a relative path");
    // Dots that stay inside are fine, and so is the root itself.
    assert_eq!(fx.d().submit_run(with_context(&fx.dir.join("ctx/inner/../other"))).unwrap().name, "submitted");
    assert_eq!(fx.d().submit_run(with_context(&fx.dir.join("ctx"))).unwrap().name, "submitted");
    // A root that is itself reached through a link.
    std::os::unix::fs::symlink(fx.dir.join("ctx/other"), fx.dir.join("viaroot")).unwrap();
    assert_eq!(fx.d().submit_run(with_context(&fx.dir.join("viaroot"))).unwrap().name, "submitted");
}

#[test]
fn with_no_context_root_every_context_path_is_refused() {
    let dir = scratch("ctx-none");
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    let stub = Arc::new(StubRuntime::new());
    stub.script("quick", Script::new(vec![]));
    let d = Daemon::start(Config { context_roots: vec![], ..config(&dir) }, stub).unwrap();
    let o = d.submit_run(with_context(&dir.join("ctx"))).unwrap();
    assert_eq!(o.name, "context-outside-roots");
    let none = d.submit_run(SubmitRun { runbook_id: "quick".into(), intake: "x".into(), ..SubmitRun::default() }).unwrap();
    assert_eq!(none.name, "submitted", "no context given needs no root");
    d.stop();
}

#[test]
fn a_context_root_that_is_not_a_directory_stops_the_start() {
    let dir = scratch("ctx-bad-root");
    let err = Daemon::start(Config { context_roots: vec![dir.join("nope")], ..config(&dir) }, Arc::new(StubRuntime::new())).err().unwrap();
    assert!(err.to_string().contains("context root"), "{err}");
}

#[test]
fn a_runbook_is_known_by_the_runtime_table_or_the_runbook_directory() {
    let fx = Fx::new("runbooks");
    std::fs::create_dir_all(fx.dir.join("runbooks/mine")).unwrap();
    std::fs::write(fx.dir.join("runbooks/theirs.protocol.yaml"), "protocol: {}").unwrap();
    std::fs::write(fx.dir.join("runbooks/.hidden"), "").unwrap();
    std::fs::create_dir_all(fx.dir.join("runbooks/sub")).unwrap();
    let name = |n: &str, v: Option<&str>| {
        let o = fx.d().submit_run(SubmitRun { runbook_id: n.into(), runbook_version: v.map(String::from), intake: "x".into(), ..SubmitRun::default() }).unwrap();
        o.name
    };
    assert_eq!(name("quick", None), "submitted");
    assert_eq!(name("mine", None), "submitted");
    assert_eq!(name("theirs", None), "submitted");
    assert_eq!(name("nothing", None), "no-runbook");
    assert_eq!(name("", None), "no-runbook");
    assert_eq!(name("../runbooks/mine", None), "no-runbook");
    assert_eq!(name(".hidden", None), "no-runbook");
    assert_eq!(name("mine", Some("1")), "no-runbook", "a version of a directory runbook cannot be checked");
    fx.stub.script("versioned", Script::new(vec![]).version("2"));
    assert_eq!(name("versioned", Some("2")), "submitted");
    assert_eq!(name("versioned", Some("3")), "no-runbook");
}

#[test]
fn a_run_that_fails_to_start_says_why_and_keeps_a_record() {
    let fx = Fx::new("failstart");
    fx.stub.script("broken", Script::new(vec![Step::FailToStart("no capacity".into())]));
    let run = fx.submit("broken");
    fx.wait_state(&run, "FailedToStart");
    let row = &fx.d().run_detail(&run)[0];
    assert_eq!(row["failure"], "no capacity");
    assert!(row["ended_at"].is_string() && row.get("started_at").is_none() && row.get("outcome").is_none());
    let events = fx.d().events_after(&run, 0);
    assert_eq!(kinds(&events), ["runbook.submitted", "runbook.start-failed"]);
    assert_eq!(events[1]["record"], json!({"reason": "no capacity"}));
}

#[test]
fn a_runtime_that_fails_after_it_started_ends_the_run_as_a_runtime_failure() {
    let fx = Fx::new("failafter");
    fx.stub.script("late", Script::new(vec![Step::Emit { kind: "a".into(), body: json!(1) }, Step::FailToStart("disk full".into())]));
    let run = fx.submit("late");
    fx.wait_state(&run, "Ended");
    assert_eq!(fx.d().run_detail(&run)[0]["outcome"], "runtime-failure");
    assert_eq!(kinds(&fx.d().events_after(&run, 0)), ["runbook.submitted", "a", "runbook.runtime-failed"]);
}

#[test]
fn a_cancel_moves_the_run_to_cancelled_and_a_second_cancel_is_refused() {
    let fx = Fx::new("cancel");
    let run = fx.submit("hold");
    fx.wait_state(&run, "Running");
    let o = fx.cancel(&run);
    assert_eq!((o.name, o.status), ("cancelled", 202));
    assert_eq!(o.published[0].0, "sdc.daemon.RunCancelled");
    assert_eq!(fx.state_of(&run), "Cancelled");
    let again = fx.cancel(&run);
    assert_eq!((again.name, again.status, again.error), ("already-ended", 409, Some("RunAlreadyEnded")));
    assert_eq!(fx.state_of(&run), "Cancelled");
    assert!(fx.d().run_detail(&run)[0]["ended_at"].is_string());
    let events = fx.d().events_after(&run, 0);
    assert_eq!(kinds(&events), ["runbook.submitted", "runbook.cancel-requested"]);
    fx.d().stop();
}

#[test]
fn a_cancel_stops_the_runtime_between_its_steps_and_the_run_stays_cancelled() {
    let fx = Fx::new("cancel-stops");
    fx.stub.script("two", Script::new(vec![Step::Sleep(60_000), Step::Emit { kind: "late".into(), body: json!(1) }, Step::End("done".into())]));
    let run = fx.submit("two");
    fx.wait_state(&run, "Running");
    fx.cancel(&run);
    std::thread::sleep(Duration::from_millis(60));
    assert_eq!(fx.state_of(&run), "Cancelled");
    assert!(!kinds(&fx.d().events_after(&run, 0)).contains(&"late".to_string()));
}

#[test]
fn a_cancel_withdraws_every_open_decision_request() {
    let fx = Fx::new("cancel-withdraws");
    let run = fx.submit("ask2");
    fx.wait_open_decisions(&run, 2);
    assert_eq!(fx.state_of(&run), "WaitingOnDecision");
    assert_eq!(fx.answer(&run, "q1").name, "answered");
    let o = fx.cancel(&run);
    assert_eq!(o.name, "cancelled");
    assert!(fx.d().open_decisions_of_run(&run).is_empty());
    let all = fx.d().decisions_of_run(&run);
    let states: BTreeMap<_, _> = all.iter().map(|d| (d["request_id"].as_str().unwrap(), d["state"].as_str().unwrap())).collect();
    assert_eq!(states, BTreeMap::from([("q1", "Answered"), ("q2", "Withdrawn")]), "an answered request keeps its state");
    let o = fx.answer(&run, "q2");
    assert_eq!((o.name, o.status, o.error), ("withdrawn", 409, Some("DecisionWithdrawn")));
}

#[test]
fn a_decision_is_answered_once_and_the_run_goes_on() {
    let fx = Fx::new("answer");
    let run = fx.submit("ask");
    fx.wait_open_decisions(&run, 1);
    assert_eq!(fx.state_of(&run), "WaitingOnDecision");
    let open = fx.d().open_decisions_of_run(&run);
    assert_eq!(open[0]["request_id"], "q1");
    assert_eq!(open[0]["question"], "Approve?");
    assert_eq!((open[0]["case_id"].clone(), open[0]["action"].clone(), open[0]["capability"].clone()), (json!("case-1"), json!("intake.approve"), json!("approve")), "the row names the step that asks");
    assert_eq!(open[0]["options"][0]["letter"], "A");
    let o = fx.answer(&run, "q1");
    assert_eq!((o.name, o.status), ("answered", 202));
    assert_eq!(o.published[0].0, "sdc.daemon.DecisionAnswered");
    assert_eq!(o.published[0].1, json!({"request_id": "q1", "run_id": run, "answered_by": "operator", "principal": super::PRINCIPAL}));
    fx.wait_state(&run, "Ended");
    assert_eq!(fx.d().run_detail(&run)[0]["outcome"], "approved");
    assert!(fx.d().open_decisions_of_run(&run).is_empty());
    let d = &fx.d().decisions_of_run(&run)[0];
    assert_eq!((d["state"].as_str(), d["answer"].as_str(), d["answered_by"].as_str()), (Some("Answered"), Some("A"), Some("operator")));
    assert!(d["answered_at"].is_string());
}

#[test]
fn a_second_answer_is_refused_by_the_state_of_its_request_before_the_state_of_its_run() {
    // While another request keeps the run waiting, the refusal is DecisionAlreadyAnswered.
    let fx = Fx::new("answer-twice");
    let run = fx.submit("ask2");
    fx.wait_open_decisions(&run, 2);
    assert_eq!(fx.answer(&run, "q1").name, "answered");
    let again = fx.answer(&run, "q1");
    assert_eq!((again.name, again.status, again.error), ("already-answered", 409, Some("DecisionAlreadyAnswered")));
    assert_eq!(fx.d().decisions_of_run(&run)[0]["answered_by"], "operator", "the first answer stands");
    // With a single request the run resumes and ends on the first answer; the request's state is still
    // checked first (decision 16), so the second answer names the request, not the run.
    let one = fx.submit("ask-b");
    fx.wait_open_decisions(&one, 1);
    assert_eq!(fx.answer(&one, "q3").name, "answered");
    fx.wait_state(&one, "Ended");
    let again = fx.answer(&one, "q3");
    assert_eq!((again.name, again.status, again.error), ("already-answered", 409, Some("DecisionAlreadyAnswered")));
    assert_eq!(again.payload, Some(json!({"request_id": "q3"})));
    assert_eq!(fx.d().decisions_of_run(&one)[0]["answered_by"], "operator", "the first answer stands");
}

#[test]
fn an_answer_to_a_request_whose_run_was_cancelled_is_withdrawn_and_not_run_not_waiting() {
    let fx = Fx::new("answer-withdrawn");
    let run = fx.submit("ask");
    fx.wait_open_decisions(&run, 1);
    assert_eq!(fx.cancel(&run).name, "cancelled");
    let o = fx.answer(&run, "q1");
    assert_eq!((o.name, o.status, o.error), ("withdrawn", 409, Some("DecisionWithdrawn")));
}

#[test]
fn an_answer_the_runtime_cannot_take_is_recorded_in_the_run() {
    let mut fx = Fx::new("answer-gone");
    let run = fx.submit("ask");
    fx.wait_open_decisions(&run, 1);
    // A runtime that resumed the run but holds no such request (the stub keeps nothing across a start).
    fx.restart_with(Arc::new(Resuming(fx.stub.clone())));
    assert_eq!(fx.state_of(&run), "WaitingOnDecision", "kept in its last state");
    assert_eq!(fx.answer(&run, "q1").name, "answered", "the answer stands");
    let events = fx.d().events_after(&run, 0);
    assert_eq!(kinds(&events).last().unwrap(), "runbook.answer-undeliverable");
}

#[test]
fn positions_and_sequences_grow_with_two_runs_interleaved() {
    let fx = Fx::new("order");
    let steps = |tag: &str| {
        let mut v = vec![];
        for i in 0..5 {
            v.push(Step::Emit { kind: format!("{tag}.{i}"), body: json!(i) });
            v.push(Step::Sleep(2));
        }
        v.push(Step::End("done".into()));
        Script::new(v)
    };
    fx.stub.script("one", steps("one"));
    fx.stub.script("two", steps("two"));
    let (a, b) = (fx.submit("one"), fx.submit("two"));
    fx.wait_state(&a, "Ended");
    fx.wait_state(&b, "Ended");
    let all = fx.d().events_of_daemon_after(0);
    assert_eq!(all.len(), 12, "two submitted events and ten records");
    let positions: Vec<u64> = all.iter().map(|e| e["position"].as_u64().unwrap()).collect();
    assert_eq!(positions, (1..=12).collect::<Vec<_>>(), "per daemon, from 1, strictly increasing");
    for run in [&a, &b] {
        let seq: Vec<u64> = fx.d().events_after(run, 0).iter().map(|e| e["sequence"].as_u64().unwrap()).collect();
        assert_eq!(seq, (1..=6).collect::<Vec<_>>(), "per run, from 1");
    }
    let runs_of: BTreeSet<_> = all.iter().map(|e| e["run_id"].as_str().unwrap()).collect();
    assert_eq!(runs_of.len(), 2);
    let after = fx.d().events_of_daemon_after(7);
    assert_eq!(after.iter().map(|e| e["position"].as_u64().unwrap()).collect::<Vec<_>>(), (8..=12).collect::<Vec<_>>());
    assert!(fx.d().events_of_daemon_after(12).is_empty());
    assert!(fx.d().events_after("no-such-run", 0).is_empty());
}

#[test]
fn sessions_group_runs_and_their_events() {
    let fx = Fx::new("sessions");
    for s in ["s1", "s2"] {
        let o = fx.d().open_session(OpenSession { session_id: s.into(), client: "test".into() }).unwrap();
        assert_eq!((o.name, o.status), ("opened", 202));
        assert_eq!(o.published[0].0, "sdc.daemon.SessionOpened");
    }
    let into = |s: &str| SubmitRun { runbook_id: "quick".into(), intake: "x".into(), session_id: Some(s.into()), ..SubmitRun::default() };
    let (a, b, c) = (fx.submit_with(into("s1")), fx.submit_with(into("s1")), fx.submit_with(into("s2")));
    for r in [&a, &b, &c] {
        fx.wait_state(r, "Ended");
    }
    let ids = |rows: Vec<Value>| rows.iter().map(|r| r["run_id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(ids(fx.d().runs_of_session("s1")), [b.clone(), a.clone()], "newest first");
    assert_eq!(ids(fx.d().runs_of_session("s2")), std::slice::from_ref(&c));
    assert!(fx.d().runs_of_session("s3").is_empty());
    assert_eq!(ids(fx.d().runs()), [c.clone(), b.clone(), a.clone()]);
    assert_eq!(ids(fx.d().runs_of_session_in_state("s1", "Ended").unwrap()).len(), 2);
    assert!(fx.d().runs_of_session_in_state("s1", "Running").unwrap().is_empty());
    assert_eq!(ids(fx.d().runs_in_state("Ended").unwrap()).len(), 3);
    let own: BTreeSet<String> = fx.d().events_of_session_after("s1", 0).iter().map(|e| e["run_id"].as_str().unwrap().to_string()).collect();
    assert_eq!(own, BTreeSet::from([a.clone(), b.clone()]));
    assert!(fx.d().events_of_session_after("s1", 0).iter().all(|e| e["session_id"] == "s1"));
    assert_eq!(fx.d().sessions().len(), 2);
    let s1 = &fx.d().sessions()[0];
    assert_eq!((s1["session_id"].as_str(), s1["state"].as_str(), s1["client"].as_str()), (Some("s1"), Some("Open"), Some("test")));
    // Closing: no new run, the runs it has stay listed.
    let closed = fx.d().close_session(CloseSession { session_id: "s1".into() }).unwrap();
    assert_eq!((closed.name, closed.status), ("closed", 202));
    let refused = fx.d().submit_run(into("s1")).unwrap();
    assert_eq!((refused.name, refused.status, refused.error), ("session-closed", 409, Some("SessionClosedToRuns")));
    assert_eq!(fx.d().runs_of_session("s1").len(), 2);
    assert_eq!(fx.d().sessions()[0]["state"], "Closed");
    assert!(fx.d().sessions()[0]["closed_at"].is_string());
}

#[test]
fn a_view_asked_for_a_state_that_does_not_exist_is_an_invalid_request() {
    let fx = Fx::new("badstate");
    assert!(matches!(fx.d().runs_in_state("Finished"), Err(Fault::Invalid(_))));
    assert!(matches!(fx.d().runs_of_session_in_state("s", "ended"), Err(Fault::Invalid(_))));
    let label = fx.d().open_session(OpenSession { session_id: "s".into(), client: "x".repeat(201) });
    assert!(matches!(label, Err(Fault::Invalid(_))));
    assert!(fx.d().open_session(OpenSession { session_id: "s".into(), client: "x".repeat(200) }).is_ok());
}

#[test]
fn the_floor_refuses_below_it_and_the_view_shows_it() {
    let fx = Fx::new("floor");
    let cfg = &fx.d().daemon_configuration()[0];
    assert_eq!(cfg["confinement_floor"], "linux");
    assert_eq!(cfg["daemon_id"], json!(fx.d().daemon_id()));
    let local = SubmitRun { runbook_id: "quick".into(), intake: "x".into(), confinement: Some(Confinement::Local), ..SubmitRun::default() };
    let o = fx.d().submit_run(local.clone()).unwrap();
    assert_eq!((o.name, o.status, o.error), ("below-floor", 409, Some("BelowConfinementFloor")));
    assert_eq!(o.payload.unwrap()["confinement"], "local");
    assert!(fx.d().runs().is_empty());
    let dir = scratch("floor-local");
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    let stub = Arc::new(StubRuntime::new());
    stub.script("quick", Script::new(vec![]));
    let d = Daemon::start(Config { confinement_floor: Confinement::Local, ..config(&dir) }, stub).unwrap();
    assert_eq!(d.submit_run(local).unwrap().name, "submitted", "local must be asked for by name, and a local floor takes it");
    assert_eq!(d.daemon_configuration()[0]["confinement_floor"], "local");
    d.stop();
}

#[test]
fn a_missing_session_is_refused_before_the_floor_and_an_existing_one_is_not() {
    let fx = Fx::new("floor-session");
    let low = |session: &str| SubmitRun {
        runbook_id: "quick".into(),
        intake: "x".into(),
        confinement: Some(Confinement::Local),
        session_id: Some(session.into()),
        ..SubmitRun::default()
    };
    // Both hold: below the floor, and the session does not exist. The specification answers no-session.
    let o = fx.d().submit_run(low("none")).unwrap();
    assert_eq!((o.name, o.status, o.error), ("no-session", 409, Some("NoSuchSession")));
    // The session exists, open: the floor refuses.
    let opened = fx.d().open_session(OpenSession { session_id: "s".into(), client: "test".into() }).unwrap();
    assert_eq!(opened.name, "opened");
    let o = fx.d().submit_run(low("s")).unwrap();
    assert_eq!((o.name, o.status, o.error), ("below-floor", 409, Some("BelowConfinementFloor")));
    // The session exists, closed: below-floor is declared above session-closed, so the floor still refuses.
    assert_eq!(fx.d().close_session(CloseSession { session_id: "s".into() }).unwrap().name, "closed");
    let o = fx.d().submit_run(low("s")).unwrap();
    assert_eq!((o.name, o.status, o.error), ("below-floor", 409, Some("BelowConfinementFloor")));
    assert!(fx.d().runs().is_empty());
}

#[test]
fn after_a_restart_the_runs_the_runtime_holds_no_more_end_as_orphaned_and_the_others_keep_their_state() {
    let mut fx = Fx::new("restart");
    let ended = fx.submit("quick");
    fx.wait_state(&ended, "Ended");
    let cancelled = fx.submit("hold");
    fx.wait_state(&cancelled, "Running");
    fx.cancel(&cancelled);
    let waiting = fx.submit("ask");
    fx.wait_open_decisions(&waiting, 1);
    let running = fx.submit("hold");
    fx.wait_state(&running, "Running");
    fx.d().open_session(OpenSession { session_id: "s1".into(), client: "c".into() }).unwrap();
    let before_detail: Vec<Value> = [&ended, &cancelled].iter().map(|r| fx.d().run_detail(r)[0].clone()).collect();
    let last_position = fx.d().events_of_daemon_after(0).last().unwrap()["position"].as_u64().unwrap();
    fx.restart();
    // The stub runtime answers `gone` for each run it is asked about: both live runs end as orphaned.
    let states: Vec<_> = fx.d().runs().iter().map(|r| r["state"].as_str().unwrap().to_string()).collect();
    assert_eq!(states, ["Ended", "Ended", "Cancelled", "Ended"], "newest first: running, waiting, cancelled, ended");
    let after_detail: Vec<Value> = [&ended, &cancelled].iter().map(|r| fx.d().run_detail(r)[0].clone()).collect();
    assert_eq!(after_detail, before_detail, "a run in no live state is untouched");
    assert_eq!(fx.d().sessions().len(), 1);
    for r in [&waiting, &running] {
        let row = &fx.d().run_detail(r)[0];
        assert_eq!((row["state"].as_str(), row["outcome"].as_str()), (Some("Ended"), Some("orphaned")), "{row}");
        assert!(row["ended_at"].is_string());
        let events = fx.d().events_after(r, 0);
        let orphaned = kinds(&events).iter().filter(|k| *k == "runbook.run-orphaned").count();
        assert_eq!((orphaned, kinds(&events).last().unwrap().as_str()), (1, "runbook.run-orphaned"), "{events:?}");
        assert!(!kinds(&events).contains(&"runbook.orphaned".to_string()), "the name of C1 is replaced");
    }
    for r in [&ended, &cancelled] {
        assert!(!kinds(&fx.d().events_after(r, 0)).contains(&"runbook.run-orphaned".to_string()));
    }
    // Its Open request is withdrawn the way a cancel does it, and shows no more as open.
    assert_eq!(fx.d().decisions_of_run(&waiting)[0]["state"], "Withdrawn");
    assert!(fx.d().open_decisions_of_run(&waiting).is_empty());
    assert_eq!(fx.answer(&waiting, "q1").name, "withdrawn");
    // Positions go on from where they were.
    let orphans = fx.d().events_of_daemon_after(last_position);
    assert_eq!(orphans.len(), 2);
    assert_eq!(orphans[0]["position"], last_position + 1);
    // Another restart changes nothing more for them: no event, no state.
    let runs = fx.d().runs();
    fx.restart();
    assert_eq!(fx.d().events_of_daemon_after(last_position).len(), 2);
    assert_eq!(fx.d().runs(), runs);
    // An orphaned run has ended: a client cancel is refused.
    assert_eq!(fx.cancel(&waiting).name, "already-ended");
    assert_eq!(fx.cancel(&running).name, "already-ended");
    // A new run after the restart takes the next position and its own number 1.
    let fresh = fx.submit("quick");
    let events = fx.d().events_after(&fresh, 0);
    assert_eq!(events[0]["sequence"], 1);
    assert!(events[0]["position"].as_u64().unwrap() > last_position + 2);
}

/// A runtime whose store lets a run go on: it answers `Resumed` for every run it is asked about.
struct Resuming(Arc<StubRuntime>);

impl Runtime for Resuming {
    fn recover(&self, _run_id: &str) -> Recovery {
        Recovery::Resumed
    }
    fn knows_runbook(&self, runbook: &str, version: Option<&str>) -> bool {
        self.0.knows_runbook(runbook, version)
    }
    fn runbooks(&self) -> Vec<RunbookInfo> {
        self.0.runbooks()
    }
    fn kind(&self) -> &'static str {
        self.0.kind()
    }
    fn start(&self, request: StartRequest, sink: &dyn EventSink, cancel: &AtomicBool) -> Result<Finish, RuntimeFailure> {
        self.0.start(request, sink, cancel)
    }
    fn answer(&self, run_id: &str, request_id: &str, answer: &str, principal: &str) -> Result<(), AnswerFailure> {
        self.0.answer(run_id, request_id, answer, principal)
    }
    fn cancel(&self, run_id: &str) {
        self.0.cancel(run_id)
    }
}

#[test]
fn a_run_the_runtime_resumes_is_left_as_it_is_and_asked_about_again_at_the_next_start() {
    let mut fx = Fx::new("restart-resumed");
    let running = fx.submit("hold");
    fx.wait_state(&running, "Running");
    let waiting = fx.submit("ask");
    fx.wait_open_decisions(&waiting, 1);
    let before = fx.d().events_of_daemon_after(0).len();
    fx.restart_with(Arc::new(Resuming(fx.stub.clone())));
    assert_eq!((fx.state_of(&running), fx.state_of(&waiting)), ("Running".to_string(), "WaitingOnDecision".to_string()));
    assert_eq!(fx.d().open_decisions_of_run(&waiting).len(), 1, "its request is still open");
    assert_eq!(fx.d().events_of_daemon_after(0).len(), before, "the daemon wrote nothing for a run that resumed");
    // The next start asks again; this runtime holds neither run.
    fx.restart();
    assert_eq!((fx.state_of(&running), fx.state_of(&waiting)), ("Ended".to_string(), "Ended".to_string()));
    assert_eq!(fx.d().events_of_daemon_after(0).len(), before + 2);
}

#[test]
fn end_orphaned_run_ends_a_live_run_with_the_cause_and_withdraws_its_requests() {
    let fx = Fx::new("end-orphaned");
    let d = fx.d();
    let running = fx.submit("hold");
    fx.wait_state(&running, "Running");
    let waiting = fx.submit("ask2");
    fx.wait_open_decisions(&waiting, 2);
    let o = d.end_orphaned_run(&waiting).unwrap();
    assert_eq!((o.command, o.name, o.status, o.error), ("end-orphaned-run", "orphaned", 202, None));
    assert_eq!(o.published.len(), 1);
    assert_eq!((o.published[0].0, o.published[0].1.clone()), ("sdc.daemon.RunOrphaned", json!({"run_id": waiting})));
    let row = &d.run_detail(&waiting)[0];
    assert_eq!((row["state"].as_str(), row["outcome"].as_str()), (Some("Ended"), Some("orphaned")));
    assert!(row["ended_at"].is_string());
    let decisions = d.decisions_of_run(&waiting);
    assert_eq!(decisions.iter().map(|x| x["state"].as_str().unwrap()).collect::<Vec<_>>(), ["Withdrawn", "Withdrawn"]);
    assert!(d.open_decisions_of_run(&waiting).is_empty());
    assert_eq!(kinds(&d.events_after(&waiting, 0)).iter().filter(|k| *k == "runbook.run-orphaned").count(), 1);
    // A run that is Running ends the same way, with no request to withdraw.
    assert_eq!(d.end_orphaned_run(&running).unwrap().name, "orphaned");
    assert_eq!(fx.state_of(&running), "Ended");
    // Refusals, by the specification's names: nothing is live any more, and an unknown run.
    for run in [&running, &waiting] {
        let o = d.end_orphaned_run(run).unwrap();
        assert_eq!((o.name, o.status, o.error, o.payload), ("wrong-state", 409, Some("RunStateConflict"), Some(json!({"run_id": run}))));
        assert!(o.published.is_empty());
    }
    let ended = fx.submit("quick");
    fx.wait_state(&ended, "Ended");
    assert_eq!(d.end_orphaned_run(&ended).unwrap().name, "wrong-state");
    assert_eq!(d.run_detail(&ended)[0]["outcome"], "done", "an ended run keeps its own outcome");
    let o = d.end_orphaned_run("none").unwrap();
    assert_eq!((o.name, o.status, o.error), ("no-run", 404, Some("NoSuchRun")));
    // An orphan has ended: a client's cancel of it is refused.
    assert_eq!(fx.cancel(&waiting).name, "already-ended");
}

#[test]
fn a_run_that_was_only_submitted_when_the_daemon_stopped_is_failed_to_start() {
    let dir = scratch("restart-submitted");
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    {
        let mut st = State::open(&dir.join("state")).unwrap();
        st.commit(Fact::RunSubmitted(Run {
            run_id: "r1".into(),
            runbook_id: "quick".into(),
            runbook_version: None,
            intake: "x".into(),
            context_path: None,
            settings: BTreeMap::new(),
            confinement: Confinement::Linux,
            session_id: None,
            idempotency_key: None,
            submitted_at: "t".into(),
            started_at: None,
            ended_at: None,
            outcome: None,
            failure: None,
            state: RunState::Submitted,
        }))
        .unwrap();
    }
    let d = Daemon::start(config(&dir), Arc::new(StubRuntime::new())).unwrap();
    let row = &d.run_detail("r1")[0];
    assert_eq!(row["state"], "FailedToStart");
    assert!(row["failure"].as_str().unwrap().contains("daemon stopped"));
    assert_eq!(kinds(&d.events_after("r1", 0)), ["runbook.start-failed"]);
    d.stop();
}

// ---- the refusals of every client command, read from the specification ----

fn domain() -> serde_yaml::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ess/domains/daemon.yaml");
    serde_yaml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn short(name: &str) -> String {
    name.rsplit('.').next().unwrap().to_string()
}

/// (command wire name, outcome name, error short name or "") of every outcome the domain declares.
fn declared() -> Vec<(String, String, String)> {
    let doc = domain();
    let mut out = vec![];
    for c in doc["commands"].as_sequence().unwrap() {
        let wire = c["naming"]["wire"].as_str().unwrap().to_string();
        for o in c["outcomes"].as_sequence().unwrap() {
            let error = o["error"].as_str().map(short).unwrap_or_default();
            out.push((wire.clone(), o["name"].as_str().unwrap().to_string(), error));
        }
    }
    out
}

#[test]
fn the_status_of_every_client_outcome_is_the_one_the_openapi_document_gives_it() {
    for command in CLIENT_COMMANDS {
        let in_doc = super::openapi::statuses(command);
        let have: BTreeMap<String, u16> = OUTCOMES.iter().filter(|s| s.command == command).map(|s| (s.outcome.to_string(), s.status)).collect();
        assert_eq!(have, in_doc, "{command}: outcome and status of runbook/src/commands.rs against runbookd.yaml");
    }
}

#[test]
fn the_outcome_table_is_the_domain_file() {
    let want: BTreeSet<_> = declared().into_iter().collect();
    let have: BTreeSet<_> = OUTCOMES.iter().map(|s| (s.command.to_string(), s.outcome.to_string(), s.error.unwrap_or("").to_string())).collect();
    assert_eq!(have, want, "an outcome added to or removed from ess/domains/daemon.yaml must be handled in runbook/src/commands.rs");
}

#[test]
fn the_outcome_table_lists_every_command_in_the_order_of_the_domain_file() {
    let mut want: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (command, outcome, _) in declared() {
        want.entry(command).or_default().push(outcome);
    }
    let mut have: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for s in OUTCOMES {
        have.entry(s.command.to_string()).or_default().push(s.outcome.to_string());
    }
    assert_eq!(have, want, "the outcomes of each command in runbook/src/commands.rs OUTCOMES are in the order of ess/domains/daemon.yaml");
}

const CLIENT_COMMANDS: [&str; 5] = ["open-session", "close-session", "submit-run", "cancel-run", "answer-decision"];

/// A way to make each refusal of a client command happen through the command layer.
fn trigger(fx: &Fx, command: &str, outcome: &str) -> Option<Outcome> {
    let d = fx.d();
    let submit = |f: &dyn Fn(&mut SubmitRun)| {
        let mut s = SubmitRun { runbook_id: "quick".into(), intake: "x".into(), ..SubmitRun::default() };
        f(&mut s);
        d.submit_run(s).unwrap()
    };
    let ask_run = |book: &str| {
        let run = fx.submit(book);
        fx.wait_open_decisions(&run, if book == "ask2" { 2 } else { 1 });
        run
    };
    Some(match (command, outcome) {
        ("open-session", "session-exists") => {
            d.open_session(OpenSession { session_id: "s".into(), client: "c".into() }).unwrap();
            d.open_session(OpenSession { session_id: "s".into(), client: "other".into() }).unwrap()
        }
        ("close-session", "no-session") => d.close_session(CloseSession { session_id: "none".into() }).unwrap(),
        ("close-session", "wrong-state") => {
            d.open_session(OpenSession { session_id: "s".into(), client: "c".into() }).unwrap();
            d.close_session(CloseSession { session_id: "s".into() }).unwrap();
            d.close_session(CloseSession { session_id: "s".into() }).unwrap()
        }
        ("submit-run", "intake-too-large") => submit(&|s| s.intake = "a".repeat(MAX_INTAKE_BYTES + 1)),
        ("submit-run", "no-runbook") => submit(&|s| s.runbook_id = "nothing".into()),
        ("submit-run", "context-outside-roots") => submit(&|s| s.context_path = Some(fx.dir.to_string_lossy().into_owned())),
        ("submit-run", "no-daemon") => d.submit_run_for("another-daemon", SubmitRun { runbook_id: "quick".into(), intake: "x".into(), ..SubmitRun::default() }).unwrap(),
        ("submit-run", "below-floor") => submit(&|s| s.confinement = Some(Confinement::Local)),
        ("submit-run", "no-session") => submit(&|s| s.session_id = Some("none".into())),
        ("submit-run", "session-closed") => {
            d.open_session(OpenSession { session_id: "s".into(), client: "c".into() }).unwrap();
            d.close_session(CloseSession { session_id: "s".into() }).unwrap();
            submit(&|s| s.session_id = Some("s".into()))
        }
        ("cancel-run", "no-run") => d.cancel_run(CancelRun { run_id: "none".into() }).unwrap(),
        ("cancel-run", "already-ended") => {
            let run = fx.submit("quick");
            fx.wait_state(&run, "Ended");
            fx.cancel(&run)
        }
        ("answer-decision", "no-request") => fx.answer("run", "none"),
        ("answer-decision", "not-of-run") => {
            let run = ask_run("ask");
            fx.answer("another-run", d.open_decisions_of_run(&run)[0]["request_id"].as_str().unwrap())
        }
        ("answer-decision", "withdrawn") => {
            let run = ask_run("ask");
            fx.cancel(&run);
            fx.answer(&run, "q1")
        }
        ("answer-decision", "already-answered") => {
            let run = ask_run("ask");
            fx.answer(&run, "q1");
            fx.wait_state(&run, "Ended");
            fx.answer(&run, "q1")
        }
        _ => return None,
    })
}

/// Refusals a request cannot reach through the command layer: the test named beside each makes the
/// state by hand (a journal written before the daemon starts).
const TESTED_BY_HAND: [(&str, &str); 2] = [("answer-decision", "no-run"), ("answer-decision", "run-not-waiting")];

#[test]
fn every_refusal_of_every_client_command_has_a_trigger_and_the_specification_s_name() {
    let mut untested = vec![];
    for (command, outcome, error) in declared().into_iter().filter(|(c, _, e)| CLIENT_COMMANDS.contains(&c.as_str()) && !e.is_empty()) {
        if TESTED_BY_HAND.contains(&(command.as_str(), outcome.as_str())) {
            continue; // the two tests below: a journal written before the daemon starts
        }
        let fx = Fx::new(&format!("refusal-{command}-{outcome}"));
        match trigger(&fx, &command, &outcome) {
            None => untested.push(format!("{command}/{outcome}")),
            Some(o) => {
                assert_eq!((o.command, o.name), (command.as_str(), outcome.as_str()));
                assert_eq!(o.error, Some(error.as_str()), "{command}/{outcome}");
                assert!(o.is_refusal() && o.published.is_empty() && o.payload.is_some(), "{o:?}");
                assert_eq!(o.body()["outcome"], outcome.as_str());
                assert_eq!(o.body()["error"], format!("sdc.daemon.{error}"));
                super::openapi::check_response(&command, &outcome, &o.body());
            }
        }
    }
    assert!(untested.is_empty(), "a refusal in ess/domains/daemon.yaml with no trigger in this test: {untested:?}");
}

#[test]
fn an_answer_for_a_request_whose_run_is_not_there_is_refused_no_such_run() {
    let dir = scratch("ghost");
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    {
        let mut st = State::open(&dir.join("state")).unwrap();
        st.commit(Fact::DaemonStarted(DaemonRow { daemon_id: "old".into(), confinement_floor: Confinement::Linux, principal: "agent:old".into(), started_at: "t".into() })).unwrap();
        st.commit(Fact::DecisionRequested(DecisionRequest {
            request_id: "ghost-q".into(),
            run_id: "ghost".into(),
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
    let d = Daemon::start(config(&dir), Arc::new(StubRuntime::new())).unwrap();
    let o = d.answer_decision(AnswerDecision { run_id: "ghost".into(), request_id: "ghost-q".into(), answer: "A".into(), answered_by: "me".into() }).unwrap();
    assert_eq!((o.name, o.status, o.error), ("no-run", 409, Some("NoSuchRun")));
    d.stop();
}

fn journal_run(id: &str) -> Run {
    Run {
        run_id: id.into(),
        runbook_id: "quick".into(),
        runbook_version: None,
        intake: "x".into(),
        context_path: None,
        settings: BTreeMap::new(),
        confinement: Confinement::Linux,
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

fn journal_request(id: &str, run: &str) -> DecisionRequest {
    DecisionRequest {
        request_id: id.into(),
        run_id: run.into(),
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
    }
}

#[test]
fn an_open_request_whose_run_does_not_wait_is_refused_run_not_waiting() {
    // The daemon withdraws a cancelled run's requests in the same step, so only a journal can hold an
    // Open request of a Cancelled run: the one case left for RunNotWaiting.
    let dir = scratch("not-waiting");
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    {
        let mut st = State::open(&dir.join("state")).unwrap();
        st.commit(Fact::RunSubmitted(journal_run("r1"))).unwrap();
        st.commit(Fact::DecisionRequested(journal_request("q1", "r1"))).unwrap();
        st.commit(Fact::RunCancelled { run_id: "r1".into(), at: "t".into() }).unwrap();
    }
    let d = Daemon::start(config(&dir), Arc::new(StubRuntime::new())).unwrap();
    let o = d.answer_decision(AnswerDecision { run_id: "r1".into(), request_id: "q1".into(), answer: "A".into(), answered_by: "me".into() }).unwrap();
    assert_eq!((o.name, o.status, o.error), ("run-not-waiting", 409, Some("RunNotWaiting")));
    assert_eq!(o.payload, Some(json!({"run_id": "r1"})));
    assert_eq!(d.decisions_of_run("r1")[0]["state"], "Open", "nothing changed");
    d.stop();
}

#[test]
fn the_runtime_side_commands_refuse_by_the_specification_s_names() {
    let fx = Fx::new("runtime-side");
    let d = fx.d();
    let name = |o: Outcome| (o.name, o.status);
    assert_eq!(name(d.start_run("none").unwrap()), ("no-run", 404));
    assert_eq!(name(d.fail_run_start("none", "r").unwrap()), ("no-run", 404));
    assert_eq!(name(d.wait_on_decision("none").unwrap()), ("no-run", 404));
    assert_eq!(name(d.resume_run("none").unwrap()), ("no-run", 404));
    assert_eq!(name(d.end_run("none", "o").unwrap()), ("no-run", 404));
    assert_eq!(name(d.withdraw_decision("none").unwrap()), ("no-request", 404));
    assert_eq!(name(d.record_run_event("none", "k", json!({})).unwrap()), ("no-run", 409));
    let request = ask("q9");
    let req = runbook::runtime::DecisionRequest {
        request_id: request.request_id.clone(),
        case_id: request.case_id.clone(),
        action: request.action.clone(),
        capability: request.capability.clone(),
        question: request.question.clone(),
        options: request.options.clone(),
    };
    assert_eq!(name(d.request_decision("none", req.clone()).unwrap()), ("no-run", 409));

    let run = fx.submit("hold");
    fx.wait_state(&run, "Running");
    assert_eq!(name(d.start_run(&run).unwrap()), ("wrong-state", 409), "already running");
    assert_eq!(name(d.fail_run_start(&run, "r").unwrap()), ("wrong-state", 409));
    assert_eq!(name(d.resume_run(&run).unwrap()), ("wrong-state", 409));
    assert_eq!(name(d.request_decision(&run, req.clone()).unwrap()), ("run-not-waiting", 409));
    assert_eq!(name(d.wait_on_decision(&run).unwrap()), ("waiting", 202));
    assert_eq!(name(d.wait_on_decision(&run).unwrap()), ("wrong-state", 409), "already waiting");
    assert_eq!(name(d.end_run(&run, "o").unwrap()), ("wrong-state", 409), "waiting is not running");
    assert_eq!(name(d.request_decision(&run, req.clone()).unwrap()), ("requested", 202));
    assert_eq!(name(d.resume_run(&run).unwrap()), ("still-open", 409));
    assert!(matches!(d.request_decision(&run, req), Err(Fault::Invalid(_))), "the runtime mints unique ids");
    assert_eq!(name(d.withdraw_decision("q9").unwrap()), ("withdrawn", 202));
    assert_eq!(name(d.withdraw_decision("q9").unwrap()), ("not-open", 409));
    assert_eq!(name(d.resume_run(&run).unwrap()), ("resumed", 202));
    assert_eq!(name(d.end_run(&run, "o").unwrap()), ("ended", 202));
    assert_eq!(name(d.record_run_event(&run, "k", json!({})).unwrap()), ("run-closed", 409));
    fx.d().stop();
}

#[test]
fn an_event_whose_numbers_do_not_grow_is_refused() {
    let fx = Fx::new("numbers");
    let run = fx.submit("hold");
    fx.wait_state(&run, "Running");
    let d = fx.d();
    let last = d.events_of_daemon_after(0).last().unwrap()["position"].as_u64().unwrap();
    let seq = d.events_after(&run, 0).last().unwrap()["sequence"].as_u64().unwrap();
    let go = |sequence, position| {
        let o = d.record_run_event_numbered(&run, sequence, position, "k", json!({"x": 1})).unwrap();
        (o.name, o.error)
    };
    assert_eq!(go(seq, last + 1), ("not-ascending", Some("SequenceNotAscending")));
    assert_eq!(go(seq + 1, last), ("position-not-ascending", Some("PositionNotAscending")));
    assert_eq!(go(seq + 1, last + 1), ("recorded", None));
    assert_eq!(go(seq + 5, last + 9), ("recorded", None), "gaps are allowed, going back is not");
    assert_eq!(d.events_after(&run, 0).len(), 3);
}
