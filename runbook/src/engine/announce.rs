//! The one hook before a bound program starts, and the wrapper that uses it.
//!
//! The library gives one point before a bound program starts: the `run_child` of the runner every
//! program starts through. [`Announcing`] wraps that runner, tells a [`StartTarget`] before the
//! program starts and again after it ended, and hands every other call to the runner it wraps
//! unchanged. Both runtimes use it: `runbook run` writes an `action_started` line into the run
//! directory (`loom_run::in_flight`), and the daemon records a run event and keeps the step in
//! flight (`commands`, through `runtime::EventSink::steps`).
//!
//! Loom's `tree run` writes no `ActionStarted` line in the pinned commit and hands its observer
//! lines only after they are written, so the action is not named by Loom at that point: `case` is
//! Loom's own `LOOM_CASE` of the child, `action` is the action of the bindings whose argv (or
//! `handoff_when`) is the child's program and arguments, and `try` is counted by this wrapper. See
//! `docs/loom-run-directory.md`.

use crate::runtime::{StartTarget, Started};
use b10x_loom_intake_slice::confinement::{Backend, ChildCommand, ConfinementError, TestExecution, TestRunner};
use b10x_loom_intake_slice::executor::TestCommand;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// One action of the bindings in a directory that runs a program: its argv and, when it has one,
/// the argv of its `handoff_when` check.
struct Bound {
    action: String,
    argv: Vec<String>,
    check: Vec<String>,
}

fn words(v: Option<&serde_yaml::Value>) -> Vec<String> {
    v.and_then(serde_yaml::Value::as_sequence).map(|s| s.iter().filter_map(|w| w.as_str().map(str::to_owned)).collect()).unwrap_or_default()
}

/// How many bytes of one bindings file are read: 64 MiB, the bound of a read of a name of a run.
const BINDINGS_READ_LIMIT: u64 = 64 * 1024 * 1024;

/// The texts of the `*.yaml` files of the bindings directory `dir`, in the order of their names, read as the names of a
/// run are: the directory is opened once without following a link at its name and is listed and read through that
/// descriptor, and each file is opened relative to it without following a link and without waiting for a writer, taken
/// only when it is a regular file with one name, and read for at most [`BINDINGS_READ_LIMIT`] bytes. A link at the
/// directory gives no file; a file that is not one of these is left out.
fn bindings_texts(dir: &Path) -> Vec<String> {
    use std::io::Read;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    let Ok(held) = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC).open(dir) else { return Vec::new() };
    let listing = PathBuf::from(format!("/proc/self/fd/{}", held.as_raw_fd()));
    let mut names: Vec<std::ffi::OsString> = std::fs::read_dir(&listing).map(|e| e.filter_map(Result::ok).map(|e| e.file_name()).filter(|n| Path::new(n).extension().is_some_and(|x| x == "yaml")).collect()).unwrap_or_default();
    names.sort();
    let mut texts = Vec::new();
    for name in names {
        let Ok(c_name) = std::ffi::CString::new(name.as_bytes()) else { continue };
        // SAFETY: `held` is an open directory for the call and `c_name` a NUL-terminated name that outlives it.
        let raw = unsafe { libc::openat(held.as_raw_fd(), c_name.as_ptr(), libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC) };
        if raw < 0 {
            continue;
        }
        // SAFETY: `raw` is a new descriptor that nothing else owns.
        let file = unsafe { std::fs::File::from_raw_fd(raw) };
        if !file.metadata().is_ok_and(|m| m.is_file() && m.nlink() == 1) {
            continue;
        }
        let mut bytes = Vec::new();
        if file.take(BINDINGS_READ_LIMIT + 1).read_to_end(&mut bytes).is_err() || bytes.len() as u64 > BINDINGS_READ_LIMIT {
            continue;
        }
        if let Ok(text) = String::from_utf8(bytes) {
            texts.push(text);
        }
    }
    texts
}

/// The actions of the bindings files in `dir` that run a program ([`bindings_texts`]).
fn bound_actions(dir: &Path) -> Vec<Bound> {
    let mut found = Vec::new();
    for text in bindings_texts(dir) {
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
/// bindings) and the arguments are the rest.
fn runs(child: &ChildCommand, argv: &[String]) -> bool {
    let Some((program, args)) = argv.split_first() else { return false };
    (child.program() == program || Path::new(child.program()).ends_with(program)) && child.args() == args
}

/// The action the bindings bind to `child`'s program and arguments, when exactly one does.
fn action_of(bound: &[Bound], child: &ChildCommand, check: bool) -> Option<String> {
    let mut hits: Vec<&str> = bound.iter().filter(|b| runs(child, if check { &b.check } else { &b.argv })).map(|b| b.action.as_str()).collect();
    hits.dedup();
    match hits.as_slice() {
        [one] => Some((*one).to_owned()),
        _ => None,
    }
}

struct Notice {
    bindings_dir: PathBuf,
    bound: Mutex<Option<Arc<Vec<Bound>>>>,
    tries: Mutex<HashMap<(String, String), u64>>,
    serial: AtomicU64,
}

impl Notice {
    fn bound(&self) -> Arc<Vec<Bound>> {
        let mut slot = self.bound.lock().unwrap_or_else(|e| e.into_inner());
        Arc::clone(slot.get_or_insert_with(|| Arc::new(bound_actions(&self.bindings_dir))))
    }

    /// What the program `child` that is about to start is.
    fn describe(&self, child: &ChildCommand) -> Started {
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
        Started { serial: self.serial.fetch_add(1, Ordering::SeqCst) + 1, role: if check { "check" } else { "program" }, case, action, try_number }
    }
}

/// The runner every bound program starts through, with its start told to a [`StartTarget`] before it
/// starts and its end told after. Every other call goes to the runner it wraps unchanged.
pub struct Announcing {
    inner: Arc<dyn TestRunner>,
    target: Arc<dyn StartTarget>,
    notice: Notice,
}

impl Announcing {
    /// Wraps `inner`: starts and ends are told to `target`, actions are read from the bindings files in `bindings_dir`.
    pub fn to(inner: Arc<dyn TestRunner>, target: Arc<dyn StartTarget>, bindings_dir: PathBuf) -> Announcing {
        Announcing { inner, target, notice: Notice { bindings_dir, bound: Mutex::new(None), tries: Mutex::new(HashMap::new()), serial: AtomicU64::new(0) } }
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
        let start = self.notice.describe(child);
        self.target.started(&start);
        let result = self.inner.run_child(child);
        self.target.ended(&start);
        result
    }

    fn preflight_child(&self, root: &Path) -> Result<(), ConfinementError> {
        self.inner.preflight_child(root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::current_exe().unwrap().parent().unwrap().join(format!("announce-unit-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    const BOUND_TEXT: &str = "actions:\n- action: a.one\n  executor:\n    kind: program\n    argv: [prog, one]\n";

    #[test]
    fn a_link_a_program_put_at_a_bindings_file_is_not_read_for_the_bound_actions() {
        let d = dir("bound-link");
        let outside = dir("bound-outside");
        std::fs::write(outside.join("steps.yaml"), BOUND_TEXT).unwrap();
        std::os::unix::fs::symlink(outside.join("steps.yaml"), d.join("b.yaml")).unwrap();
        assert!(bound_actions(&d).is_empty(), "a file outside the run was read through a link at a bindings file");
        std::fs::write(d.join("c.yaml"), BOUND_TEXT).unwrap();
        assert_eq!(bound_actions(&d).len(), 1);
    }

    #[test]
    fn a_link_a_program_put_at_the_bindings_directory_is_not_read_for_the_bound_actions() {
        let d = dir("bound-dir-link");
        let outside = dir("bound-dir-outside");
        std::fs::write(outside.join("steps.yaml"), BOUND_TEXT).unwrap();
        std::os::unix::fs::symlink(&outside, d.join("bindings")).unwrap();
        assert!(bound_actions(&d.join("bindings")).is_empty(), "a file outside the run was read through a link at the bindings directory");
        assert_eq!(bound_actions(&outside).len(), 1, "the bindings directory itself is read");
    }

    #[test]
    fn a_named_pipe_and_a_second_name_at_a_bindings_file_are_not_read_for_the_bound_actions() {
        let d = dir("bound-pipe");
        let outside = dir("bound-pipe-outside");
        std::fs::write(outside.join("steps.yaml"), BOUND_TEXT).unwrap();
        std::fs::hard_link(outside.join("steps.yaml"), d.join("b.yaml")).unwrap();
        let fifo = std::ffi::CString::new(d.join("p.yaml").to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o644) }, 0);
        let (tx, rx) = std::sync::mpsc::channel();
        let probe = d.clone();
        std::thread::spawn(move || {
            let _ = tx.send(bound_actions(&probe).len());
        });
        let got = rx.recv_timeout(std::time::Duration::from_secs(10));
        if got.is_err() {
            let _ = std::fs::OpenOptions::new().write(true).open(d.join("p.yaml"));
        }
        assert_eq!(got.ok(), Some(0), "a bindings file is read as a regular file with one name and no pipe is waited for");
    }

    fn child_of_case(program: &str, args: &[&str], dir: &Path, case: &str) -> ChildCommand {
        use b10x_loom_intake_slice::confinement::ExecEnvironment;
        ChildCommand::new(program, args.iter().map(|a| (*a).to_owned()), dir).with_environment(ExecEnvironment { allow: Vec::new(), set: std::collections::BTreeMap::from([("LOOM_CASE".to_string(), case.to_string())]) })
    }

    /// Keeps what it is told, with whether `marker` exists at that moment.
    #[derive(Default)]
    struct Recorder {
        log: Mutex<Vec<(&'static str, Started, bool)>>,
        marker: Mutex<Option<PathBuf>>,
    }

    impl Recorder {
        fn note(&self, what: &'static str, s: &Started) {
            let there = self.marker.lock().unwrap().as_ref().is_some_and(|m| m.exists());
            self.log.lock().unwrap().push((what, s.clone(), there));
        }
    }

    impl StartTarget for Recorder {
        fn started(&self, s: &Started) {
            self.note("started", s);
        }

        fn ended(&self, s: &Started) {
            self.note("ended", s);
        }
    }

    #[test]
    fn announcing_tells_the_start_before_and_the_end_after_the_program_counts_tries_per_case_and_no_try_for_a_check() {
        let dir = dir("tries");
        let bindings = dir.join("bindings");
        let work = dir.join("work");
        std::fs::create_dir_all(&bindings).unwrap();
        std::fs::create_dir_all(&work).unwrap();
        std::fs::write(bindings.join("b.yaml"), "format: loom.bindings/1\nactions:\n  - action: a.one\n    executor:\n      kind: program\n      argv: [sh, -c, \"echo x > ran.txt\"]\n    handoff_when: [sh, -c, \"test -f ran.txt\"]\n").unwrap();
        let rec = Arc::new(Recorder::default());
        *rec.marker.lock().unwrap() = Some(work.join("ran.txt"));
        let wrapper = Announcing::to(crate::engine::calls::runner(crate::runtime::Confinement::Local).unwrap(), rec.clone(), bindings);
        let program = |case: &str| child_of_case("sh", &["-c", "echo x > ran.txt"], &work, case);
        wrapper.run_child(&program("c1")).unwrap();
        wrapper.run_child(&program("c1")).unwrap();
        wrapper.run_child(&child_of_case("sh", &["-c", "test -f ran.txt"], &work, "c1").with_read_only_workspace()).unwrap();
        wrapper.run_child(&program("c2")).unwrap();
        let log = rec.log.lock().unwrap().clone();
        let shape: Vec<_> = log.iter().map(|(w, s, _)| (*w, s.serial, s.role, s.case.as_deref(), s.action.as_deref(), s.try_number)).collect();
        assert_eq!(
            shape,
            vec![
                ("started", 1, "program", Some("c1"), Some("a.one"), Some(1)),
                ("ended", 1, "program", Some("c1"), Some("a.one"), Some(1)),
                ("started", 2, "program", Some("c1"), Some("a.one"), Some(2)),
                ("ended", 2, "program", Some("c1"), Some("a.one"), Some(2)),
                ("started", 3, "check", Some("c1"), Some("a.one"), None),
                ("ended", 3, "check", Some("c1"), Some("a.one"), None),
                ("started", 4, "program", Some("c2"), Some("a.one"), Some(1)),
                ("ended", 4, "program", Some("c2"), Some("a.one"), Some(1)),
            ]
        );
        // the first program wrote ran.txt: absent when its start was told, present when its end was
        assert_eq!((log[0].2, log[1].2), (false, true), "the start was told after the program ran, or the end before it");
    }

    struct CannotStart;

    impl TestRunner for CannotStart {
        fn run(&self, _c: &TestCommand, _r: &Path) -> Result<TestExecution, ConfinementError> {
            unreachable!()
        }

        fn backend(&self) -> Backend {
            Backend::Substrate
        }
    }

    #[test]
    fn announcing_tells_the_end_of_a_program_that_could_not_start() {
        let dir = dir("cannot-start");
        let rec = Arc::new(Recorder::default());
        let wrapper = Announcing::to(Arc::new(CannotStart), rec.clone(), dir.clone());
        assert!(wrapper.run_child(&child_of_case("sh", &["-c", "true"], &dir, "c1")).is_err());
        let log: Vec<(&str, u64)> = rec.log.lock().unwrap().iter().map(|(w, s, _)| (*w, s.serial)).collect();
        assert_eq!(log, vec![("started", 1), ("ended", 1)]);
    }
}
