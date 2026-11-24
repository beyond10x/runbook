//! The spend ledger: a JSON file shared by every run that counts against one cap, read and
//! written under an exclusive lock. Fails closed: an unreadable, locked or invalid ledger is an
//! error, never an empty one.
//!
//! Format `runbook.spend-ledger/1`:
//! `{"format", "spent_micro_usd", "reserved": {"<id>": micro_usd}, "entries": [{"run", "what", "micro_usd"}]}`.
//! A call reserves its most before it starts and commits its real cost after; the reservations of
//! all runs count against the cap, so two runs never both get the same remainder.

use serde_json::{json, Map, Value};
use std::fs::{File, OpenOptions, TryLockError};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The format id written into the ledger.
pub const FORMAT: &str = "runbook.spend-ledger/1";

/// The number of the next reservation of this process: with the process id it makes a reservation's
/// id one that no other reservation of the ledger has, however many were committed or released.
static NEXT_RESERVATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// How long a run waits for the lock before it fails closed.
const LOCK_WAIT: Duration = Duration::from_secs(10);

/// A handle on the ledger file.
#[derive(Debug, Clone)]
pub struct Ledger {
    path: PathBuf,
}

/// What a reservation grants: at most `micro_usd` may be spent by the call.
#[derive(Debug, PartialEq, Eq)]
pub struct Reservation {
    /// The id under which the amount is held in the ledger.
    pub id: String,
    /// The amount granted, micro USD.
    pub micro_usd: u64,
    /// What the cap had left when the amount was asked for, before this grant (`u64::MAX` when no
    /// cap is named): 0 means the cap is reached.
    pub left: u64,
}

struct State {
    spent: u64,
    reserved: Map<String, Value>,
    entries: Vec<Value>,
}

impl State {
    fn parse(text: &str) -> Result<State, String> {
        if text.trim().is_empty() {
            return Ok(State { spent: 0, reserved: Map::new(), entries: Vec::new() });
        }
        let v: Value = serde_json::from_str(text).map_err(|e| format!("not valid JSON: {e}"))?;
        let o = v.as_object().ok_or("not a JSON object")?;
        if o.get("format").and_then(Value::as_str) != Some(FORMAT) {
            return Err(format!("format is not {FORMAT}"));
        }
        let spent = o.get("spent_micro_usd").and_then(Value::as_u64).ok_or("spent_micro_usd is not a number")?;
        let reserved = o.get("reserved").and_then(Value::as_object).ok_or("reserved is not an object")?.clone();
        if reserved.values().any(|v| v.as_u64().is_none()) {
            return Err("a reservation is not a number".to_owned());
        }
        let entries = o.get("entries").and_then(Value::as_array).ok_or("entries is not an array")?.clone();
        Ok(State { spent, reserved, entries })
    }

    fn reserved_total(&self) -> u64 {
        self.reserved.values().filter_map(Value::as_u64).fold(0u64, |a, b| a.saturating_add(b))
    }

    /// What the entries of `run` add up to.
    fn spent_by(&self, run: &str) -> u64 {
        self.entries.iter().filter(|e| e.get("run").and_then(Value::as_str) == Some(run)).filter_map(|e| e.get("micro_usd").and_then(Value::as_u64)).fold(0u64, |a, b| a.saturating_add(b))
    }

    /// What `run` holds: the reservations whose id starts with the run's id and a dash (`grant` names
    /// them so). The ids of the daemon's runs are minted and of one length, so none starts with another.
    fn reserved_by(&self, run: &str) -> u64 {
        let prefix = format!("{run}-");
        self.reserved.iter().filter(|(id, _)| id.starts_with(&prefix)).filter_map(|(_, v)| v.as_u64()).fold(0u64, |a, b| a.saturating_add(b))
    }

    fn render(&self) -> String {
        let v = json!({
            "format": FORMAT,
            "spent_micro_usd": self.spent,
            "reserved": Value::Object(self.reserved.clone()),
            "entries": self.entries,
        });
        serde_json::to_string_pretty(&v).unwrap() + "\n"
    }
}

impl Ledger {
    /// Opens the ledger at `path`; creates it empty when absent; errors when it exists and is
    /// not a valid ledger.
    pub fn open(path: &Path) -> Result<Ledger, String> {
        let ledger = Ledger { path: path.to_path_buf() };
        ledger.with_state(|_| Ok(()))?;
        Ok(ledger)
    }

    fn with_state<T>(&self, f: impl FnOnce(&mut State) -> Result<T, String>) -> Result<T, String> {
        let name = self.path.display().to_string();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.path)
            .map_err(|e| format!("spend ledger {name}: {e}"))?;
        lock(&file).map_err(|e| format!("spend ledger {name}: {e}"))?;
        let mut text = String::new();
        file.read_to_string(&mut text).map_err(|e| format!("spend ledger {name}: unreadable: {e}"))?;
        let mut state = State::parse(&text).map_err(|e| format!("spend ledger {name}: {e}"))?;
        let value = f(&mut state)?;
        let out = state.render();
        if out != text {
            file.set_len(0).map_err(|e| format!("spend ledger {name}: {e}"))?;
            file.seek(SeekFrom::Start(0)).map_err(|e| format!("spend ledger {name}: {e}"))?;
            file.write_all(out.as_bytes()).map_err(|e| format!("spend ledger {name}: {e}"))?;
            file.sync_all().map_err(|e| format!("spend ledger {name}: {e}"))?;
        }
        Ok(value)
    }

    /// Under the lock: reserves `min(want_micro_usd, cap - spent - reserved)` for run `run` and
    /// returns it; a grant of 0 means the cap is reached. The cap is shared by every run of the
    /// ledger: what all runs spent and hold counts (`runbook run`).
    pub fn reserve(&self, run: &str, cap_micro_usd: u64, want_micro_usd: u64) -> Result<Reservation, String> {
        self.grant(run, want_micro_usd, |s| cap_micro_usd.saturating_sub(s.spent).saturating_sub(s.reserved_total()))
    }

    /// Under the lock: reserves `min(want_micro_usd, cap - spent - reserved)` for run `run`, where
    /// `spent` and `reserved` are the run's own (its entries and its reservations): the cap is the
    /// run's, and what other runs of the ledger spent or hold never reduces it (the daemon).
    pub fn reserve_for_run(&self, run: &str, cap_micro_usd: u64, want_micro_usd: u64) -> Result<Reservation, String> {
        self.grant(run, want_micro_usd, |s| cap_micro_usd.saturating_sub(s.spent_by(run)).saturating_sub(s.reserved_by(run)))
    }

    fn grant(&self, run: &str, want_micro_usd: u64, remaining: impl FnOnce(&State) -> u64) -> Result<Reservation, String> {
        self.with_state(|s| {
            let left = remaining(s);
            let grant = want_micro_usd.min(left);
            let id = format!("{run}-{}-{}", std::process::id(), NEXT_RESERVATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
            if grant > 0 {
                s.reserved.insert(id.clone(), Value::from(grant));
            }
            Ok(Reservation { id, micro_usd: grant, left })
        })
    }

    /// Under the lock: drops every reservation `run` holds (a run that is not alive holds nothing).
    pub fn release_run(&self, run: &str) -> Result<(), String> {
        let prefix = format!("{run}-");
        self.with_state(|s| {
            s.reserved.retain(|id, _| !id.starts_with(&prefix));
            Ok(())
        })
    }

    /// Under the lock: what the entries of `run` add up to, micro USD.
    pub fn spent_by_run(&self, run: &str) -> Result<u64, String> {
        self.with_state(|s| Ok(s.spent_by(run)))
    }

    /// Under the lock: releases `reservation` and adds `actual_micro_usd` as spent (an entry
    /// naming the run and `what`).
    pub fn commit(&self, reservation: &Reservation, run: &str, what: &str, actual_micro_usd: u64) -> Result<(), String> {
        self.with_state(|s| {
            s.reserved.remove(&reservation.id);
            s.spent = s.spent.saturating_add(actual_micro_usd);
            if actual_micro_usd > 0 {
                s.entries.push(json!({"run": run, "what": what, "micro_usd": actual_micro_usd}));
            }
            Ok(())
        })
    }

    /// Under the lock: what the cap has left, micro USD (`cap - spent - reserved` over all runs of
    /// the ledger).
    pub fn left(&self, cap_micro_usd: u64) -> Result<u64, String> {
        self.with_state(|s| Ok(cap_micro_usd.saturating_sub(s.spent).saturating_sub(s.reserved_total())))
    }

    /// Under the lock: releases `reservation` without spending.
    pub fn release(&self, reservation: &Reservation) -> Result<(), String> {
        self.with_state(|s| {
            s.reserved.remove(&reservation.id);
            Ok(())
        })
    }

    /// Under the lock: the total spent, micro USD.
    pub fn spent(&self) -> Result<u64, String> {
        self.with_state(|s| Ok(s.spent))
    }
}

fn lock(file: &File) -> Result<(), String> {
    let start = Instant::now();
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(()),
            Err(TryLockError::WouldBlock) => {
                if start.elapsed() > LOCK_WAIT {
                    return Err(format!("locked for more than {} s", LOCK_WAIT.as_secs()));
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(TryLockError::Error(e)) => return Err(format!("cannot lock: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ledger(tag: &str) -> Ledger {
        let d = std::env::current_exe().unwrap().parent().unwrap().join(format!("ledger-unit-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Ledger::open(&d.join("l.json")).unwrap()
    }

    #[test]
    fn a_runs_cap_is_held_against_its_own_spend_and_reservations_only() {
        let l = ledger("own");
        let a = l.reserve_for_run("run-a", 1000, 400).unwrap();
        l.commit(&a, "run-a", "step", 400).unwrap();
        let held = l.reserve_for_run("run-a", 1000, 500).unwrap();
        // run-b is not reduced by the 400 spent and the 500 held by run-a
        let b = l.reserve_for_run("run-b", 1000, 1000).unwrap();
        assert_eq!((held.micro_usd, b.micro_usd), (500, 1000));
        // run-a has 100 left of its own
        assert_eq!(l.reserve_for_run("run-a", 1000, 300).unwrap().micro_usd, 100);
        assert_eq!((l.spent_by_run("run-a").unwrap(), l.spent_by_run("run-b").unwrap()), (400, 0));
        // the shared reserve of `runbook run` counts every run
        assert_eq!(l.reserve("run-c", 1000, 1000).unwrap().micro_usd, 0);
    }

    #[test]
    fn a_runs_reservations_are_dropped_by_its_id_and_no_other_runs() {
        let l = ledger("release");
        l.reserve_for_run("run-a", 1000, 300).unwrap();
        l.reserve_for_run("run-ab", 1000, 200).unwrap();
        l.release_run("run-a").unwrap();
        assert_eq!(l.reserve_for_run("run-a", 1000, 1000).unwrap().micro_usd, 1000, "its own reservation is gone");
        assert_eq!(l.reserve_for_run("run-ab", 1000, 1000).unwrap().micro_usd, 800, "the run whose id starts alike keeps its own");
    }
}
