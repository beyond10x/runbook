//! The `b10x-loom` child: an environment the runner builds itself, one call at a time, ended by
//! a signal or by the wall-time cap.

use std::path::Path;
use std::sync::atomic::AtomicBool;

/// What one call of `b10x-loom` left.
#[derive(Debug)]
pub struct ChildResult {
    /// The exit code; `None` when the child was ended by a signal of its own.
    pub code: Option<i32>,
    /// What it printed on standard output.
    pub stdout: Vec<u8>,
    /// What it printed on standard error.
    pub stderr: Vec<u8>,
    /// Why the runner ended the child early, if it did.
    pub ended_by: Option<EndedBy>,
}

/// Why the runner ended a running child.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndedBy {
    /// SIGTERM or SIGINT reached the runner.
    Signal,
    /// The wall-time deadline passed.
    WallTime,
}

/// The environment of every `b10x-loom` child, built by the runner: `PATH` (the runner's own),
/// `HOME` and `XDG_DATA_HOME` inside the run directory. Nothing else is passed.
pub fn child_env(_run_dir: &Path) -> Vec<(String, String)> {
    Vec::new()
}

/// Runs `loom args...` with exactly `env` (cleared first), working directory `cwd`; waits for it,
/// ending its process group when `cancel` is set (SIGTERM, then SIGKILL after 3 s) or when
/// `deadline_secs` (seconds since the Unix epoch on `now`) has passed.
pub fn run(
    _loom: &Path,
    _args: &[String],
    _env: &[(String, String)],
    _cwd: &Path,
    _cancel: &AtomicBool,
    _deadline_secs: u64,
    _now: &dyn Fn() -> u64,
) -> Result<ChildResult, String> {
    Err(crate::loom_run::not_built("child run"))
}
