//! What a run records so that a reader can tell what is happening now: the runner's own record
//! (`runner.json`: its process and the start time the kernel gave it) and the event of an action
//! that starts (`action_started` in `events.jsonl`).
//!
//! The record names the process so that a reader can tell a runner that is gone from one that is
//! alive: a process id is reused, the start time of the process is not. It holds no environment, no
//! argv and no path. It is closed (`closed_at` set) when the run ends, on every way out of
//! `run_with` (the `Drop` of [`RunnerRecord`] covers an early return, a panic and the signals the
//! runner handles, which end the run through its normal return). A runner ended by SIGKILL leaves
//! it open, which is what a reader reads as "the runner is gone".
//!
//! The library gives one point before a bound program starts: the `run_child` of the runner every
//! program starts through. The wrapper that uses it, `runbook::engine::announce::Announcing`, is
//! the daemon's too; [`announcing`] gives it the target that writes the event into the run
//! directory, before the program starts. See `docs/loom-run-directory.md`.

use crate::loom_run::events::{Event, StartSource};
use crate::loom_run::record_write::{append, write_atomic};
use crate::loom_run::run_files::event_json;
use b10x_loom_intake_slice::confinement::TestRunner;
use runbook::engine::announce::Announcing;
use runbook::runtime::{StartTarget, Started};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The file of the runner's record, in the run directory.
pub const RUNNER_FILE: &str = "runner.json";

fn system_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// The start time of this process as the kernel gives it: field 22 of `/proc/self/stat`, in clock
/// ticks since boot.
fn own_start_ticks() -> Result<u64, String> {
    let stat = std::fs::read_to_string("/proc/self/stat").map_err(|e| format!("/proc/self/stat: {e}"))?;
    let after = stat.rfind(')').and_then(|i| stat.get(i + 2..)).ok_or("/proc/self/stat: no process name")?;
    after.split_whitespace().nth(19).and_then(|t| t.parse().ok()).ok_or_else(|| "/proc/self/stat: no start time".to_owned())
}

/// The record of the runner in the run directory, open while the run lives.
pub struct RunnerRecord {
    path: PathBuf,
    pid: u32,
    start_ticks: u64,
    started_at: u64,
    closed: bool,
}

impl RunnerRecord {
    /// Writes `runner.json` into `run_dir`: the process id, its start time and `started_at`.
    pub fn create(run_dir: &Path, started_at: u64) -> Result<RunnerRecord, String> {
        let record = RunnerRecord { path: run_dir.join(RUNNER_FILE), pid: std::process::id(), start_ticks: own_start_ticks()?, started_at, closed: false };
        record.write(None)?;
        Ok(record)
    }

    fn write(&self, closed_at: Option<u64>) -> Result<(), String> {
        let v = json!({"format": "runbook.runner/1", "pid": self.pid, "start_ticks": self.start_ticks, "started_at": self.started_at, "closed_at": closed_at});
        let text = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())? + "\n";
        write_atomic(&self.path, text.as_bytes())
    }

    /// Closes the record at `at`; a second call does nothing.
    pub fn close(&mut self, at: u64) {
        if !self.closed {
            self.closed = true;
            let _ = self.write(Some(at.max(self.started_at)));
        }
    }
}

impl Drop for RunnerRecord {
    fn drop(&mut self) {
        self.close(system_secs());
    }
}

/// Appends one event as one line of `events.jsonl`, in one write, to a regular file with one name only.
///
/// The programs of a run can write the run directory (`--confinement local`), so what lies at `events.jsonl` is checked,
/// not assumed: the open follows no link, waits for no reader of a pipe and creates nothing, and the descriptor that is
/// open must be a regular file with one name (a file with a second name may be a file outside the run). Anything else is
/// left alone and the event is not written; the run goes on.
pub fn append_event(run_dir: &Path, at: u64, event: &Event) {
    let Ok(mut line) = serde_json::to_string(&event_json(at, event)) else { return };
    line.push('\n');
    let _ = append(&run_dir.join("events.jsonl"), line.as_bytes(), false);
}

/// The target of `runbook run`: the event of a program that starts goes into `events.jsonl` of the run directory.
struct RunDirTarget {
    run_dir: PathBuf,
}

impl StartTarget for RunDirTarget {
    fn started(&self, start: &Started) {
        let event = Event::ActionStarted { source: StartSource::ProgramStart, role: Some(start.role), case: start.case.clone(), action: start.action.clone(), try_number: start.try_number };
        append_event(&self.run_dir, system_secs(), &event);
    }
}

/// Wraps `inner` so that the event of every bound program is written into `run_dir/events.jsonl` before the program
/// starts; actions are read from the bindings copy in `bindings_dir`.
pub fn announcing(inner: Arc<dyn TestRunner>, run_dir: PathBuf, bindings_dir: PathBuf) -> Announcing {
    Announcing::to(inner, Arc::new(RunDirTarget { run_dir }), bindings_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::current_exe().unwrap().parent().unwrap().join(format!("in-flight-unit-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn read(d: &Path) -> Value {
        serde_json::from_str(&std::fs::read_to_string(d.join(RUNNER_FILE)).unwrap()).unwrap()
    }

    #[test]
    fn a_record_dropped_without_a_close_is_closed() {
        let d = dir("drop");
        let record = RunnerRecord::create(&d, 1_700_000_000).unwrap();
        assert!(read(&d)["closed_at"].is_null());
        drop(record);
        assert!(read(&d)["closed_at"].as_u64().is_some_and(|t| t >= 1_700_000_000));
    }

    #[test]
    fn a_close_is_kept_by_the_drop_after_it() {
        let d = dir("close");
        let mut record = RunnerRecord::create(&d, 100).unwrap();
        record.close(150);
        drop(record);
        assert_eq!(read(&d)["closed_at"], 150);
    }

    #[test]
    fn an_event_is_appended_only_to_a_regular_file_with_one_name() {
        let d = dir("append");
        let event = Event::ActionStarted { source: StartSource::ProgramStart, role: Some("program"), case: None, action: None, try_number: None };
        let outside = d.join("outside.txt");
        std::fs::write(&outside, "keep\n").unwrap();
        let events = d.join("events.jsonl");
        // no file: nothing is created
        append_event(&d, 1, &event);
        assert!(!events.exists(), "an events file was created");
        // a regular file with one name: one line
        std::fs::write(&events, "").unwrap();
        append_event(&d, 1, &event);
        assert_eq!(std::fs::read_to_string(&events).unwrap().lines().count(), 1);
        // a link, a second name of a file outside, a pipe and a directory: left alone, and the call returns
        std::fs::remove_file(&events).unwrap();
        std::os::unix::fs::symlink(&outside, &events).unwrap();
        append_event(&d, 1, &event);
        std::fs::remove_file(&events).unwrap();
        std::fs::hard_link(&outside, &events).unwrap();
        append_event(&d, 1, &event);
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep\n");
        std::fs::remove_file(&events).unwrap();
        let c = std::ffi::CString::new(events.to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o644) }, 0);
        append_event(&d, 1, &event);
        std::fs::remove_file(&events).unwrap();
        std::fs::create_dir(&events).unwrap();
        append_event(&d, 1, &event);
    }

    #[test]
    fn an_event_is_not_appended_to_a_pipe_that_has_a_reader() {
        use std::io::Read;
        use std::os::unix::fs::OpenOptionsExt;
        let d = dir("pipe-reader");
        let events = d.join("events.jsonl");
        let c = std::ffi::CString::new(events.to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o644) }, 0);
        let mut reader = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(&events).unwrap();
        let event = Event::ActionStarted { source: StartSource::ProgramStart, role: Some("program"), case: None, action: None, try_number: None };
        append_event(&d, 1, &event);
        let mut got = Vec::new();
        let read = reader.read_to_end(&mut got);
        assert!(got.is_empty(), "a line was written into a pipe: {:?} {read:?}", String::from_utf8_lossy(&got));
    }

    #[test]
    fn the_start_event_has_the_form_of_its_neighbours() {
        let event = Event::ActionStarted { source: StartSource::ProgramStart, role: Some("program"), case: Some("C-1".into()), action: Some("a.one".into()), try_number: Some(2) };
        assert_eq!(event_json(1_700_000_000, &event), json!({"at": 1_700_000_000u64, "event": "action_started", "source": "program_start", "role": "program", "case": "C-1", "action": "a.one", "try": 2}));
    }
}
