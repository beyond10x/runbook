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

use crate::budget;
use crate::caps::{Caps, Usage};
use crate::cost::CostReader;
use crate::ledger::{Ledger, Reservation};
use crate::runtime::EventSink;
use b10x_loom_intake_slice::confinement::{Backend, ChildCommand, ConfinementError, ConfinementRefusal, TestExecution, TestRunner};
use b10x_loom_intake_slice::executor::TestCommand;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
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

/// What a run has used: shared by the run's thread and the view of the run.
pub struct Meter {
    wall_before: u64,
    started: Instant,
    calls: AtomicU64,
    spent: AtomicU64,
}

impl Meter {
    pub fn new(before: Usage) -> Meter {
        Meter { wall_before: before.wall_seconds, started: Instant::now(), calls: AtomicU64::new(before.llm_calls), spent: AtomicU64::new(before.spent_micro_usd) }
    }

    pub fn usage(&self) -> Usage {
        Usage { wall_seconds: self.wall_before + self.started.elapsed().as_secs(), llm_calls: self.calls.load(Ordering::SeqCst), spent_micro_usd: self.spent.load(Ordering::SeqCst) }
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

pub struct Gate {
    pub caps: Caps,
    pub meter: Arc<Meter>,
    held: Arc<Held>,
    run_dir: PathBuf,
    workspace: PathBuf,
    reader: CostReader,
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
        Ok(Gate { caps, meter, held, run_dir: run_dir.to_path_buf(), workspace: workspace.to_path_buf(), reader })
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
        let (booked, what, calls, outcome) = match result {
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
        self.meter.spent.fetch_add(booked, Ordering::SeqCst);
        self.meter.calls.fetch_add(calls, Ordering::SeqCst);
        if calls > 0 {
            sink.record("runbook.cost-booked", json!({"calls": calls, "micro_usd": booked, "spent_micro_usd": self.meter.usage().spent_micro_usd}));
        }
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
            None => ledger.commit(&Reservation { id: String::new(), micro_usd: 0 }, run, what, booked),
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
        let gate = Gate::open(caps, &dir, "rb", "run-a", &dir, &dir, false, Arc::new(Meter::new(Usage::default())), budget::Reserves::default()).unwrap();
        let ledger = &gate.held.ledger;
        let first = ledger.reserve_for_run("run-a", 1000, 300).unwrap();
        let second = ledger.reserve_for_run("run-a", 1000, 200).unwrap();
        gate.commit(&[first, second], "step", 100).unwrap();
        // 100 spent, nothing held: 900 of the run's 1000 are left
        assert_eq!(ledger.reserve_for_run("run-a", 1000, 1000).unwrap().micro_usd, 900, "a reservation of the step is still held after it was booked");
    }
}
