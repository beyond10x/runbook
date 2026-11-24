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
//! program starts through. [`Announcing`] wraps that runner and writes the event there, before the
//! program starts. Loom's `tree run` writes no `ActionStarted` line in the pinned commit and hands
//! its observer lines only after they are written, so the action is not named by Loom at that
//! point: `case` is Loom's own `LOOM_CASE` of the child, `action` is the action of the bindings
//! copy whose argv (or `handoff_when`) is the child's program and arguments, and `try` is counted
//! by this runner. See `docs/loom-run-directory.md`.

use crate::loom_run::events::{Event, StartSource};
use crate::loom_run::record_write::{append, write_atomic, Held};
use crate::loom_run::run_files::event_json;
use b10x_loom_intake_slice::confinement::{Backend, ChildCommand, ConfinementError, TestExecution, TestRunner};
use b10x_loom_intake_slice::executor::TestCommand;
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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

/// One action of the bindings copy that runs a program: its argv and, when it has one, the argv of
/// its `handoff_when` check.
struct Bound {
    action: String,
    argv: Vec<String>,
    check: Vec<String>,
}

fn words(v: Option<&serde_yaml::Value>) -> Vec<String> {
    v.and_then(serde_yaml::Value::as_sequence).map(|s| s.iter().filter_map(|w| w.as_str().map(str::to_owned)).collect()).unwrap_or_default()
}

/// The actions of the bindings copy in `dir` that run a program.
fn bound_actions(dir: &Path) -> Vec<Bound> {
    let Ok(held) = Held::open(dir) else { return Vec::new() };
    let mut found = Vec::new();
    for name in held.names().unwrap_or_default().into_iter().filter(|n| Path::new(n).extension().is_some_and(|x| x == "yaml")) {
        let Ok(text) = held.read_text(&name) else { continue };
        let Ok(doc) = serde_yaml::from_str::<serde_yaml::Value>(&text) else { continue };
        let Some(actions) = doc.get("actions").and_then(serde_yaml::Value::as_sequence) else { continue };
        for entry in actions {
            let Some(action) = entry.get("action").and_then(serde_yaml::Value::as_str) else { continue };
            let argv = words(entry.get("executor").filter(|e| e.get("kind").and_then(serde_yaml::Value::as_str) == Some("program")).and_then(|e| e.get("argv")));
            let check = words(entry.get("handoff_when"));
            if !argv.is_empty() || !check.is_empty() {
                found.push(Bound { action: action.to_owned(), argv, check });
            }
        }
    }
    found
}

/// Whether `child` runs `argv`: the program is argv's first word (as written, or resolved below the
/// bindings copy) and the arguments are the rest.
fn runs(child: &ChildCommand, argv: &[String]) -> bool {
    let Some((program, args)) = argv.split_first() else { return false };
    (child.program() == program || Path::new(child.program()).ends_with(program)) && child.args() == args
}

/// The action the bindings copy binds to `child`'s program and arguments, when exactly one does.
fn action_of(bound: &[Bound], child: &ChildCommand, check: bool) -> Option<String> {
    let mut hits: Vec<&str> = bound.iter().filter(|b| runs(child, if check { &b.check } else { &b.argv })).map(|b| b.action.as_str()).collect();
    hits.dedup();
    match hits.as_slice() {
        [one] => Some((*one).to_owned()),
        _ => None,
    }
}

struct Notice {
    run_dir: PathBuf,
    bindings_dir: PathBuf,
    bound: Mutex<Option<Arc<Vec<Bound>>>>,
    tries: Mutex<HashMap<(String, String), u64>>,
}

impl Notice {
    fn bound(&self) -> Arc<Vec<Bound>> {
        let mut slot = self.bound.lock().unwrap_or_else(|e| e.into_inner());
        Arc::clone(slot.get_or_insert_with(|| Arc::new(bound_actions(&self.bindings_dir))))
    }

    /// Writes the event of the program `child` that is about to start. Best effort: a file that
    /// cannot be written leaves the run going, it is a record for a reader and not a step of the run.
    fn announce(&self, child: &ChildCommand) {
        let check = child.read_only_workspace();
        let case = child.environment().set.get("LOOM_CASE").cloned();
        let action = action_of(&self.bound(), child, check);
        let try_number = match (&case, &action, check) {
            (Some(case), Some(action), false) => {
                let mut tries = self.tries.lock().unwrap_or_else(|e| e.into_inner());
                let n = tries.entry((case.clone(), action.clone())).or_insert(0);
                *n += 1;
                Some(*n)
            }
            _ => None,
        };
        let event = Event::ActionStarted { source: StartSource::ProgramStart, role: Some(if check { "check" } else { "program" }), case, action, try_number };
        append_event(&self.run_dir, system_secs(), &event);
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

/// The runner every bound program starts through, with the event of its start written before it
/// starts. Every other call goes to the runner it wraps unchanged.
pub struct Announcing {
    inner: Arc<dyn TestRunner>,
    notice: Notice,
}

impl Announcing {
    /// Wraps `inner`: events go into `run_dir/events.jsonl`, actions are read from the bindings copy in `bindings_dir`.
    pub fn new(inner: Arc<dyn TestRunner>, run_dir: PathBuf, bindings_dir: PathBuf) -> Announcing {
        Announcing { inner, notice: Notice { run_dir, bindings_dir, bound: Mutex::new(None), tries: Mutex::new(HashMap::new()) } }
    }
}

impl TestRunner for Announcing {
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
        self.notice.announce(child);
        self.inner.run_child(child)
    }

    fn preflight_child(&self, root: &Path) -> Result<(), ConfinementError> {
        self.inner.preflight_child(root)
    }
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

    fn child(program: &str, args: &[&str]) -> ChildCommand {
        ChildCommand::new(program, args.iter().map(|a| (*a).to_owned()), Path::new("."))
    }

    fn bound(action: &str, argv: &[&str], check: &[&str]) -> Bound {
        Bound { action: action.to_owned(), argv: argv.iter().map(|a| (*a).to_owned()).collect(), check: check.iter().map(|a| (*a).to_owned()).collect() }
    }

    #[test]
    fn an_action_is_named_only_when_exactly_one_binding_runs_the_child() {
        let table = [bound("a.one", &["tools/x.sh", "--one"], &[]), bound("a.two", &["tools/x.sh", "--two"], &["tools/x.sh", "wait"]), bound("a.three", &["tools/x.sh", "--two"], &[])];
        assert_eq!(action_of(&table, &child("/bindings/tools/x.sh", &["--one"]), false).as_deref(), Some("a.one"));
        assert_eq!(action_of(&table, &child("tools/x.sh", &["--one"]), false).as_deref(), Some("a.one"));
        assert_eq!(action_of(&table, &child("tools/x.sh", &["--two"]), false), None, "two actions run it: no guess");
        assert_eq!(action_of(&table, &child("tools/x.sh", &["wait"]), true).as_deref(), Some("a.two"));
        assert_eq!(action_of(&table, &child("tools/x.sh", &["wait"]), false), None, "a check is not a program of an action");
        assert_eq!(action_of(&table, &child("tools/other.sh", &[]), false), None);
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

    #[test]
    fn a_link_a_program_put_at_a_bindings_file_is_not_read_for_the_bound_actions() {
        let d = dir("bound-link");
        let outside = dir("bound-outside");
        let text = "actions:\n- action: a.one\n  executor:\n    kind: program\n    argv: [prog, one]\n";
        std::fs::write(outside.join("steps.yaml"), text).unwrap();
        std::os::unix::fs::symlink(outside.join("steps.yaml"), d.join("b.yaml")).unwrap();
        assert!(bound_actions(&d).is_empty(), "a file outside the run was read through a link at a bindings file");
        std::fs::write(d.join("c.yaml"), text).unwrap();
        assert_eq!(bound_actions(&d).len(), 1);
    }
}
