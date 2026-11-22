//! The loop of `runbook loom-run`.

use crate::loom_run::{Args, Outcome};

/// The clock the loop reads: seconds since the Unix epoch. Tests give their own.
pub trait Clock {
    /// Now, in seconds.
    fn now_secs(&self) -> u64;
}

/// The system clock.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_secs(&self) -> u64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
    }
}

/// Runs the whole command. `Err` is a refusal before anything was written (exit 2); `Ok` is how
/// the run ended, written into the outcome file.
pub fn run(_args: &Args) -> Result<Outcome, String> {
    Err(crate::loom_run::not_built("loom-run loop"))
}
