//! The spend ledger: a JSON file shared by every run that counts against one cap, read and
//! written under an exclusive lock. Fails closed: an unreadable, locked or invalid ledger is an
//! error, never an empty one.

use std::path::{Path, PathBuf};

/// The format id written into the ledger.
pub const FORMAT: &str = "runbook.spend-ledger/1";

/// A handle on the ledger file.
#[derive(Debug)]
pub struct Ledger {
    #[allow(dead_code)]
    path: PathBuf,
}

/// What a reservation grants: at most `micro_usd` may be spent by the call.
#[derive(Debug, PartialEq, Eq)]
pub struct Reservation {
    /// The id under which the amount is held in the ledger.
    pub id: String,
    /// The amount granted, micro USD.
    pub micro_usd: u64,
}

impl Ledger {
    /// Opens the ledger at `path`; creates it empty when absent; errors when it exists and is
    /// not a valid ledger.
    pub fn open(path: &Path) -> Result<Ledger, String> {
        let _ = path;
        Err(crate::loom_run::not_built("ledger open"))
    }

    /// Under the lock: reserves `min(want_micro_usd, cap - spent - reserved)` for run `run` and
    /// returns it; a grant of 0 means the cap is reached.
    pub fn reserve(&self, _run: &str, _cap_micro_usd: u64, _want_micro_usd: u64) -> Result<Reservation, String> {
        Err(crate::loom_run::not_built("ledger reserve"))
    }

    /// Under the lock: releases `reservation` and adds `actual_micro_usd` as spent (an entry
    /// naming the run and `what`).
    pub fn commit(&self, _reservation: &Reservation, _run: &str, _what: &str, _actual_micro_usd: u64) -> Result<(), String> {
        Err(crate::loom_run::not_built("ledger commit"))
    }

    /// Under the lock: releases `reservation` without spending.
    pub fn release(&self, _reservation: &Reservation) -> Result<(), String> {
        Err(crate::loom_run::not_built("ledger release"))
    }

    /// Under the lock: the total spent, micro USD.
    pub fn spent(&self) -> Result<u64, String> {
        Err(crate::loom_run::not_built("ledger spent"))
    }
}
