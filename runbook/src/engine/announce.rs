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
//! `handoff_when`) is the child's program and arguments, and `try` is counted by this wrapper. When
//! more than one bindings file of the directory binds that argv (two runbooks of one home that run
//! the same program with the same arguments), the action is the one of the file whose `protocol` is
//! the protocol of the child's case, read from the `CaseOpened` line of the case's run log in the
//! run's store ([`Announcing::reading_cases`]). See `docs/loom-run-directory.md`.

use crate::record_write::Held;
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
    /// The `protocol` of the bindings file (`<id>@<revision>`), when it names one.
    protocol: Option<String>,
}

fn words(v: Option<&serde_yaml::Value>) -> Vec<String> {
    v.and_then(serde_yaml::Value::as_sequence).map(|s| s.iter().filter_map(|w| w.as_str().map(str::to_owned)).collect()).unwrap_or_default()
}

/// The texts of the `*.yaml` files of the bindings directory `dir`, in the order of their names, read as the names of a
/// run are, through the one guard of the run's records ([`Held`]): the directory is opened once without following a link
/// at its name ([`Held::open`]) and is listed ([`Held::names`]) and read ([`Held::read_text`]) through that descriptor,
/// each file without following a link and without waiting for a writer, only when it is a regular file with one name, for
/// at most [`crate::record_write::READ_LIMIT`] bytes. A link at the directory gives no file; a file that is not one of
/// these is left out.
fn bindings_texts(dir: &Path) -> Vec<String> {
    let Ok(held) = Held::open(dir) else { return Vec::new() };
    let names = held.names().unwrap_or_default();
    names.into_iter().filter(|n| Path::new(n).extension().is_some_and(|x| x == "yaml")).filter_map(|n| held.read_text(&n).ok()).collect()
}

/// The actions of the bindings files in `dir` that run a program ([`bindings_texts`]).
fn bound_actions(dir: &Path) -> Vec<Bound> {
    let mut found = Vec::new();
    for text in bindings_texts(dir) {
        let Ok(doc) = serde_yaml::from_str::<serde_yaml::Value>(&text) else { continue };
        let Some(actions) = doc.get("actions").and_then(serde_yaml::Value::as_sequence) else { continue };
        let protocol = doc.get("protocol").and_then(serde_yaml::Value::as_str).map(str::to_owned);
        for entry in actions {
            let Some(action) = entry.get("action").and_then(serde_yaml::Value::as_str) else { continue };
            let argv = words(entry.get("executor").filter(|e| e.get("kind").and_then(serde_yaml::Value::as_str) == Some("program")).and_then(|e| e.get("argv")));
            let check = words(entry.get("handoff_when"));
            if !argv.is_empty() || !check.is_empty() {
                found.push(Bound { action: action.to_owned(), argv, check, protocol: protocol.clone() });
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

/// The action the bindings bind to `child`'s program and arguments, when exactly one does. `protocol` is the protocol
/// of the child's case (`<id>@<revision>`), when it is known: when more than one action binds the child, only those of
/// the bindings files of that protocol count (two runbooks of one home may run one program with the same arguments).
fn action_of(bound: &[Bound], child: &ChildCommand, check: bool, protocol: Option<&str>) -> Option<String> {
    fn names<'a>(of: &[&'a Bound]) -> Vec<&'a str> {
        let mut names: Vec<&str> = of.iter().map(|b| b.action.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        names
    }
    let matching: Vec<&Bound> = bound.iter().filter(|b| runs(child, if check { &b.check } else { &b.argv })).collect();
    let mut hits = names(&matching);
    if hits.len() > 1 {
        let of_case: Vec<&Bound> = matching.iter().copied().filter(|b| protocol.is_some() && b.protocol.as_deref() == protocol).collect();
        hits = names(&of_case);
    }
    match hits.as_slice() {
        [one] => Some((*one).to_owned()),
        _ => None,
    }
}

/// The longest first line of a case's run log that is read for its protocol.
const CASE_LINE_LIMIT: u64 = 64 * 1024;

/// The protocol (`<id>@<revision>`) of the case `case` of the store `store`: the `protocol` of the `CaseOpened` line
/// that opens the case's run log. The store is opened once without following a link at its name and the log is opened
/// through it without following a link and without waiting for a writer, only when it is a regular file, and only its
/// first line is read, at most [`CASE_LINE_LIMIT`] bytes. `None` when any of that does not hold.
fn protocol_of_case(store: &Path, case: &str) -> Option<String> {
    use std::io::{BufRead, Read};
    let held = Held::open(store).ok()?;
    let file = held.open_as_is(b10x_loom_runlog::log_file_name(case)).ok()?;
    if !file.metadata().ok()?.file_type().is_file() {
        return None;
    }
    let mut line = String::new();
    std::io::BufReader::new(file.take(CASE_LINE_LIMIT)).read_line(&mut line).ok()?;
    let opened: serde_json::Value = serde_json::from_str(line.trim_end()).ok()?;
    if opened.get("kind").and_then(serde_json::Value::as_str) != Some("CaseOpened") || opened.get("case").and_then(serde_json::Value::as_str) != Some(case) {
        return None;
    }
    opened.get("protocol").and_then(serde_json::Value::as_str).map(str::to_owned)
}

struct Notice {
    bindings_dir: PathBuf,
    bound: Mutex<Option<Arc<Vec<Bound>>>>,
    /// The run's store, where the protocol of a case is read ([`protocol_of_case`]); none: no protocol is read.
    store: Option<PathBuf>,
    /// The protocol of each case read so far: a case keeps the protocol it was opened with.
    protocols: Mutex<HashMap<String, String>>,
    tries: Mutex<HashMap<(String, String), u64>>,
    serial: AtomicU64,
}

impl Notice {
    fn bound(&self) -> Arc<Vec<Bound>> {
        let mut slot = self.bound.lock().unwrap_or_else(|e| e.into_inner());
        Arc::clone(slot.get_or_insert_with(|| Arc::new(bound_actions(&self.bindings_dir))))
    }

    /// The protocol of the case `case`, read once from the run's store.
    fn protocol(&self, case: &str) -> Option<String> {
        let store = self.store.as_deref()?;
        let mut known = self.protocols.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(p) = known.get(case) {
            return Some(p.clone());
        }
        let read = protocol_of_case(store, case)?;
        known.insert(case.to_owned(), read.clone());
        Some(read)
    }

    /// What the program `child` that is about to start is.
    fn describe(&self, child: &ChildCommand) -> Started {
        let check = child.read_only_workspace();
        let case = child.environment().set.get("LOOM_CASE").cloned();
        let protocol = case.as_deref().and_then(|c| self.protocol(c));
        let action = action_of(&self.bound(), child, check, protocol.as_deref());
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
        Announcing {
            inner,
            target,
            notice: Notice { bindings_dir, bound: Mutex::new(None), store: None, protocols: Mutex::new(HashMap::new()), tries: Mutex::new(HashMap::new()), serial: AtomicU64::new(0) },
        }
    }

    /// Reads the protocol of a program's case from the run's store `store` (the `CaseOpened` line of the case's run
    /// log): when more than one bindings file binds the program's argv, the action is the one of the file whose
    /// `protocol` is the case's.
    pub fn reading_cases(mut self, store: PathBuf) -> Announcing {
        self.notice.store = Some(store);
        self
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
        Bound { action: action.to_owned(), argv: argv.iter().map(|a| (*a).to_owned()).collect(), check: check.iter().map(|a| (*a).to_owned()).collect(), protocol: None }
    }

    #[test]
    fn an_action_is_named_only_when_exactly_one_binding_runs_the_child() {
        let table = [bound("a.one", &["tools/x.sh", "--one"], &[]), bound("a.two", &["tools/x.sh", "--two"], &["tools/x.sh", "wait"]), bound("a.three", &["tools/x.sh", "--two"], &[])];
        assert_eq!(action_of(&table, &child("/bindings/tools/x.sh", &["--one"]), false, None).as_deref(), Some("a.one"));
        assert_eq!(action_of(&table, &child("tools/x.sh", &["--one"]), false, None).as_deref(), Some("a.one"));
        assert_eq!(action_of(&table, &child("tools/x.sh", &["--two"]), false, None), None, "two actions run it: no guess");
        assert_eq!(action_of(&table, &child("tools/x.sh", &["wait"]), true, None).as_deref(), Some("a.two"));
        assert_eq!(action_of(&table, &child("tools/x.sh", &["wait"]), false, None), None, "a check is not a program of an action");
        assert_eq!(action_of(&table, &child("tools/other.sh", &[]), false, None), None);
    }

    #[test]
    fn two_bindings_files_that_bind_one_argv_name_the_action_of_the_cases_protocol() {
        let of = |action: &str, protocol: &str| Bound { protocol: Some(protocol.to_owned()), ..bound(action, &["tools/call.sh", "-x"], &["tools/call.sh", "wait"]) };
        let table = [of("lib.call", "lib@1"), of("strict.call", "strict@1")];
        let program = child("tools/call.sh", &["-x"]);
        assert_eq!(action_of(&table, &program, false, Some("strict@1")).as_deref(), Some("strict.call"));
        assert_eq!(action_of(&table, &program, false, Some("lib@1")).as_deref(), Some("lib.call"));
        assert_eq!(action_of(&table, &child("tools/call.sh", &["wait"]), true, Some("strict@1")).as_deref(), Some("strict.call"), "a check of the case's protocol");
        assert_eq!(action_of(&table, &program, false, None), None, "no protocol of the case: no guess");
        assert_eq!(action_of(&table, &program, false, Some("other@1")), None, "a protocol no file binds: no guess");
        let one_file = [of("a.one", "p@1"), of("a.two", "p@1")];
        assert_eq!(action_of(&one_file, &program, false, Some("p@1")), None, "two actions of one protocol run it: no guess");
    }

    #[test]
    fn the_protocol_of_a_case_is_read_from_its_run_log_and_not_through_a_link() {
        let d = dir("case-protocol");
        let store = d.join("store");
        std::fs::create_dir_all(&store).unwrap();
        let log = |case: &str| store.join(b10x_loom_runlog::log_file_name(case));
        std::fs::write(log("c1"), "{\"seq\":0,\"kind\":\"CaseOpened\",\"case\":\"c1\",\"protocol\":\"strict@1\"}\n{\"seq\":1,\"kind\":\"Stopped\"}\n").unwrap();
        assert_eq!(protocol_of_case(&store, "c1").as_deref(), Some("strict@1"));
        assert_eq!(protocol_of_case(&store, "c2"), None, "a case with no run log");
        std::fs::write(d.join("elsewhere.jsonl"), "{\"seq\":0,\"kind\":\"CaseOpened\",\"case\":\"c3\",\"protocol\":\"lib@1\"}\n").unwrap();
        std::os::unix::fs::symlink(d.join("elsewhere.jsonl"), log("c3")).unwrap();
        assert_eq!(protocol_of_case(&store, "c3"), None, "a run log read through a link");
        std::fs::write(log("c4"), "{\"seq\":0,\"kind\":\"CaseOpened\",\"case\":\"c1\",\"protocol\":\"lib@1\"}\n").unwrap();
        assert_eq!(protocol_of_case(&store, "c4"), None, "a run log that opens another case");
    }

    #[test]
    fn announcing_names_the_action_of_the_cases_protocol_when_two_runbooks_bind_one_program() {
        let d = dir("two-runbooks");
        let (bindings, store) = (d.join("bindings"), d.join("store"));
        std::fs::create_dir_all(&bindings).unwrap();
        std::fs::create_dir_all(&store).unwrap();
        for (file, protocol, action) in [("lib.yaml", "lib@1", "lib.call"), ("strict.yaml", "strict@1", "strict.call")] {
            std::fs::write(bindings.join(file), format!("format: loom.bindings/1\nprotocol: {protocol}\nactions:\n  - action: {action}\n    executor:\n      kind: program\n      argv: [sh, -c, \"true\"]\n")).unwrap();
        }
        std::fs::write(store.join(b10x_loom_runlog::log_file_name("run-1")), "{\"seq\":0,\"kind\":\"CaseOpened\",\"case\":\"run-1\",\"protocol\":\"strict@1\"}\n").unwrap();
        let rec = Arc::new(Recorder::default());
        let wrapper = Announcing::to(crate::engine::calls::runner(crate::runtime::Confinement::Local).unwrap(), rec.clone(), bindings).reading_cases(store);
        wrapper.run_child(&child_of_case("sh", &["-c", "true"], &d, "run-1")).unwrap();
        let named: Vec<(Option<String>, Option<u64>)> = rec.log.lock().unwrap().iter().map(|(_, s, _)| (s.action.clone(), s.try_number)).collect();
        assert_eq!(named, vec![(Some("strict.call".to_owned()), Some(1)); 2]);
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
