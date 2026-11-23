//! `runbook run --watch`: `sdc-watch` beside the run records every file change of the run in
//! `fs/changes.jsonl`, attributed to the `tree run` step in flight, and ends with the run.
//!
//! The runs are the engine fixture's runbook in process (`tests/fixtures/engine-runbook`: a writer
//! program and one person step, answered by a scripted decider): no model call, no network. The
//! helper is the real `sdc-watch` of `watch/` (`task test:runbook` builds it before the tests; by
//! hand: `cd watch && cargo build --locked`); `RUNBOOK_HOME` points at the fixture so that the
//! helper is looked up by `PATH` alone, and each test names its own `PATH`.

use super::core::BIN;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// The built `sdc-watch` of the checkout.
fn watch_binary() -> PathBuf {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../watch/target/debug/sdc-watch");
    assert!(p.is_file(), "{} is not built: `cd watch && cargo build --locked` (task test:runbook does it before the tests)", p.display());
    p.canonicalize().unwrap()
}

struct Rig {
    root: PathBuf,
}

impl Rig {
    fn new(tag: &str) -> Rig {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("watch-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("runs")).unwrap();
        copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine-runbook"), &root.join("fx"));
        std::fs::write(root.join("idea.yaml"), "text: a note on tea\n").unwrap();
        let settings = json!({"in_process": true, "confinement": "local", "decider": {"kind": "scripted", "max_call_micro_usd": 0, "answers": [{"approve": true, "cost_micro_usd": 0}]}});
        std::fs::write(root.join("settings.toml"), super::core::toml_text(&settings)).unwrap();
        Rig { root }
    }

    /// The writer program of the fixture replaced by `script`.
    fn writer(&self, script: &str) {
        std::fs::write(self.root.join("fx/bindings/tools/draft-write.sh"), script).unwrap();
    }

    fn run_dir(&self) -> PathBuf {
        self.root.join("runs/r1")
    }

    fn args(&self, extra: &[&str]) -> Vec<String> {
        let r = |p: &str| self.root.join(p).display().to_string();
        let mut a: Vec<String> = ["run", "sdc", "--run", "r1", "--runs-dir"].iter().map(|s| (*s).to_owned()).collect();
        a.extend([r("runs"), "--bindings".into(), r("fx/bindings"), "--protocols".into(), r("fx/protocol")]);
        a.extend(["--root-protocol", "engine-demo@1", "--case", "ROOT-1", "--confinement", "local"].map(String::from));
        a.extend(["--intake".into(), r("idea.yaml"), "--settings".into(), r("settings.toml"), "--spend-ledger".into(), r("ledger.json")]);
        a.extend(["--cap-micro-usd", "1000000", "--max-llm-calls", "100", "--max-wall-seconds", "120"].map(String::from));
        a.extend(extra.iter().map(|s| (*s).to_owned()));
        a
    }

    /// The runner with `PATH` = `path`; the assets are the fixture's, so nothing else is looked up.
    fn command(&self, args: &[String], path: &str) -> Command {
        let mut c = Command::new(BIN);
        c.args(args).env_clear().env("PATH", path).env("RUNBOOK_HOME", self.root.join("fx")).stdin(Stdio::null());
        c
    }

    /// A `PATH` with the real watcher first.
    fn path_with_watcher(&self) -> String {
        format!("{}:/usr/bin:/bin", watch_binary().parent().unwrap().display())
    }

    fn run(&self, args: &[String], path: &str) -> Output {
        self.command(args, path).output().unwrap()
    }

    fn changes(&self) -> Vec<Value> {
        let text = std::fs::read_to_string(self.run_dir().join("fs/changes.jsonl")).unwrap_or_else(|e| panic!("no fs/changes.jsonl: {e}"));
        text.lines().map(|l| serde_json::from_str::<Value>(l).unwrap()).filter(|v| v["format"] == "sdc.fs-change/1").collect()
    }

    /// The process id the runner recorded for its watcher.
    fn recorded_pid(&self) -> i32 {
        let record: Value = serde_json::from_str(&std::fs::read_to_string(self.run_dir().join("fs/watcher.json")).unwrap()).unwrap();
        assert_eq!(record["format"], "runbook.watcher/1");
        record["pid"].as_i64().unwrap() as i32
    }
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap_or(-1)
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// Whether the process is running: a zombie has ended and only waits for its parent to read it.
fn alive(pid: i32) -> bool {
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(s) => s.rsplit(") ").next().is_some_and(|rest| !rest.starts_with('Z')),
        Err(_) => false,
    }
}

/// A writer that names the step it runs in: the lines of `rounds.jsonl` (one per finished step)
/// plus one, written into a file whose name carries the number.
const STEP_WRITER: &str = "#!/bin/sh\nn=$(cat ../rounds.jsonl 2>/dev/null | wc -l)\nstep=$((n + 1))\necho \"$step\" > \"at-step-$step.txt\"\necho draft >> draft.txt\n";

#[test]
fn a_file_changed_during_a_step_is_recorded_with_that_step() {
    let rig = Rig::new("step");
    rig.writer(STEP_WRITER);
    let started = Instant::now();
    let o = rig.run(&rig.args(&["--watch"]), &rig.path_with_watcher());
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let changes = rig.changes();
    let marked: Vec<&Value> = changes.iter().filter(|c| c["path"].as_str().is_some_and(|p| p.starts_with("at-step-"))).collect();
    assert!(!marked.is_empty(), "no change of a step marker in {changes:?}");
    for c in &marked {
        let step: u64 = c["path"].as_str().unwrap().trim_start_matches("at-step-").trim_end_matches(".txt").parse().unwrap();
        assert_eq!(c["root"], "work", "{c}");
        assert_eq!(c["op"], "create", "{c}");
        assert_eq!(c["attribution"]["step"], step, "{c}");
        assert_eq!(c["attribution"]["basis"], "step-window", "{c}");
        assert!(c["attribution"]["action"].is_null() && c["attribution"]["case"].is_null(), "no action is known under the Loom runner: {c}");
        assert_eq!(c["after"].as_str().map(str::len), Some(64), "{c}");
    }
    assert!(changes.iter().any(|c| c["path"] == "draft.txt"), "the writer's own product is recorded");
    assert!(changes.iter().all(|c| c["attribution"]["step"].as_u64().is_some_and(|s| s >= 1)), "{changes:?}");
    // what the runner writes to keep its run is not product
    let paths: Vec<&str> = changes.iter().map(|c| c["path"].as_str().unwrap()).collect();
    assert!(!paths.iter().any(|p| p.contains("rounds") || p.contains("events.jsonl") || p.contains("runlog")), "{paths:?}");
    eprintln!("watched run took {:?}", started.elapsed());
}

#[test]
fn the_watcher_is_gone_when_the_run_has_ended_whatever_the_end() {
    // closed
    let rig = Rig::new("end-closed");
    let o = rig.run(&rig.args(&["--watch"]), &rig.path_with_watcher());
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let pid = rig.recorded_pid();
    assert!(!alive(pid), "sdc-watch {pid} was left after a closed run");
    assert!(!rig.run_dir().join("fs/sdc-watch.pid").exists(), "it did not stop as on SIGTERM (its pid file is still there)");
    let log = std::fs::read_to_string(rig.run_dir().join("fs/watch.log")).unwrap();
    assert!(log.contains("stopped;"), "{log}");
    assert!(rig.run_dir().join("fs/changes.jsonl").is_file());

    // stopped by a cap, an end that is not a close
    let rig = Rig::new("end-cap");
    let mut args = rig.args(&["--watch"]);
    let i = args.iter().position(|a| a == "--max-llm-calls").unwrap();
    args[i + 1] = "0".into();
    let o = rig.run(&args, &rig.path_with_watcher());
    assert_eq!(code(&o), 6, "{}", stderr(&o));
    assert!(!alive(rig.recorded_pid()), "sdc-watch was left after a stopped run");

    // an error: the helper ends at its start (the run is not started and says why)
    let rig = Rig::new("end-error");
    let stand_in = rig.root.join("bin");
    std::fs::create_dir_all(&stand_in).unwrap();
    std::fs::write(stand_in.join("sdc-watch"), "#!/bin/sh\necho 'inotify: no watches left' >&2\nexit 3\n").unwrap();
    std::fs::set_permissions(stand_in.join("sdc-watch"), std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let o = rig.run(&rig.args(&["--watch"]), &format!("{}:/usr/bin:/bin", stand_in.display()));
    assert_ne!(code(&o), 0, "{}", stderr(&o));
    assert!(stderr(&o).contains("sdc-watch ended at its start") && stderr(&o).contains("no watches left"), "{}", stderr(&o));
    let outcome: Value = serde_json::from_str(&std::fs::read_to_string(rig.run_dir().join("outcome.json")).unwrap()).unwrap();
    assert_eq!(outcome["outcome"], "error", "{outcome}");
}

#[test]
fn a_signal_ends_the_run_and_the_watcher_with_it() {
    let rig = Rig::new("signal");
    rig.writer("#!/bin/sh\necho started > started.txt\nsleep 2\n");
    let mut child = rig.command(&rig.args(&["--watch"]), &rig.path_with_watcher()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let started = rig.run_dir().join("work/started.txt");
    let limit = Instant::now() + Duration::from_secs(40);
    while !started.exists() {
        assert!(Instant::now() < limit, "the writer did not start");
        std::thread::sleep(Duration::from_millis(20));
    }
    let pid = rig.recorded_pid();
    assert!(alive(pid));
    unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
    let status = child.wait().unwrap();
    assert_eq!(status.code(), Some(7), "stopped by a signal: {status:?}");
    assert!(!alive(pid), "sdc-watch {pid} was left after a signal");
    assert!(!rig.changes().is_empty());
}

#[test]
fn a_killed_runner_leaves_no_watcher() {
    let rig = Rig::new("killed");
    rig.writer("#!/bin/sh\necho started > started.txt\nexec sleep 2\n");
    let mut child = rig.command(&rig.args(&["--watch"]), &rig.path_with_watcher()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let started = rig.run_dir().join("work/started.txt");
    let limit = Instant::now() + Duration::from_secs(40);
    while !started.exists() {
        assert!(Instant::now() < limit, "the writer did not start");
        std::thread::sleep(Duration::from_millis(20));
    }
    let pid = rig.recorded_pid();
    assert!(alive(pid));
    unsafe { libc::kill(child.id() as i32, libc::SIGKILL) };
    child.wait().unwrap();
    let t = Instant::now();
    while alive(pid) {
        assert!(t.elapsed() < Duration::from_secs(3), "sdc-watch {pid} outlived its killed runner");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn without_watch_no_watcher_starts_and_nothing_of_it_is_written() {
    let rig = Rig::new("off");
    // the helper is on PATH and is not used
    let o = rig.run(&rig.args(&[]), &rig.path_with_watcher());
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(!rig.run_dir().join("fs").exists(), "fs/ was made without --watch");
}

#[test]
fn watch_without_the_helper_is_refused_before_anything_is_written() {
    let rig = Rig::new("refused");
    let o = rig.run(&rig.args(&["--watch"]), "/usr/bin:/bin");
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(stderr(&o).contains("--watch needs the helper sdc-watch"), "{}", stderr(&o));
    assert!(!rig.run_dir().exists() && !rig.root.join("ledger.json").exists(), "something was written");
}
