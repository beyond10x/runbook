//! Adversary cases of wave 6 against the serial key (`story:a-run-waits-for-its-serial-key`, `docs/daemon-api.md`,
//! "Runs of one serial key"): "Runs with one key run one at a time".
//!
//! In process. The runtime is the stub with one more runbook, `linger`: a run that holds until it is cancelled and then
//! takes a while before its runtime lets it go, as the engine does after a cancel (the kill of the program in flight,
//! then up to `ROUTE_DRAIN`, 5 s, for a model call still in flight: `runbook/src/engine/mod.rs`).

use super::commands::{config, start};
use super::state::scratch;
use runbook::commands::{CancelRun, Daemon, SubmitRun};
use runbook::runtime::{AnswerFailure, EventSink, Finish, Recovery, RunbookInfo, Runtime, RuntimeFailure, StartRequest};
use runbook::stub::{Script, Step, StubRuntime};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long a `linger` run's runtime still runs after its cancel.
const LINGER: Duration = Duration::from_millis(400);

#[derive(Default)]
struct Seen {
    /// When the runtime let the `linger` run go after its cancel.
    released: Option<Instant>,
    /// When the runtime took up the `quick` run.
    started: Option<Instant>,
}

struct Lingering {
    stub: Arc<StubRuntime>,
    seen: Arc<Mutex<Seen>>,
}

impl Runtime for Lingering {
    fn recover(&self, run_id: &str) -> Recovery {
        self.stub.recover(run_id)
    }

    fn knows_runbook(&self, runbook: &str, version: Option<&str>) -> bool {
        runbook == "linger" || self.stub.knows_runbook(runbook, version)
    }

    fn runbooks(&self) -> Vec<RunbookInfo> {
        let mut all = self.stub.runbooks();
        all.push(RunbookInfo { name: "linger".into(), version: None });
        all
    }

    fn kind(&self) -> &'static str {
        "stub"
    }

    fn start(&self, request: StartRequest, sink: &dyn EventSink, cancel: &AtomicBool) -> Result<Finish, RuntimeFailure> {
        if request.runbook != "linger" {
            self.seen.lock().unwrap().started.get_or_insert_with(Instant::now);
            return self.stub.start(request, sink, cancel);
        }
        sink.started();
        while !cancel.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(5));
        }
        std::thread::sleep(LINGER);
        self.seen.lock().unwrap().released = Some(Instant::now());
        Ok(Finish::Cancelled)
    }

    fn answer(&self, run_id: &str, request_id: &str, answer: &str, principal: &str) -> Result<(), AnswerFailure> {
        self.stub.answer(run_id, request_id, answer, principal)
    }

    fn cancel(&self, run_id: &str) {
        self.stub.cancel(run_id);
    }
}

fn state_of(d: &Daemon, run: &str) -> String {
    d.run_detail(run)[0]["state"].as_str().unwrap().to_owned()
}

fn wait_state(d: &Daemon, run: &str, want: &str) {
    let until = Instant::now() + Duration::from_secs(10);
    while state_of(d, run) != want {
        assert!(Instant::now() < until, "run {run} is {} and not {want}", state_of(d, run));
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn submit(d: &Daemon, book: &str, key: &str) -> String {
    let o = d.submit_run(SubmitRun { runbook_id: book.into(), intake: "x".into(), serial_key: Some(key.into()), ..SubmitRun::default() }).unwrap();
    assert_eq!(o.name, "submitted", "{o:?}");
    o.run_id.unwrap()
}

/// A cancel of the run that holds the key: the next run of the key starts while the runtime still runs the cancelled
/// one, because the key is read from the run's state (Cancelled at once) and not from the end of its runtime. For the
/// time between, two runs of one key run at once.
#[test]
fn the_next_run_of_a_key_starts_only_after_the_runtime_let_the_cancelled_holder_go() {
    let dir = scratch("adv-w6-linger");
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    std::fs::create_dir_all(dir.join("runbooks")).unwrap();
    let stub = Arc::new(StubRuntime::new());
    stub.script("quick", Script::new(vec![Step::End("done".into())]));
    let seen = Arc::new(Mutex::new(Seen::default()));
    let d = start(config(&dir), Arc::new(Lingering { stub, seen: seen.clone() })).unwrap();

    let holder = submit(&d, "linger", "k");
    let next = submit(&d, "quick", "k");
    wait_state(&d, &holder, "Running");
    wait_state(&d, &next, "WaitingOnKey");
    assert_eq!(d.cancel_run(CancelRun { run_id: holder.clone() }).unwrap().name, "cancelled");
    wait_state(&d, &next, "Ended");
    let until = Instant::now() + Duration::from_secs(5);
    while seen.lock().unwrap().released.is_none() {
        assert!(Instant::now() < until, "the linger run was never let go");
        std::thread::sleep(Duration::from_millis(5));
    }
    let s = seen.lock().unwrap();
    let (released, started) = (s.released.unwrap(), s.started.unwrap());
    drop(s);
    d.stop();
    assert!(
        started >= released,
        "the run behind the key started {:?} before the runtime let the cancelled holder go",
        released.saturating_duration_since(started)
    );
}
