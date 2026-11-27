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
use super::route::StepInFlight;
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

/// The kind of the run event that names the failed calls of a step whose program went on and did not fail
/// ([`Gate::settle_failed`]).
pub const FAILED_EVENT: &str = "runbook.model-calls-failed";

/// Why a failed call booked after its step had ended did not end the run ([`Gate::record_calls`]).
const LATE_WHY: &str = "the call ended after its step had ended";
/// Why the record names a failed call of a step whose program ended with an exit its bindings took as a result.
const RESULT_WHY: &str = "its program's exit was taken as a result, and the protocol went on";

/// The cause a failed call of a step gives the run's failure: `<cause>: route <route> (chosen by <source>) failed the
/// call: <code>: <detail>` (`refused the call` for `call-refused`), read from the call's ledger line.
fn failed_call_cause(line: &Value) -> String {
    let text = |k: &str| line.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
    let cause = text("cause");
    let verb = if cause == crate::relay::CALL_REFUSED { "refused" } else { "failed" };
    format!("{cause}: route {} (chosen by {}) {verb} the call: {}: {}", text("route"), text("source"), text("code"), text("detail"))
}

/// The ledger lines of failed calls taken apart by their step, the steps in the order their first call was booked.
fn by_step(lines: &[Value]) -> Vec<(String, Vec<&Value>)> {
    let mut steps: Vec<(String, Vec<&Value>)> = Vec::new();
    for line in lines {
        let step = line.get("step").and_then(Value::as_str).unwrap_or("");
        match steps.iter_mut().find(|(s, _)| s == step) {
            Some((_, calls)) => calls.push(line),
            None => steps.push((step.to_owned(), vec![line])),
        }
    }
    steps
}

/// Records the event `runbook.model-calls-failed` that names the failed calls `calls` of the step `step`; `why` says
/// why they did not end the run.
fn record_failed(sink: &dyn EventSink, step: &str, calls: &[&Value], why: &str) {
    let named: Vec<Value> = calls
        .iter()
        .map(|l| json!({"step": l.get("step"), "route": l.get("route"), "gateway_call": l.get("gateway_call"), "cause": l.get("cause"), "code": l.get("code"), "detail": l.get("detail"), "error": l.get("error")}))
        .collect();
    let detail = format!("{} model call(s) of the step {step} failed at the gateway; {why}", named.len());
    sink.record(FAILED_EVENT, json!({"step": step, "calls": named, "detail": detail}));
}

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

    /// The cap of a run that ended with `outcome`, if that outcome is a cap's.
    pub fn of_outcome(outcome: &str) -> Option<Cap> {
        [Cap::Wall, Cap::Calls, Cap::Spend].into_iter().find(|c| c.outcome() == outcome)
    }

    /// The `case_state` of the view `cases` for a case that had not ended when the daemon ended its run at this cap.
    pub fn case_state(self) -> &'static str {
        match self {
            Cap::Wall => "WallCapReached",
            Cap::Calls => "CallCapReached",
            Cap::Spend => "SpendCapReached",
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
/// (the step or answer is then ended as a cancelled one, which the run reads as the spend cap). The agent node of a
/// delegation starts with the model calls the run has left ([`crate::delegation::CALLS_LEFT_VARIABLE`]: the call cap
/// less the calls used, at least 1; a run with none left starts no step, [`Gate::admit`]), which bounds its turns.
struct Reserving {
    inner: Arc<dyn TestRunner>,
    held: Arc<Held>,
    meter: Arc<Meter>,
    max_calls: u64,
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
        if crate::delegation::is_agent_node(child.program(), child.args()) {
            let left = self.max_calls.saturating_sub(self.meter.usage().llm_calls).max(1);
            let mut environment = child.environment().clone();
            environment.set.insert(crate::delegation::CALLS_LEFT_VARIABLE.to_owned(), left.to_string());
            return self.inner.run_child(&child.clone().with_environment(environment));
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
/// ([`crate::relay::Booking::reservation_micro_usd`], read as the call is admitted). The lines wait here until the run's thread records them as
/// events of the run.
pub struct RelayBooking {
    held: Arc<Held>,
    meter: Arc<Meter>,
    max_calls: u64,
    step: Arc<StepInFlight>,
    pending: Mutex<Vec<Value>>,
    /// What fails the step in flight: the first call the ledger could not book, or the first call of that very start of
    /// the step whose route the gateway refused or whose spend ceiling it reached (`crate::relay::stops_the_run`).
    failed: Mutex<Option<String>>,
    /// The lines of the other failed calls of a step (`call-refused`, `route-unavailable`) booked while that step was in
    /// flight, since [`Gate::take_failed`] took them: they fail no step on their own, and the run ends as their step ends.
    failed_calls: Mutex<Vec<Value>>,
    /// The lines of the failed calls booked after their step had ended (no step in flight, another one, or a later start
    /// of the same action), a refusal of the route among them: their step went on, so they never fail the run;
    /// [`Gate::record_calls`] names them under their own step.
    late_failed: Mutex<Vec<Value>>,
    /// What a call of the decider may cost (`max_call_micro_usd`); `u64::MAX` when it names no budget.
    max_call: AtomicU64,
    /// The calls taken and not yet booked.
    in_flight: AtomicU64,
    /// The calls of a step (their line names it) booked since the last step was booked ([`Gate::book`] takes the count).
    booked: AtomicU64,
    /// The calls taken while a step was in flight since the last step was booked ([`Gate::book`] takes the count): each
    /// is booked by the relay when it ends, the step's own end before it or not.
    taken: AtomicU64,
    /// The run ended: what is booked now is no event of the run any more.
    closed: AtomicBool,
    /// The first call of the run a route's daily cap refused (`crate::route_day`): the route, the key and the figure. The
    /// run ends at that cap ([`Gate::route_cap`]).
    route_capped: Mutex<Option<(String, String, String)>>,
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
                if self.step.name().is_some() {
                    saturating_add(&self.taken, 1);
                }
                Ok(())
            }
        }
    }

    /// In a step: the largest reservation the step holds. Otherwise (a call of the decider, or a step that holds
    /// nothing): what a call of the decider may cost, at most what the run's spend cap has left
    /// (`docs/model-routes.md`, section 5). The relay reads it as it admits a call, never when it books one.
    fn reservation_micro_usd(&self) -> u64 {
        let largest = lock(&self.held.reserved).iter().map(|r| r.micro_usd).max().unwrap_or(0);
        if largest > 0 && self.step.name().is_some() {
            return largest;
        }
        let left = self.held.cap_micro_usd.saturating_sub(self.held.ledger.spent_by_run(&self.held.run_id).unwrap_or(0));
        self.max_call.load(Ordering::SeqCst).min(left)
    }

    fn step(&self) -> Option<String> {
        self.step.name()
    }

    fn step_started(&self) -> (Option<String>, u64) {
        self.step.now()
    }

    /// Books a call as one of the step in flight now.
    fn book(&self, line: Value, micro_usd: u64) -> Result<(), String> {
        self.book_started(line, micro_usd, self.step.now().1)
    }

    fn book_started(&self, line: Value, micro_usd: u64, started: u64) -> Result<(), String> {
        // a call of a step booked after that step ended (its program exited with the call still open): the step in flight
        // now is another, or a later start of the same action
        let late = line.get("step").and_then(Value::as_str).is_some_and(|step| {
            let (name, now) = self.step.now();
            name.as_deref() != Some(step) || now != started
        });
        let booked = self.held.ledger.book_line(&self.held.run_id, &line, micro_usd);
        saturating_add(&self.meter.spent, micro_usd);
        // a call of a step (a decider's call between steps names none) counts toward the step's booked calls
        if line.get("step").is_some_and(Value::is_string) {
            saturating_add(&self.booked, 1);
        }
        let _ = self.in_flight.try_update(Ordering::SeqCst, Ordering::SeqCst, |n| Some(n.saturating_sub(1)));
        // a failed call of a step: one the gateway refused the route for (or its spend ceiling stopped) fails the step at
        // once; any other is kept, and the run ends as its step ends. One booked after its step ended fails no step: that
        // step went on, and the next call on a refused route meets the refusal in its own step.
        let failed_call = line.get("step").is_some_and(Value::is_string) && matches!(line.get("cause").and_then(Value::as_str), Some(crate::relay::ROUTE_UNAVAILABLE | crate::relay::CALL_REFUSED));
        let stops = failed_call && crate::relay::stops_the_run(line.get("code").and_then(Value::as_str).unwrap_or(""));
        let failure = match &booked {
            Err(why) => Some(why.clone()),
            Ok(()) if stops && !late => Some(failed_call_cause(&line)),
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
            // kept under the lock of the pending lines: the run's close takes both at once (`Gate::close_route`)
            // a call booked while its own step is in flight belongs to that step's end; one booked after its step ended
            // belongs to a step that is over and went on, whatever failed it
            if booked.is_ok() && failed_call && late {
                lock(&self.late_failed).push(line.clone());
            } else if booked.is_ok() && failed_call && !stops {
                lock(&self.failed_calls).push(line.clone());
            }
            pending.push(line);
        }
        drop(pending);
        booked
    }

    fn route_capped(&self, route: &str, key: &str, detail: &str) {
        lock(&self.route_capped).get_or_insert_with(|| (route.to_owned(), key.to_owned(), detail.to_owned()));
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
    /// The failed calls of a step whose program ended with an exit its bindings took as a result, and that step's case
    /// ([`Self::hold_failed`]): the run's next call into the tree tells whether the protocol went on.
    held_failed: Mutex<Option<(String, Vec<Value>)>>,
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
        Ok(Gate { caps, meter, held, run_dir: run_dir.to_path_buf(), workspace: workspace.to_path_buf(), reader, routed: None, held_failed: Mutex::new(None) })
    }

    /// The run is on a route: from now on its model calls are taken against its caps and booked by the relay through
    /// what this returns, and a cost record of a bound program carries no cost (`CostReader::routed`). `step` holds
    /// the step in flight.
    pub fn route(&mut self, step: Arc<StepInFlight>) -> Arc<RelayBooking> {
        let booking = Arc::new(RelayBooking { held: self.held.clone(), meter: self.meter.clone(), max_calls: self.caps.max_llm_calls, step, pending: Mutex::new(Vec::new()), failed: Mutex::new(None), failed_calls: Mutex::new(Vec::new()), late_failed: Mutex::new(Vec::new()), max_call: AtomicU64::new(u64::MAX), in_flight: AtomicU64::new(0), booked: AtomicU64::new(0), taken: AtomicU64::new(0), closed: AtomicBool::new(false), route_capped: Mutex::new(None) });
        self.reader = std::mem::take(&mut self.reader).routed();
        self.routed = Some(booking.clone());
        booking
    }

    /// The failed calls of a step that fail no step on their own (a `4xx` the client may recover from, a `5xx`, a
    /// connection that failed, an answer that broke off), booked since the last time, taken once: the run ends as their
    /// step ends ([`Self::settle_failed`]).
    pub fn take_failed(&self) -> Vec<Value> {
        self.routed.as_ref().map(|b| std::mem::take(&mut *lock(&b.failed_calls))).unwrap_or_default()
    }

    /// Holds the failed calls of a step of `case` whose program ended with an exit its bindings took as a result, until
    /// the run's next call into the tree tells whether the protocol went on ([`Self::take_held_failed`]); held calls
    /// the run's end finds are named as the calls of a step that went on ([`Self::close_route`]).
    pub fn hold_failed(&self, case: &str, failed_calls: Vec<Value>) {
        lock(&self.held_failed).get_or_insert_with(|| (case.to_owned(), Vec::new())).1.extend(failed_calls);
    }

    /// The calls [`Self::hold_failed`] holds and their case, taken once.
    pub fn take_held_failed(&self) -> Option<(String, Vec<Value>)> {
        lock(&self.held_failed).take()
    }

    /// The run's record names held calls (`Self::hold_failed`) whose step went on, one event `runbook.model-calls-failed`
    /// per step.
    pub fn note_held(held: &[Value], sink: &dyn EventSink) {
        for (step, calls) in by_step(held) {
            record_failed(sink, &step, &calls, RESULT_WHY);
        }
    }

    /// The run's end follows the step of each of the calls `failed_calls` ([`Self::take_failed`]), taken apart by their
    /// step (a call left in flight by an earlier step may be booked while a later one runs): the run's record names the
    /// failed calls of each step that went on with one event `runbook.model-calls-failed`; when a step `failed` (its
    /// program failed), its own first failed call is the run's first cause ([`failed_call_cause`]), never a call of
    /// another step.
    pub fn settle_failed(failed_calls: &[Value], failed: impl Fn(&str) -> bool, sink: &dyn EventSink) -> Result<(), String> {
        let mut cause = None;
        for (step, calls) in by_step(failed_calls) {
            if failed(&step) {
                cause.get_or_insert_with(|| failed_call_cause(calls[0]));
            } else {
                record_failed(sink, &step, &calls, "the step went on and its program did not fail");
            }
        }
        cause.map_or(Ok(()), Err)
    }

    /// The role names of the run's route file: the reason of a model call that left no cost record names them when the
    /// client did not know the model it asked for (`CostReader::with_route_roles`).
    pub fn name_route_roles(&mut self, roles: Vec<String>) {
        self.reader = std::mem::take(&mut self.reader).with_route_roles(roles);
    }

    /// Records the model calls the relay booked since the last time as events of the run (`runbook.model-call`), then
    /// names the failed ones among them that were booked after their step had ended, one event
    /// `runbook.model-calls-failed` per step: their step went on, and they never fail the run.
    pub fn record_calls(&self, sink: &dyn EventSink) {
        if let Some(booking) = &self.routed {
            let (lines, late) = {
                let mut pending = lock(&booking.pending);
                (std::mem::take(&mut *pending), std::mem::take(&mut *lock(&booking.late_failed)))
            };
            for line in lines {
                sink.record(crate::relay::MODEL_CALL_EVENT, line);
            }
            for (step, calls) in by_step(&late) {
                record_failed(sink, &step, &calls, LATE_WHY);
            }
        }
    }

    /// The run has ended and its token is void: waits up to `wait` for the calls still in flight (their callers
    /// were ended with the run, the gateway still answers them), records every booked call as an event of the run,
    /// and records `runbook.model-calls-unfinished` when calls are still in flight then. Such a call is booked in the
    /// ledger when it ends and said on the daemon's stderr. A failed call that ended after its step was booked is
    /// named in the run's record with one event `runbook.model-calls-failed` for its step.
    pub fn close_route(&self, sink: &dyn EventSink, wait: std::time::Duration) {
        let Some(booking) = &self.routed else { return };
        let until = Instant::now() + wait;
        while booking.in_flight() > 0 && Instant::now() < until {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let (lines, failed_calls, late) = {
            let mut pending = lock(&booking.pending);
            booking.closed.store(true, Ordering::SeqCst);
            (std::mem::take(&mut *pending), std::mem::take(&mut *lock(&booking.failed_calls)), std::mem::take(&mut *lock(&booking.late_failed)))
        };
        for line in lines {
            sink.record(crate::relay::MODEL_CALL_EVENT, line);
        }
        // a failed call that ended after its step was booked: the run's end is taken, and its record names the call
        for (step, calls) in by_step(&failed_calls) {
            record_failed(sink, &step, &calls, "the call ended after its step was booked");
        }
        for (step, calls) in by_step(&late) {
            record_failed(sink, &step, &calls, LATE_WHY);
        }
        if let Some((_, held)) = self.take_held_failed() {
            Self::note_held(&held, sink);
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
        Arc::new(Reserving { inner, held: self.held.clone(), meter: self.meter.clone(), max_calls: self.caps.max_llm_calls })
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
    /// record that cannot be read books the reservations and is an error (fail closed), unless the relay booked or took
    /// calls of the step: then the step costs the sum of those calls (a call still in flight is booked by the relay when
    /// it ends, once, at the reservation it was admitted with), its reservations are released, and its cost records (which
    /// carry no cost on a route) do not matter.
    pub fn book(&mut self, sink: &dyn EventSink) -> Result<(), String> {
        let held: Vec<Reservation> = std::mem::take(&mut *lock(&self.held.reserved));
        let largest = held.iter().map(|r| r.micro_usd).max().unwrap_or(0);
        let result = self.reader.read_new(&self.workspace);
        let routed = self.routed.is_some();
        // the calls of the step the relay booked or takes still (a call in flight when its program was killed is booked
        // by the relay when the gateway answers it, at the reservation it was admitted with)
        let relayed = self.routed.as_ref().map_or(0, |b| b.booked.swap(0, Ordering::SeqCst).saturating_add(b.taken.swap(0, Ordering::SeqCst)));
        let relay_failed = || self.routed.as_ref().and_then(|b| lock(&b.failed).take()).map_or(Ok(()), Err);
        let (booked, what, calls, outcome) = match result {
            // The relay took and booked every model call of the step; the step's reservations are released. A call
            // the ledger could not book fails the step (fail closed).
            Ok(_) if routed => (0, "step", 0, relay_failed()),
            Err(_) if relayed > 0 => (0, "step", 0, relay_failed()),
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

    /// A route's daily cap refused a call of the run (`crate::route_day`, taken once): the run ends as a run at a cap
    /// ends, at the cap of the same figure (`stopped: llm calls` for `daily_max_llm_calls`, `stopped: spend cap` for
    /// `daily_cap_micro_usd`), and its `runbook.cap-reached` names the route and the key. None when no call was refused so.
    pub fn route_cap(&self, sink: &dyn EventSink) -> Option<String> {
        let (route, key, detail) = lock(&self.routed.as_ref()?.route_capped).take()?;
        let cap = if key.ends_with(crate::route_day::DAILY_CALLS) { Cap::Calls } else { Cap::Spend };
        sink.record("runbook.cap-reached", json!({"cap": cap.name(), "detail": detail, "route": route, "cap_key": key}));
        self.persist();
        Some(cap.outcome().to_owned())
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
        let booking = gate.route(Arc::new(StepInFlight::default()));
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

    /// A call whose route the gateway refused (`wire-not-served`, `model-unknown`, `credential-*`) or whose spend ceiling
    /// it reached fails the step it ran in at once, `route-unavailable`, naming the route, the code and the setting.
    #[test]
    fn a_relayed_call_whose_route_the_gateway_refused_fails_the_step_it_ran_in_route_unavailable_naming_the_route_the_code_and_the_setting() {
        use crate::relay::Booking;
        for (code, status) in [("wire-not-served", 400), ("model-unknown", 404), ("credential-rejected", 401), ("spend-ceiling", 503)] {
            let (mut gate, dir) = routed_gate(&format!("relay-refused-{code}"), Caps { max_wall_seconds: 60, max_llm_calls: 10, cap_micro_usd: 1000 });
            let booking = gate.route(Arc::new(StepInFlight::at("notes.write")));
            booking.admit().unwrap();
            // the line the relay books for a call its gateway refused (`relay::Call::line`)
            let line = json!({"what": "model call", "run": "run-a", "step": "notes.write", "role": "writer", "route": "qwen", "alias": "qwen-alias", "source": "protocol", "wire": "messages", "gateway_call": null, "input_tokens": null, "output_tokens": null, "cost_known": true, "micro_usd": 0, "cause": "route-unavailable", "code": code, "detail": format!("the gateway answered {status}")});
            booking.book(line, 0).unwrap();
            let outcome = gate.book(&Quiet);
            let kept = gate.take_failed();
            let _ = std::fs::remove_dir_all(&dir);
            let why = outcome.expect_err("the step whose model call the gateway refused was booked Ok");
            for word in ["route-unavailable", "qwen", code, "protocol"] {
                assert!(why.contains(word), "the step's failure does not name {word}: {why}");
            }
            assert!(kept.is_empty(), "a call that fails the step at once was kept to follow the step: {kept:?}");
        }
    }

    /// A call admitted in one start of `notes.write` and booked while a later start of the same action runs ended after
    /// its own step: it is no call of the step in flight, and a refusal of the route among such calls fails no step. The
    /// record names both under their step; a refusal of the route in the start it was made in still fails that step.
    #[test]
    fn a_call_of_an_earlier_start_of_the_same_action_is_late_and_a_late_refusal_of_the_route_fails_no_step() {
        use crate::relay::Booking;
        let (mut gate, dir) = routed_gate("relay-late-start", Caps { max_wall_seconds: 60, max_llm_calls: 10, cap_micro_usd: 1000 });
        let step = Arc::new(StepInFlight::default());
        let booking = gate.route(step.clone());
        let line = |call: &str, cause: &str, code: &str| json!({"what": "model call", "run": "run-a", "step": "notes.write", "route": "default", "source": "run", "gateway_call": call, "cost_known": true, "micro_usd": 0, "cause": cause, "code": code, "detail": "the gateway answered"});
        step.start(Some("notes.write".to_owned()));
        let (_, first) = booking.step_started();
        for _ in 0..3 {
            booking.admit().unwrap();
        }
        step.end();
        step.start(Some("notes.write".to_owned()));
        booking.book_started(line("gw-earlier", "route-unavailable", "upstream-failed"), 0, first).unwrap();
        booking.book_started(line("gw-refused-late", "route-unavailable", "wire-not-served"), 0, first).unwrap();
        let kept = Kept(Mutex::new(Vec::new()));
        let late_outcome = gate.book(&kept);
        let own = gate.take_failed();
        gate.record_calls(&kept);
        let (_, now) = booking.step_started();
        booking.book_started(line("gw-refused-own", "route-unavailable", "wire-not-served"), 0, now).unwrap();
        let own_outcome = gate.book(&Quiet);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(late_outcome.is_ok(), "a call of an earlier start failed the start in flight: {late_outcome:?}");
        assert!(own.is_empty(), "a call of an earlier start was taken with the failed calls of the start in flight: {own:?}");
        let named: Vec<(Value, Vec<Value>)> = kept
            .0
            .into_inner()
            .unwrap()
            .into_iter()
            .filter(|(k, _)| k == FAILED_EVENT)
            .map(|(_, r)| (r["step"].clone(), r["calls"].as_array().unwrap().iter().map(|c| c["gateway_call"].clone()).collect()))
            .collect();
        assert_eq!(named, vec![(json!("notes.write"), vec![json!("gw-earlier"), json!("gw-refused-late")])]);
        assert!(own_outcome.is_err_and(|why| why.contains("wire-not-served")), "a refusal of the route in its own start did not fail that step");
    }

    struct Kept(Mutex<Vec<(String, Value)>>);

    impl EventSink for Kept {
        fn steps(&self) -> Option<Arc<dyn crate::runtime::StartTarget>> {
            None
        }
        fn started(&self) {}
        fn record(&self, kind: &str, record: Value) {
            lock(&self.0).push((kind.to_owned(), record));
        }
        fn decision_requested(&self, _: crate::runtime::DecisionRequest) {}
        fn decision_answered(&self, _: &str, _: &str, _: &str, _: &str) -> bool {
            false
        }
        fn resumed(&self) {}
    }

    /// A failed call of a step that is no refusal of its route (a `4xx` the client may recover from booked
    /// `call-refused`; a `5xx`, a connection that failed and an answer that broke off booked `route-unavailable`) fails
    /// no step when it is booked; it is taken once, and the run's end follows its step: a step that failed names the
    /// first such call as its cause, one that went on is noted in the run's record with each call's cause.
    #[test]
    fn a_failed_call_that_is_no_refusal_of_its_route_fails_no_step_and_the_run_follows_its_step() {
        use crate::relay::Booking;
        let (mut gate, dir) = routed_gate("relay-call-failed", Caps { max_wall_seconds: 60, max_llm_calls: 10, cap_micro_usd: 1000 });
        let booking = gate.route(Arc::new(StepInFlight::at("delegation.free")));
        let base = json!({"what": "model call", "run": "run-a", "step": "delegation.free", "role": "default", "route": "sonnet", "alias": "sonnet", "source": "run", "wire": "messages", "input_tokens": null, "output_tokens": null, "cost_known": true, "micro_usd": 0});
        let failures = [
            ("gw-c", "call-refused", "invalid_request_error", "the gateway answered 400"),
            ("gw-d", "route-unavailable", "upstream-failed", "the gateway answered 502"),
            ("gw-e", "route-unavailable", "model-stopped", "the gateway answered 503"),
            ("gw-f", "route-unavailable", "connection-failed", "no answer: TimedOut"),
            ("gw-g", "route-unavailable", "answer-broken", "the answer broke off: ConnectionReset"),
        ];
        for (call, cause, code, detail) in failures {
            booking.admit().unwrap();
            let mut line = base.clone();
            line["gateway_call"] = json!(call);
            line["cause"] = json!(cause);
            line["code"] = json!(code);
            line["detail"] = json!(detail);
            booking.book(line, 0).unwrap();
        }
        let outcome = gate.book(&Quiet);
        let failed = gate.take_failed();
        let again = gate.take_failed();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(outcome.is_ok(), "the step of a failed call that is no refusal of its route failed on the booking: {outcome:?}");
        assert_eq!((failed.len(), again.len()), (failures.len(), 0), "{failed:?} {again:?}");
        let why = Gate::settle_failed(&failed, |step| step == "delegation.free", &Quiet).expect_err("a failed step's failed call was not its cause");
        assert_eq!(why, "call-refused: route sonnet (chosen by run) refused the call: invalid_request_error: the gateway answered 400");
        let why = Gate::settle_failed(&failed[1..], |_| true, &Quiet).unwrap_err();
        assert_eq!(why, "route-unavailable: route sonnet (chosen by run) failed the call: upstream-failed: the gateway answered 502");
        let kept = Kept(Mutex::new(Vec::new()));
        Gate::settle_failed(&failed, |_| false, &kept).unwrap();
        let kept = kept.0.into_inner().unwrap();
        assert_eq!(kept.len(), 1, "{kept:?}");
        assert_eq!((kept[0].0.as_str(), kept[0].1["step"].clone()), (FAILED_EVENT, json!("delegation.free")));
        let named: Vec<(Value, Value)> = kept[0].1["calls"].as_array().unwrap().iter().map(|c| (c["gateway_call"].clone(), c["cause"].clone())).collect();
        let want: Vec<(Value, Value)> = failures.iter().map(|(call, cause, _, _)| (json!(call), json!(cause))).collect();
        assert_eq!(named, want);
        assert!(Gate::settle_failed(&[], |_| true, &Quiet).is_ok());
    }

    /// Failed calls taken together after a step are settled each by its own step: a late call of an earlier step that
    /// went on is named under that step, and the step that failed has its own first failed call as the run's cause.
    #[test]
    fn failed_calls_of_two_steps_are_settled_each_by_its_own_step() {
        let line = |step: &str, call: &str, cause: &str, code: &str| json!({"step": step, "route": "default", "source": "run", "gateway_call": call, "cause": cause, "code": code, "detail": "the gateway answered 4xx or 5xx"});
        let calls = [
            line("notes.write", "gw-late", "route-unavailable", "upstream-failed"),
            line("result.write", "gw-own", "call-refused", "invalid_request_error"),
            line("notes.write", "gw-late-2", "route-unavailable", "model-stopped"),
        ];
        let kept = Kept(Mutex::new(Vec::new()));
        let why = Gate::settle_failed(&calls, |step| step == "result.write", &kept).expect_err("the failed step's own call was not the run's cause");
        assert!(why.starts_with("call-refused: route default (chosen by run) refused the call: invalid_request_error"), "{why}");
        let kept = kept.0.into_inner().unwrap();
        let noted: Vec<(String, Value, Vec<Value>)> = kept.iter().map(|(k, r)| (k.clone(), r["step"].clone(), r["calls"].as_array().unwrap().iter().map(|c| c["gateway_call"].clone()).collect())).collect();
        assert_eq!(noted, vec![(FAILED_EVENT.to_owned(), json!("notes.write"), vec![json!("gw-late"), json!("gw-late-2")])]);
    }

    /// A failed call that ends after its step was booked (its client hung up, the relay finished it) is named in the
    /// run's record when the run's route is closed, under its own step.
    #[test]
    fn a_failed_call_booked_after_its_step_is_named_when_the_route_closes() {
        use crate::relay::Booking;
        let (mut gate, dir) = routed_gate("relay-late-close", Caps { max_wall_seconds: 60, max_llm_calls: 10, cap_micro_usd: 1000 });
        let booking = gate.route(Arc::new(StepInFlight::at("delegation.free")));
        booking.admit().unwrap();
        let outcome = gate.book(&Quiet);
        assert!(gate.take_failed().is_empty());
        let late = json!({"what": "model call", "run": "run-a", "step": "delegation.free", "route": "default", "source": "run", "gateway_call": "gw-late", "cost_known": true, "micro_usd": 0, "cause": "route-unavailable", "code": "upstream-failed", "detail": "the gateway answered 502"});
        booking.book(late, 0).unwrap();
        let kept = Kept(Mutex::new(Vec::new()));
        gate.close_route(&kept, std::time::Duration::from_millis(100));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(outcome.is_ok(), "{outcome:?}");
        let kept = kept.0.into_inner().unwrap();
        let kinds: Vec<&str> = kept.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(kinds, vec![crate::relay::MODEL_CALL_EVENT, FAILED_EVENT], "{kept:?}");
        assert_eq!((kept[1].1["step"].clone(), kept[1].1["calls"][0]["gateway_call"].clone()), (json!("delegation.free"), json!("gw-late")));
    }

    /// A failed call booked when its own step is no longer in flight (another step runs, or none) is never taken with the
    /// failed calls of the step that ends next, so it cannot be a run's cause: the next recording of the run's calls names
    /// it under its own step, once. A failed call of the step in flight is kept for that step's end as before.
    #[test]
    fn a_failed_call_booked_after_its_step_ended_is_named_under_its_step_and_never_taken_as_a_failed_call_of_the_next() {
        use crate::relay::Booking;
        let (mut gate, dir) = routed_gate("relay-late-step", Caps { max_wall_seconds: 60, max_llm_calls: 10, cap_micro_usd: 1000 });
        let step = Arc::new(StepInFlight::at("digest.write"));
        let booking = gate.route(step.clone());
        let line = |step: &str, call: &str| json!({"what": "model call", "run": "run-a", "step": step, "route": "default", "source": "run", "gateway_call": call, "cost_known": true, "micro_usd": 0, "cause": "route-unavailable", "code": "upstream-failed", "detail": "the gateway answered 502"});
        for _ in 0..3 {
            booking.admit().unwrap();
        }
        // the call of an earlier step, booked while another runs; then one of the step in flight
        booking.book(line("notes.write", "gw-late"), 0).unwrap();
        booking.book(line("digest.write", "gw-own"), 0).unwrap();
        // between two steps: the call of the step that just ended
        step.end();
        booking.book(line("proposals.write", "gw-between"), 0).unwrap();
        let kept = Kept(Mutex::new(Vec::new()));
        let outcome = gate.book(&kept);
        let own: Vec<Value> = gate.take_failed().iter().map(|l| l["gateway_call"].clone()).collect();
        gate.record_calls(&kept);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(outcome.is_ok(), "{outcome:?}");
        assert_eq!(own, vec![json!("gw-own")], "a call booked after its step ended was taken with the failed calls of the step that ended next");
        let named: Vec<(Value, Vec<Value>)> = kept
            .0
            .into_inner()
            .unwrap()
            .into_iter()
            .filter(|(k, _)| k == FAILED_EVENT)
            .map(|(_, r)| (r["step"].clone(), r["calls"].as_array().unwrap().iter().map(|c| c["gateway_call"].clone()).collect()))
            .collect();
        assert_eq!(named, vec![(json!("notes.write"), vec![json!("gw-late")]), (json!("proposals.write"), vec![json!("gw-between")])], "the late calls are not named once each under their own step");
    }

    #[test]
    fn a_step_whose_calls_the_relay_booked_costs_their_sum_and_its_reservation_is_released_even_when_its_cost_record_is_missing() {
        use crate::relay::Booking;
        let (mut gate, dir) = routed_gate("relay-release", Caps { max_wall_seconds: 60, max_llm_calls: 10, cap_micro_usd: 5_000_000 });
        let booking = gate.route(Arc::new(StepInFlight::at("proposals.write")));
        // the step's program reserved its budget, made one call the relay booked at 0 (a route of price 0), and was
        // killed before its agent node wrote the cost record of its call
        gate.held.reserve(1_000_000).unwrap();
        std::fs::create_dir_all(dir.join(".sdc/llm/001-proposals.write")).unwrap();
        std::fs::write(dir.join(".sdc/llm/001-proposals.write/request.json"), "{}").unwrap();
        booking.admit().unwrap();
        booking.book(json!({"what": "model call", "step": "proposals.write", "cost_known": true, "micro_usd": 0}), 0).unwrap();
        let outcome = gate.book(&Quiet);
        let (spent, shown) = (gate.held.ledger.spent_by_run("run-a").unwrap(), gate.meter.usage().spent_micro_usd);
        // a step with no booked call and a cost that cannot be read still books its reservation (fail closed)
        gate.held.reserve(1_000_000).unwrap();
        std::fs::create_dir_all(dir.join(".sdc/llm/002-notes.write")).unwrap();
        std::fs::write(dir.join(".sdc/llm/002-notes.write/request.json"), "{}").unwrap();
        let unbooked = gate.book(&Quiet);
        let after = gate.held.ledger.spent_by_run("run-a").unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(outcome.is_ok(), "the step whose call the relay booked failed on its cost record: {outcome:?}");
        assert_eq!((spent, shown), (0, 0), "the step booked its reservation beside the relay's line of its call");
        assert!(unbooked.is_err(), "a step with no booked call and no cost record was booked as costing nothing");
        assert_eq!(after, 1_000_000, "a step with no booked call and no cost record did not book its reservation");
    }

    #[test]
    fn decider_calls_of_unknown_cost_reach_the_spend_cap_when_the_runbooks_programs_reserve_nothing() {
        use crate::relay::Booking;
        // a runbook whose bound programs call no model (`max_budget_usd: 0` on each, as `budget::reserves` asks of
        // such a program): its largest budget is 0. The decider of kind `model` calls through the relay between
        // steps, and its route declares no price.
        let (mut gate, dir) = routed_gate("relay-unknown", Caps { max_wall_seconds: 60, max_llm_calls: 100, cap_micro_usd: 1000 });
        let booking = gate.route(Arc::new(StepInFlight::default()));
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
        let booking = gate.route(Arc::new(StepInFlight::default()));
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
        let booking = gate.route(Arc::new(StepInFlight::default()));
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
