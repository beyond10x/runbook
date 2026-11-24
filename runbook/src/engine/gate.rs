//! What keeps a daemon run inside its caps (`crate::caps`): before each step the wall time and the
//! model calls used are held against their caps and the spend ledger of the runbook reserves what the
//! step may cost; after it, the cost records the bound programs wrote are booked. The ledger, the
//! cost reader and the call-budget reader are the ones `runbook run` uses.
//!
//! A step reserves the largest per-call budget (`--max-budget-usd`) of the bindings of the runbook,
//! never the whole cap, so two runs of one runbook do not stop each other while each stays under
//! the cap. Bindings with no per-call budget reserve nothing; what their steps cost is booked all the
//! same, from the cost records, and the next step is refused once the cap is reached.

use crate::budget;
use crate::caps::{Caps, Usage};
use crate::cost::CostReader;
use crate::ledger::{Ledger, Reservation};
use crate::runtime::EventSink;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
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

/// What to do before a step.
pub enum Admit {
    /// Run the step; the reservation is booked after it.
    Go(Reservation),
    /// The step is over a cap: the run stops here.
    Stop(Cap, String),
}

pub struct Gate {
    pub caps: Caps,
    pub meter: Arc<Meter>,
    run_id: String,
    run_dir: PathBuf,
    workspace: PathBuf,
    ledger: Ledger,
    want: u64,
    reader: CostReader,
}

impl Gate {
    /// The gate of a run: its ledger is `<state>/ledgers/<runbook>.json`, one per runbook. `resumed`: the
    /// run has a store from an earlier process, whose usage is read and whose cost records are
    /// not booked again.
    #[allow(clippy::too_many_arguments)]
    pub fn open(caps: Caps, state_dir: &Path, runbook: &str, run_id: &str, run_dir: &Path, workspace: &Path, bindings_dir: &Path, resumed: bool, meter: Arc<Meter>) -> Result<Gate, String> {
        let dir = state_dir.join("ledgers");
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let ledger = Ledger::open(&dir.join(format!("{runbook}.json")))?;
        let want = budget::largest_call_budget(bindings_dir)?.unwrap_or(0);
        let mut reader = CostReader::with_home(std::env::var("HOME").ok());
        if resumed {
            // what the earlier process wrote was booked then (or is lost with it): it is not booked again
            let _ = reader.read_new(workspace);
        }
        Ok(Gate { caps, meter, run_id: run_id.to_owned(), run_dir: run_dir.to_path_buf(), workspace: workspace.to_path_buf(), ledger, want, reader })
    }

    /// Whether the run's wall time is used up.
    pub fn wall_over(&self) -> bool {
        self.meter.usage().wall_seconds >= self.caps.max_wall_seconds
    }

    /// The caps held against the use before the next step: the model calls, then the spend (the cap
    /// reached, or the reservation over what is left).
    pub fn admit(&self) -> Result<Admit, String> {
        let used = self.meter.usage();
        if used.llm_calls >= self.caps.max_llm_calls {
            return Ok(Admit::Stop(Cap::Calls, format!("{} of {} calls", used.llm_calls, self.caps.max_llm_calls)));
        }
        let cap = self.caps.cap_micro_usd;
        let spent = self.ledger.spent()?;
        if spent >= cap {
            return Ok(Admit::Stop(Cap::Spend, format!("{spent} of {cap} micro USD spent")));
        }
        let reservation = self.ledger.reserve(&self.run_id, cap, self.want)?;
        if reservation.micro_usd < self.want {
            self.ledger.release(&reservation)?;
            return Ok(Admit::Stop(Cap::Spend, format!("{spent} of {cap} micro USD spent, and the next step may cost {}", self.want)));
        }
        Ok(Admit::Go(reservation))
    }

    /// Books what the step spent: the cost records written since the last step, at their cost, and a
    /// call that failed with no cost at its reservation. A record that cannot be read books the whole
    /// reservation and is an error (fail closed).
    pub fn book(&mut self, reservation: &Reservation, sink: &dyn EventSink) -> Result<(), String> {
        match self.reader.read_new(&self.workspace) {
            Ok(cost) => {
                let unknown = cost.unknown.len() as u64;
                let booked = cost.micro_usd.saturating_add(reservation.micro_usd.saturating_mul(unknown));
                let what = if unknown == 0 { "step" } else { "step, cost unknown for a failed call: reservation booked" };
                self.ledger.commit(reservation, &self.run_id, what, booked)?;
                let calls = cost.records.len() as u64 + unknown;
                self.meter.spent.fetch_add(booked, Ordering::SeqCst);
                self.meter.calls.fetch_add(calls, Ordering::SeqCst);
                if calls > 0 {
                    sink.record("runbook.cost-booked", json!({"calls": calls, "micro_usd": booked, "spent_micro_usd": self.meter.usage().spent_micro_usd}));
                }
                self.persist();
                Ok(())
            }
            Err(reason) => {
                self.ledger.commit(reservation, &self.run_id, "step, cost unknown: reservation booked", reservation.micro_usd)?;
                self.meter.spent.fetch_add(reservation.micro_usd, Ordering::SeqCst);
                self.persist();
                Err(reason)
            }
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
