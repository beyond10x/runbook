//! What keeps a daemon run inside its caps (`crate::caps`): before each step the wall time and the
//! model calls used are held against their caps; a bound program that names a per-call budget
//! reserves it in the spend ledger of the runbook as it starts; after the step, the cost records the
//! bound programs wrote are booked. The ledger, the cost reader and the call-budget reader are the
//! ones `runbook run` uses.
//!
//! The spend cap is the run's own: a reservation is held against the cap less what this run spent and
//! holds, and what other runs of the runbook spent or hold never reduces it. The ledger is one file
//! per runbook, with each entry and each reservation keyed by the run.
//!
//! What a program reserves as it starts, a step's or one a person's answer starts, is its own budget
//! (`--max-budget-usd <v>` or `--max-budget-usd=<v>` in that program's own arguments, which come from
//! the bindings the run reads and nowhere else; else the member `max_budget_usd` of its action, else
//! that of its bindings file: `budget::Reserves`). A program whose budget no source names may call a
//! model all the same: it reserves the largest budget the runbook's bindings name, and a runbook with
//! such a program and no budget anywhere is refused at load (`budget::reserves`). What a program
//! costs is booked from the cost records after it, and the next step is refused once the cap is
//! reached.
//!
//! A run on a route ([`Gate::route`]) books its model calls where they are relayed (`crate::relay`): each
//! call is taken against the model calls and the spend of the run before it is sent ([`RelayBooking`]),
//! and its line in the ledger is its cost; the cost records of the bound programs then book nothing.

use crate::budget;
use crate::caps::{Caps, Usage};
use crate::cost::CostReader;
use crate::ledger::{Ledger, Reservation};
use crate::runtime::EventSink;
use b10x_loom_intake_slice::confinement::{Backend, ChildCommand, ConfinementError, ConfinementRefusal, TestExecution, TestRunner};
use b10x_loom_intake_slice::executor::TestCommand;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// The file of a run's usage, in the run's own directory: what a restarted daemon and the view of an
/// ended run read.
pub const USAGE_FILE: &str = "caps-used.json";

/// The cap a run reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cap {
    Wall,
    Calls,
    Spend,
}

impl Cap {
    /// The outcome of a run stopped at this cap, the words `runbook run` uses.
    pub fn outcome(self) -> &'static str {
        match self {
            Cap::Wall => "stopped: wall time",
            Cap::Calls => "stopped: llm calls",
            Cap::Spend => "stopped: spend cap",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Cap::Wall => "wall time",
            Cap::Calls => "llm calls",
            Cap::Spend => "spend cap",
        }
    }
}

/// What the wall cap of a run is held against. A running daemon has one: the host's monotonic
/// clock. A test that builds the engine in process and needs the wall cap spent at one moment (while
/// a given program runs, say) gives it a clock that stands still until the test moves it
/// (`Engine::with_wall_clock`); no option, environment variable or request sets one.
#[derive(Clone, Default)]
pub struct WallClock(Option<Arc<AtomicU64>>);

impl WallClock {
    /// A clock that stands at 0 seconds until [`WallClock::advance`] moves it.
    pub fn standing() -> WallClock {
        WallClock(Some(Arc::new(AtomicU64::new(0))))
    }

    /// Moves a standing clock on by `secs`; the host's clock is not moved.
    pub fn advance(&self, secs: u64) {
        if let Some(at) = &self.0 {
            at.fetch_add(secs, Ordering::SeqCst);
        }
    }

    fn standing_at(&self) -> u64 {
        self.0.as_ref().map_or(0, |at| at.load(Ordering::SeqCst))
    }
}

/// What a run has used: shared by the run's thread and the view of the run.
pub struct Meter {
    wall_before: u64,
    started: Instant,
    clock: WallClock,
    standing_from: u64,
    calls: AtomicU64,
    spent: AtomicU64,
}

impl Meter {
    /// A meter whose wall seconds `clock` counts.
    pub fn on(before: Usage, clock: WallClock) -> Meter {
        let standing_from = clock.standing_at();
        Meter { wall_before: before.wall_seconds, started: Instant::now(), clock, standing_from, calls: AtomicU64::new(before.llm_calls), spent: AtomicU64::new(before.spent_micro_usd) }
    }

    pub fn usage(&self) -> Usage {
        let ran = match self.clock.0 {
            None => self.started.elapsed().as_secs(),
            Some(_) => self.clock.standing_at().saturating_sub(self.standing_from),
        };
        Usage { wall_seconds: self.wall_before + ran, llm_calls: self.calls.load(Ordering::SeqCst), spent_micro_usd: self.spent.load(Ordering::SeqCst) }
    }
}

/// The usage a run left in its directory, if it left one.
pub fn read_usage(run_dir: &Path) -> Option<Usage> {
    let v: Value = serde_json::from_slice(&std::fs::read(run_dir.join(USAGE_FILE)).ok()?).ok()?;
    let n = |k: &str| v.get(k).and_then(Value::as_u64);
    Some(Usage { wall_seconds: n("wall_seconds")?, llm_calls: n("llm_calls")?, spent_micro_usd: n("spent_micro_usd")? })
}

fn write_usage(run_dir: &Path, u: Usage) -> Result<(), String> {
    let text = json!({"wall_seconds": u.wall_seconds, "llm_calls": u.llm_calls, "spent_micro_usd": u.spent_micro_usd}).to_string() + "\n";
    let tmp = run_dir.join(format!("{USAGE_FILE}.tmp"));
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, run_dir.join(USAGE_FILE)).map_err(|e| format!("{}: {e}", run_dir.join(USAGE_FILE).display()))
}

/// Why a step was ended before its program started.
pub enum Stopped {
    /// The reservation for a program is less than the program may cost: the spend cap.
    Spend(String),
    /// The ledger could not be read or written: the run fails closed.
    Ledger(String),
}

/// What the run's thread and the runner of its programs share: the run's spend ledger and what the
/// step in progress holds.
struct Held {
    ledger: Ledger,
    run_id: String,
    cap_micro_usd: u64,
    /// What each program reserves: the budget its arguments or its binding name, else the largest
    /// budget of the runbook.
    reserves: budget::Reserves,
    reserved: Mutex<Vec<Reservation>>,
    stopped: Mutex<Option<Stopped>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Held {
    /// Reserves `want` for a program about to start. A grant under `want` is released and the step
    /// stops before the program starts.
    fn reserve(&self, want: u64) -> Result<(), ConfinementError> {
        let refuse = |why: &str| ConfinementError { reason: ConfinementRefusal::Cancelled, detail: why.to_owned() };
        let granted = match self.ledger.reserve_for_run(&self.run_id, self.cap_micro_usd, want) {
            Ok(r) => r,
            Err(why) => {
                *lock(&self.stopped) = Some(Stopped::Ledger(why));
                return Err(refuse("the spend ledger is not readable; nothing started"));
            }
        };
        if granted.micro_usd >= want {
            lock(&self.reserved).push(granted);
            return Ok(());
        }
        let outcome = self.ledger.release(&granted).and_then(|()| self.ledger.spent_by_run(&self.run_id));
        *lock(&self.stopped) = Some(match outcome {
            Ok(spent) => Stopped::Spend(format!("{spent} of {} micro USD spent, and the next call may cost {want}", self.cap_micro_usd)),
            Err(why) => Stopped::Ledger(why),
        });
        Err(refuse("the reservation for the program is less than it may cost; nothing started"))
    }
}

/// The runner every bound program of a daemon run starts through, a step's and an answer's: it
/// reserves the per-call budget a program names before the program starts (the largest budget of
/// the runbook for a program whose budget no source names), and starts nothing when the reservation is short
/// (the step or answer is then ended as a cancelled one, which the run reads as the spend cap).
struct Reserving {
    inner: Arc<dyn TestRunner>,
    held: Arc<Held>,
}

impl TestRunner for Reserving {
    fn run(&self, command: &TestCommand, root: &Path) -> Result<TestExecution, ConfinementError> {
        self.inner.run(command, root)
    }

    fn backend(&self) -> Backend {
        self.inner.backend()
    }

    fn label(&self) -> String {
        self.inner.label()
    }

    fn local_reason(&self) -> Option<&str> {
        self.inner.local_reason()
    }

    fn run_child(&self, child: &ChildCommand) -> Result<TestExecution, ConfinementError> {
        let want = self.held.reserves.of(child.program(), child.args());
        if want > 0 {
            self.held.reserve(want)?;
        }
        self.inner.run_child(child)
    }

    fn preflight_child(&self, root: &Path) -> Result<(), ConfinementError> {
        self.inner.preflight_child(root)
    }
}

/// What the relay of a run on a route asks of the run (`crate::relay::Booking`): each call is taken against the
/// run's caps of model calls and spend before it is sent, and booked into the run's spend ledger when its answer
/// has ended, at the cost the relay read; a call whose cost is not known books its reservation
/// ([`crate::relay::Booking::reservation_micro_usd`]). The lines wait here until the run's thread records them as
/// events of the run.
pub struct RelayBooking {
    held: Arc<Held>,
    meter: Arc<Meter>,
    max_calls: u64,
    step: Arc<Mutex<Option<String>>>,
    pending: Mutex<Vec<Value>>,
    /// What fails the step in flight: the first call of it the ledger could not book, or the first the route did
    /// not serve (`route-unavailable`).
    failed: Mutex<Option<String>>,
    /// What a call of the decider may cost (`max_call_micro_usd`); `u64::MAX` when it names no budget.
    max_call: AtomicU64,
    /// The calls taken and not yet booked.
    in_flight: AtomicU64,
    /// The run ended: what is booked now is no event of the run any more.
    closed: AtomicBool,
}

/// `counter + n`, held at `u64::MAX`.
fn saturating_add(counter: &AtomicU64, n: u64) {
    let _ = counter.try_update(Ordering::SeqCst, Ordering::SeqCst, |v| Some(v.saturating_add(n)));
}

impl RelayBooking {
    /// What a call of the decider may cost: the call of unknown cost outside a step books it.
    pub fn set_max_call(&self, micro_usd: u64) {
        self.max_call.store(micro_usd, Ordering::SeqCst);
    }

    /// The calls taken and not yet booked.
    pub fn in_flight(&self) -> u64 {
        self.in_flight.load(Ordering::SeqCst)
    }
}

impl crate::relay::Booking for RelayBooking {
    fn admit(&self) -> Result<(), String> {
        let max = self.max_calls;
        self.meter.calls.try_update(Ordering::SeqCst, Ordering::SeqCst, |n| (n < max).then_some(n + 1)).map_err(|n| format!("llm calls: {n} of {max} calls"))?;
        let cap = self.held.cap_micro_usd;
        let refused = match self.held.ledger.spent_by_run(&self.held.run_id) {
            Ok(spent) if spent >= cap => Some(format!("spend cap: {spent} of {cap} micro USD spent")),
            Ok(_) => None,
            Err(why) => Some(format!("the spend ledger cannot be read: {why}")),
        };
        match refused {
            Some(why) => {
                self.meter.calls.fetch_sub(1, Ordering::SeqCst);
                Err(why)
            }
            None => {
                saturating_add(&self.in_flight, 1);
                Ok(())
            }
        }
    }

    /// In a step: the largest reservation the step holds. Otherwise (a call of the decider, or a step that holds
    /// nothing): what a call of the decider may cost, at most what the run's spend cap has left
    /// (`docs/model-routes.md`, section 5).
    fn reservation_micro_usd(&self) -> u64 {
        let largest = lock(&self.held.reserved).iter().map(|r| r.micro_usd).max().unwrap_or(0);
        if largest > 0 && lock(&self.step).is_some() {
            return largest;
        }
        let left = self.held.cap_micro_usd.saturating_sub(self.held.ledger.spent_by_run(&self.held.run_id).unwrap_or(0));
        self.max_call.load(Ordering::SeqCst).min(left)
    }

    fn step(&self) -> Option<String> {
        lock(&self.step).clone()
    }

    fn book(&self, line: Value, micro_usd: u64) -> Result<(), String> {
        let booked = self.held.ledger.book_line(&self.held.run_id, &line, micro_usd);
        saturating_add(&self.meter.spent, micro_usd);
        let _ = self.in_flight.try_update(Ordering::SeqCst, Ordering::SeqCst, |n| Some(n.saturating_sub(1)));
        let text = |k: &str| line.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
        let failure = match &booked {
            Err(why) => Some(why.clone()),
            Ok(()) if line.get("cause").and_then(Value::as_str) == Some(crate::relay::ROUTE_UNAVAILABLE) && line.get("step").is_some_and(Value::is_string) => {
                Some(format!("{}: route {} (chosen by {}) failed the call: {}: {}", crate::relay::ROUTE_UNAVAILABLE, text("route"), text("source"), text("code"), text("detail")))
            }
            Ok(()) => None,
        };
        if let Some(why) = failure {
            lock(&self.failed).get_or_insert(why);
        }
        let mut pending = lock(&self.pending);
        if self.closed.load(Ordering::SeqCst) {
            // The run ended before this call did: its line is in the ledger, and no event of the run can take it.
            eprintln!("runbook: run {}: a model call ended after the run had ended; its ledger line: {line}", self.held.run_id);
        } else {
            pending.push(line);
        }
        drop(pending);
        booked
    }
}

pub struct Gate {
    pub caps: Caps,
    pub meter: Arc<Meter>,
    held: Arc<Held>,
    run_dir: PathBuf,
    workspace: PathBuf,
    reader: CostReader,
    /// The run is on a route: its model calls are taken and booked by the relay.
    routed: Option<Arc<RelayBooking>>,
}

impl Gate {
    /// The gate of a run: its ledger is `<state>/ledgers/<runbook>.json`, one per runbook, each entry
    /// and reservation keyed by the run. A reservation the run's id left in it (the run's earlier
    /// process ended with the step in progress) is dropped: a run that is not alive holds nothing.
    /// `resumed`: the run has a store from an earlier process, whose usage is read and whose cost
    /// records are not booked again. `reserves`: what each program reserves as it starts
    /// (`budget::reserves` of the bindings the run reads).
    #[allow(clippy::too_many_arguments)]
    pub fn open(caps: Caps, state_dir: &Path, runbook: &str, run_id: &str, run_dir: &Path, workspace: &Path, resumed: bool, meter: Arc<Meter>, reserves: budget::Reserves) -> Result<Gate, String> {
        let dir = state_dir.join("ledgers");
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let ledger = Ledger::open(&dir.join(format!("{runbook}.json")))?;
        ledger.release_run(run_id)?;
        let mut reader = CostReader::with_home(std::env::var("HOME").ok());
        if resumed {
            // what the earlier process wrote was booked then (or is lost with it): it is not booked again
            let _ = reader.read_new(workspace);
        }
        let held = Arc::new(Held { ledger, run_id: run_id.to_owned(), cap_micro_usd: caps.cap_micro_usd, reserves, reserved: Mutex::new(Vec::new()), stopped: Mutex::new(None) });
        Ok(Gate { caps, meter, held, run_dir: run_dir.to_path_buf(), workspace: workspace.to_path_buf(), reader, routed: None })
    }

    /// The run is on a route: from now on its model calls are taken against its caps and booked by the relay through
    /// what this returns, and a cost record of a bound program carries no cost (`CostReader::routed`). `step` holds
    /// the step in flight.
    pub fn route(&mut self, step: Arc<Mutex<Option<String>>>) -> Arc<RelayBooking> {
        let booking = Arc::new(RelayBooking { held: self.held.clone(), meter: self.meter.clone(), max_calls: self.caps.max_llm_calls, step, pending: Mutex::new(Vec::new()), failed: Mutex::new(None), max_call: AtomicU64::new(u64::MAX), in_flight: AtomicU64::new(0), closed: AtomicBool::new(false) });
        self.reader = std::mem::take(&mut self.reader).routed();
        self.routed = Some(booking.clone());
        booking
    }

    /// Records the model calls the relay booked since the last time as events of the run (`runbook.model-call`).
    pub fn record_calls(&self, sink: &dyn EventSink) {
        if let Some(booking) = &self.routed {
            for line in std::mem::take(&mut *lock(&booking.pending)) {
                sink.record("runbook.model-call", line);
            }
        }
    }

    /// The run has ended and its token is void: waits up to `wait` for the calls still in flight (their callers
    /// were ended with the run, the gateway still answers them), records every booked call as an event of the run,
    /// and records `runbook.model-calls-unfinished` when calls are still in flight then. Such a call is booked in the
    /// ledger when it ends and said on the daemon's stderr.
    pub fn close_route(&self, sink: &dyn EventSink, wait: std::time::Duration) {
        let Some(booking) = &self.routed else { return };
        let until = Instant::now() + wait;
        while booking.in_flight() > 0 && Instant::now() < until {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let lines = {
            let mut pending = lock(&booking.pending);
            booking.closed.store(true, Ordering::SeqCst);
            std::mem::take(&mut *pending)
        };
        for line in lines {
            sink.record("runbook.model-call", line);
        }
        let left = booking.in_flight();
        if left > 0 {
            sink.record(
                "runbook.model-calls-unfinished",
                json!({"in_flight": left, "detail": format!("{left} model call(s) had not ended when the run ended: each is booked in the spend ledger when it ends, and is no event of the run")}),
            );
        }
    }

    /// The runner of the run's bound programs: `inner`, reserving the budget of each program that
    /// names one as it starts.
    pub fn runner(&self, inner: Arc<dyn TestRunner>) -> Arc<dyn TestRunner> {
        Arc::new(Reserving { inner, held: self.held.clone() })
    }

    /// Why the step just run ended before a program started, if it did (taken once).
    pub fn take_stopped(&self) -> Option<Stopped> {
        lock(&self.held.stopped).take()
    }

    /// Whether the run's wall time is used up.
    pub fn wall_over(&self) -> bool {
        self.meter.usage().wall_seconds >= self.caps.max_wall_seconds
    }

    /// The caps held against the use before the next step: the model calls, then the spend of this
    /// run (the cap reached). The reservation of a program is held as it starts, against what this
    /// run has left.
    pub fn admit(&self) -> Result<Option<(Cap, String)>, String> {
        let used = self.meter.usage();
        if used.llm_calls >= self.caps.max_llm_calls {
            return Ok(Some((Cap::Calls, format!("{} of {} calls", used.llm_calls, self.caps.max_llm_calls))));
        }
        let cap = self.caps.cap_micro_usd;
        let spent = self.held.ledger.spent_by_run(&self.held.run_id)?;
        if spent >= cap {
            return Ok(Some((Cap::Spend, format!("{spent} of {cap} micro USD spent"))));
        }
        Ok(None)
    }

    /// Books what the step spent: the cost records written since the last step, at their cost, and a
    /// call that failed with no cost at its reservation (the largest the step held). The step's
    /// reservations are released: the first is committed with the whole cost, the others released. A
    /// record that cannot be read books the reservations and is an error (fail closed).
    pub fn book(&mut self, sink: &dyn EventSink) -> Result<(), String> {
        let held: Vec<Reservation> = std::mem::take(&mut *lock(&self.held.reserved));
        let largest = held.iter().map(|r| r.micro_usd).max().unwrap_or(0);
        let result = self.reader.read_new(&self.workspace);
        let routed = self.routed.is_some();
        let (booked, what, calls, outcome) = match result {
            // The relay took and booked every model call of the step; the step's reservations are released. A call
            // the ledger could not book fails the step (fail closed).
            Ok(_) if routed => (0, "step", 0, self.routed.as_ref().and_then(|b| lock(&b.failed).take()).map_or(Ok(()), Err)),
            Ok(cost) => {
                let unknown = cost.unknown.len() as u64;
                let booked = cost.micro_usd.saturating_add(largest.saturating_mul(unknown));
                let what = if unknown == 0 { "step" } else { "step, cost unknown for a failed call: reservation booked" };
                (booked, what, cost.records.len() as u64 + unknown, Ok(()))
            }
            Err(reason) => {
                let all = held.iter().fold(0u64, |a, r| a.saturating_add(r.micro_usd));
                (all, "step, cost unknown: reservation booked", 0, Err(reason))
            }
        };
        self.commit(&held, what, booked)?;
        saturating_add(&self.meter.spent, booked);
        saturating_add(&self.meter.calls, calls);
        if calls > 0 {
            sink.record("runbook.cost-booked", json!({"calls": calls, "micro_usd": booked, "spent_micro_usd": self.meter.usage().spent_micro_usd}));
        }
        self.record_calls(sink);
        self.persist();
        outcome
    }

    fn commit(&self, held: &[Reservation], what: &str, booked: u64) -> Result<(), String> {
        let ledger = &self.held.ledger;
        let run = &self.held.run_id;
        match held.split_first() {
            Some((first, rest)) => {
                ledger.commit(first, run, what, booked)?;
                rest.iter().try_for_each(|r| ledger.release(r))
            }
            None => ledger.commit(&Reservation { id: String::new(), micro_usd: 0, left: u64::MAX }, run, what, booked),
        }
    }

    /// The run reached `cap`: the run's record says so, and the outcome is the cap's.
    pub fn reached(&self, cap: Cap, detail: &str, sink: &dyn EventSink) -> String {
        sink.record("runbook.cap-reached", json!({"cap": cap.name(), "detail": detail}));
        self.persist();
        cap.outcome().to_owned()
    }

    /// Writes the usage into the run's directory (best effort: the view falls back on nothing).
    pub fn persist(&self) {
        let _ = write_usage(&self.run_dir, self.meter.usage());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reservation_a_step_held_is_released_when_the_step_is_booked() {
        let dir = std::env::current_exe().unwrap().parent().unwrap().join(format!("gate-unit-release-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let caps = Caps { max_wall_seconds: 60, max_llm_calls: 10, cap_micro_usd: 1000 };
        let gate = Gate::open(caps, &dir, "rb", "run-a", &dir, &dir, false, Arc::new(Meter::on(Usage::default(), WallClock::default())), budget::Reserves::default()).unwrap();
        let ledger = &gate.held.ledger;
        let first = ledger.reserve_for_run("run-a", 1000, 300).unwrap();
        let second = ledger.reserve_for_run("run-a", 1000, 200).unwrap();
        gate.commit(&[first, second], "step", 100).unwrap();
        // 100 spent, nothing held: 900 of the run's 1000 are left
        assert_eq!(ledger.reserve_for_run("run-a", 1000, 1000).unwrap().micro_usd, 900, "a reservation of the step is still held after it was booked");
    }

    struct Quiet;

    impl EventSink for Quiet {
        fn steps(&self) -> Option<Arc<dyn crate::runtime::StartTarget>> {
            None
        }
        fn started(&self) {}
        fn record(&self, _: &str, _: Value) {}
        fn decision_requested(&self, _: crate::runtime::DecisionRequest) {}
        fn decision_answered(&self, _: &str, _: &str, _: &str, _: &str) -> bool {
            false
        }
        fn resumed(&self) {}
    }

    #[test]
    fn a_relayed_call_the_ledger_could_not_book_fails_the_step_it_ran_in() {
        use crate::relay::Booking;
        let dir = std::env::current_exe().unwrap().parent().unwrap().join(format!("gate-unit-relay-book-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let caps = Caps { max_wall_seconds: 60, max_llm_calls: 10, cap_micro_usd: 1000 };
        let mut gate = Gate::open(caps, &dir, "rb", "run-a", &dir, &dir, false, Arc::new(Meter::on(Usage::default(), WallClock::default())), budget::Reserves::default()).unwrap();
        let booking = gate.route(Arc::new(Mutex::new(None)));
        let file = dir.join("ledgers").join("rb.json");
        let kept = std::fs::read(&file).unwrap_or_default();
        let _ = std::fs::remove_file(&file);
        std::fs::create_dir(&file).unwrap();
        assert!(booking.admit().is_err(), "a call was admitted while the ledger could not be read");
        assert!(booking.book(json!({"what": "model call"}), 5).is_err(), "a ledger that cannot be written booked the call");
        std::fs::remove_dir(&file).unwrap();
        std::fs::write(&file, kept).unwrap();
        let outcome = gate.book(&Quiet);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(outcome.is_err_and(|why| why.contains("spend ledger")), "the step of a call the ledger did not book ended without its error");
    }

    fn routed_gate(tag: &str, caps: Caps) -> (Gate, std::path::PathBuf) {
        let dir = std::env::current_exe().unwrap().parent().unwrap().join(format!("gate-unit-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let gate = Gate::open(caps, &dir, "rb", "run-a", &dir, &dir, false, Arc::new(Meter::on(Usage::default(), WallClock::default())), budget::Reserves::default()).unwrap();
        (gate, dir)
    }

    #[test]
    fn a_relayed_call_the_gateway_refused_fails_the_step_it_ran_in_route_unavailable_naming_the_route_the_code_and_the_setting() {
        use crate::relay::Booking;
        let (mut gate, dir) = routed_gate("relay-refused", Caps { max_wall_seconds: 60, max_llm_calls: 10, cap_micro_usd: 1000 });
        let booking = gate.route(Arc::new(Mutex::new(Some("notes.write".to_owned()))));
        booking.admit().unwrap();
        // the line the relay books for a call its gateway refused 503 model-stopped (`relay::Call::line`)
        let line = json!({"what": "model call", "run": "run-a", "step": "notes.write", "role": "writer", "route": "qwen", "alias": "qwen-alias", "source": "protocol", "wire": "messages", "gateway_call": null, "input_tokens": null, "output_tokens": null, "cost_known": true, "micro_usd": 0, "cause": "route-unavailable", "code": "model-stopped", "detail": "the gateway answered 503"});
        booking.book(line, 0).unwrap();
        let outcome = gate.book(&Quiet);
        let _ = std::fs::remove_dir_all(&dir);
        let why = outcome.expect_err("the step whose model call the gateway refused was booked Ok");
        for word in ["route-unavailable", "qwen", "model-stopped", "protocol"] {
            assert!(why.contains(word), "the step's failure does not name {word}: {why}");
        }
    }

    #[test]
    fn decider_calls_of_unknown_cost_reach_the_spend_cap_when_the_runbooks_programs_reserve_nothing() {
        use crate::relay::Booking;
        // a runbook whose bound programs call no model (`max_budget_usd: 0` on each, as `budget::reserves` asks of
        // such a program): its largest budget is 0. The decider of kind `model` calls through the relay between
        // steps, and its route declares no price.
        let (mut gate, dir) = routed_gate("relay-unknown", Caps { max_wall_seconds: 60, max_llm_calls: 100, cap_micro_usd: 1000 });
        let booking = gate.route(Arc::new(Mutex::new(None)));
        let mut admitted = 0;
        for _ in 0..5 {
            if booking.admit().is_err() {
                break;
            }
            admitted += 1;
            // what `relay::Call::line` books for a call of unknown cost
            let reservation = booking.reservation_micro_usd();
            booking.book(json!({"what": "model call", "cost_known": false, "micro_usd": reservation}), reservation).unwrap();
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert!(admitted < 5, "5 model calls of unknown cost were admitted against a spend cap of 1000 micro USD: each booked {} (docs/model-routes.md, section 5: a call of unknown cost books its reservation, max_call_micro_usd)", booking.reservation_micro_usd());
    }

    #[test]
    fn the_spend_the_meter_shows_does_not_wrap_past_the_largest_amount() {
        use crate::relay::Booking;
        let (mut gate, dir) = routed_gate("relay-wrap", Caps { max_wall_seconds: 60, max_llm_calls: 10, cap_micro_usd: u64::MAX });
        let booking = gate.route(Arc::new(Mutex::new(None)));
        // `relay::cost_of` saturates a huge count of tokens at u64::MAX; the ledger saturates its sum
        booking.book(json!({"what": "model call"}), u64::MAX).unwrap();
        booking.book(json!({"what": "model call"}), 2).unwrap();
        let shown = gate.meter.usage().spent_micro_usd;
        let ledger = gate.held.ledger.spent_by_run("run-a").unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!((shown, ledger), (u64::MAX, u64::MAX), "the run's shown spend wrapped while the ledger saturated");
    }

    #[derive(Default)]
    struct Recorded(Mutex<Vec<(String, Value)>>);

    impl EventSink for Recorded {
        fn steps(&self) -> Option<Arc<dyn crate::runtime::StartTarget>> {
            None
        }
        fn started(&self) {}
        fn record(&self, kind: &str, record: Value) {
            self.0.lock().unwrap().push((kind.to_owned(), record));
        }
        fn decision_requested(&self, _: crate::runtime::DecisionRequest) {}
        fn decision_answered(&self, _: &str, _: &str, _: &str, _: &str) -> bool {
            false
        }
        fn resumed(&self) {}
    }

    #[test]
    fn a_call_still_in_flight_when_the_run_ends_is_said_in_the_runs_record_and_still_booked_in_the_ledger() {
        use crate::relay::Booking;
        let (mut gate, dir) = routed_gate("relay-late", Caps { max_wall_seconds: 60, max_llm_calls: 10, cap_micro_usd: 1000 });
        let booking = gate.route(Arc::new(Mutex::new(None)));
        booking.admit().unwrap();
        booking.book(json!({"what": "model call", "role": "early"}), 3).unwrap();
        booking.admit().unwrap();
        let sink = Recorded::default();
        gate.close_route(&sink, std::time::Duration::from_millis(50));
        booking.book(json!({"what": "model call", "role": "late"}), 4).unwrap();
        gate.record_calls(&sink);
        let spent = gate.held.ledger.spent_by_run("run-a").unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let kinds: Vec<(String, Value)> = sink.0.lock().unwrap().clone();
        assert_eq!(kinds.iter().filter(|(k, _)| k == "runbook.model-call").count(), 1, "{kinds:?}");
        let unfinished = kinds.iter().find(|(k, _)| k == "runbook.model-calls-unfinished").map(|(_, v)| v.clone());
        assert_eq!(unfinished.and_then(|v| v.get("in_flight").cloned()), Some(json!(1)), "the call in flight at the end is not in the run's record: {kinds:?}");
        assert_eq!(spent, 7, "the call that ended after the run is not in the ledger");
    }
}
