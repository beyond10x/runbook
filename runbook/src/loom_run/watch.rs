//! The `watch` port of `runbook run` (`--watch`): `sdc-watch run <run directory>` beside the run,
//! as the old launcher started it (`scripts/watch-run.sh`). It records every file change of the
//! run in `fs/changes.jsonl`, attributed to the `tree run` step in flight (`docs/fs-changes.md`).
//!
//! The runner starts the watcher as its child once the run directory exists and ends it when the
//! run ends, whatever the end: [`WatchChild`] ends it when it is dropped, and the watcher ends
//! itself when the runner is killed (`--parent`: a parent-death signal and a poll of its parent).

use super::ports::{Ports, Refusal};
use super::EXIT_REFUSED;
use std::fs::File;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// The helper binary.
const HELPER: &str = "sdc-watch";
/// What the watcher's start may take: it scans the run directory before it says it watches.
const START_WITHIN: Duration = Duration::from_secs(60);
/// What its end may take: it flushes the changes still held back.
const END_WITHIN: Duration = Duration::from_secs(30);
/// The record of the pid the runner started, in the watcher's output directory.
pub const PID_RECORD: &str = "fs/watcher.json";

/// The helper binary, found as the other helpers are (`runbook doctor`): `libexec/runbook` and the
/// directory of the executable, the checkout's `target/` directories, `PATH`.
fn locate() -> Option<PathBuf> {
    let lookup = crate::assets::Lookup::from_process();
    let assets = lookup.locate().ok();
    lookup.helper(HELPER, assets.as_ref())
}

/// `--watch` without the helper is refused before anything is written. The port itself is built:
/// this is the only refusal of it left, kept in the hook of the unbuilt ports (`ports.rs`).
pub fn watch_not_built(ports: &Ports) -> Result<(), Refusal> {
    if ports.watch && locate().is_none() {
        return Err((EXIT_REFUSED, format!("--watch needs the helper {HELPER}: build it with `cd watch && cargo build --release`, or put it on PATH (`runbook doctor` lists the helpers)")));
    }
    Ok(())
}

/// A running `sdc-watch` of one run. Ended by its process id when dropped.
pub struct WatchChild {
    child: Child,
}

/// Starts the watcher when the run asks for one: `Ok(None)` without `--watch`.
pub fn start_if(on: bool, run_dir: &Path) -> Result<Option<WatchChild>, String> {
    if !on {
        return Ok(None);
    }
    let binary = locate().ok_or_else(|| format!("--watch needs the helper {HELPER}, and it was not found"))?;
    WatchChild::start(&binary, run_dir).map(Some)
}

impl WatchChild {
    /// Starts `binary run <run_dir>` as a child of this process and returns once it watches (its
    /// pid file is there). The child has a process group of its own, so a Ctrl-C at the terminal
    /// reaches the runner first and the runner ends the watcher after the run's last file.
    pub fn start(binary: &Path, run_dir: &Path) -> Result<WatchChild, String> {
        let out = run_dir.join("fs");
        std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
        let log_path = out.join("watch.log");
        let log = File::create(&log_path).map_err(|e| format!("{}: {e}", log_path.display()))?;
        let mut child = Command::new(binary)
            .arg("run")
            .arg(run_dir)
            .args(["--kind", "loom", "--parent"])
            .arg(std::process::id().to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log)
            .process_group(0)
            .spawn()
            .map_err(|e| format!("{}: {e}", binary.display()))?;
        let record = serde_json::json!({"format": "runbook.watcher/1", "pid": child.id()});
        if let Err(e) = std::fs::write(run_dir.join(PID_RECORD), format!("{record}\n")) {
            Self::kill(&mut child);
            return Err(format!("{PID_RECORD}: {e}"));
        }
        let pid_file = out.join("sdc-watch.pid");
        let started = Instant::now();
        let failure = loop {
            if pid_file.is_file() {
                return Ok(WatchChild { child });
            }
            if let Ok(Some(status)) = child.try_wait() {
                let said = std::fs::read_to_string(&log_path).unwrap_or_default();
                break format!("{HELPER} ended at its start ({status}): {}", said.trim().lines().last().unwrap_or("no message"));
            }
            if started.elapsed() > START_WITHIN {
                break format!("{HELPER} did not say it watches within {} s", START_WITHIN.as_secs());
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        Self::kill(&mut child);
        Err(failure)
    }

    fn kill(child: &mut Child) {
        let _ = child.kill();
        let _ = child.wait();
    }

    /// SIGTERM to the pid, so the watcher flushes what it holds and exits; a watcher that does not
    /// end within the time is killed. Nothing is looked up by name.
    fn end(&mut self) {
        if matches!(self.child.try_wait(), Ok(Some(_))) {
            return;
        }
        // SAFETY: kill(2) with the pid of our own child, which has not been waited for.
        unsafe { libc::kill(self.child.id() as libc::pid_t, libc::SIGTERM) };
        let started = Instant::now();
        while started.elapsed() < END_WITHIN {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Self::kill(&mut self.child);
    }
}

impl Drop for WatchChild {
    fn drop(&mut self) {
        self.end();
    }
}
