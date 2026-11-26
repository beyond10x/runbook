//! Adversary cases of wave 6 against `sdc-command` (`docs/loom-bindings.md`, "A command's deadline, retry and poll";
//! story `program-commands-have-a-deadline-and-a-retry`): the deadline is "the time of the whole step", and at the
//! deadline "the try that runs is ended together with every process it started".
//!
//! Each case bounds itself: a helper or a process the case started that is still there when the case looks is killed
//! by its pid before the assertion, so a red case leaves nothing behind.

use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_sdc-command");

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("adversary-w6-command").join(name);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// Whether process `pid` is gone (or a zombie no one has collected yet).
fn gone(pid: i32) -> bool {
    match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Err(_) => true,
        Ok(stat) => stat.rfind(')').and_then(|i| stat[i + 1..].split_whitespace().next().map(|s| s == "Z" || s == "X")).unwrap_or(true),
    }
}

/// Waits up to `within` for `pid` to be gone; a process still there then is killed, and the answer is whether it went
/// by itself.
fn went_by_itself(pid: i32, within: Duration) -> bool {
    let until = Instant::now() + within;
    while !gone(pid) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    let went = gone(pid);
    if !went {
        // SAFETY: a signal to a process this case started; ESRCH is not an error here.
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
    }
    went
}

/// The pid a command of the case wrote to `file`, waited for up to 5 s.
fn pid_in(file: &Path) -> i32 {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(pid) = fs::read_to_string(file).ok().and_then(|s| s.trim().parse::<i32>().ok()) {
            return pid;
        }
        assert!(Instant::now() < until, "{} was not written", file.display());
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Waits up to `within` for `child` to exit; one still running then is killed. The status when it exited by itself.
fn exited_within(child: &mut Child, within: Duration) -> Option<std::process::ExitStatus> {
    let until = Instant::now() + within;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return Some(status);
        }
        if Instant::now() >= until {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Loom starts a bound program as the leader of a process group of its own and, under the confinement `local`, ends it
/// on a cancel (and at its timeout) by killing that group with SIGKILL ("under `local` with its process group only",
/// Loom's `Cancel`). The helper starts each try in a group of its own (`process_group(0)` in `run_one`), so the kill
/// that ends the step does not reach the command: it runs on with no parent and no deadline.
#[test]
fn a_try_ends_when_the_runner_kills_the_helpers_process_group() {
    let d = scratch("group-kill");
    let mut helper = Command::new(BIN)
        .args(["--deadline", "20s", "--", "sh", "-c", "echo $$ > command.pid; exec sleep 20"])
        .current_dir(&d)
        .env_remove("LOOM_TIMEOUT_SECONDS")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .unwrap();
    let command = pid_in(&d.join("command.pid"));
    let group = helper.id() as libc::pid_t;
    // SAFETY: the group this case made for the helper.
    unsafe {
        libc::kill(-group, libc::SIGKILL);
    }
    let _ = helper.wait();
    assert!(
        went_by_itself(command, Duration::from_secs(2)),
        "the command of the try ({command}) outlived the kill of the helper's process group, the way Loom ends a step under `local`"
    );
}

/// The story: "a command deadline longer than the time left of its action is refused by name, never extended". A helper
/// under `LOOM_TIMEOUT_SECONDS` that names no `--deadline` has none at all: it retries on past the step's time, so the
/// runner's timeout ends it (and, by the case above, not its try) and no report of its tries is written.
#[test]
fn under_a_step_limit_a_helper_with_no_deadline_ends_within_the_steps_time() {
    let d = scratch("no-deadline");
    let mut helper = Command::new(BIN)
        .args(["--retry", "1000", "--retry-delay", "500ms", "--", "sh", "-c", "exit 1"])
        .current_dir(&d)
        .env("LOOM_TIMEOUT_SECONDS", "3")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let status = exited_within(&mut helper, Duration::from_secs(5));
    let mut out = String::new();
    if let Some(mut stdout) = helper.stdout.take() {
        use std::io::Read;
        let _ = stdout.read_to_string(&mut out);
    }
    let status = status.unwrap_or_else(|| panic!("the helper ran past its step's 3 s (LOOM_TIMEOUT_SECONDS) with no deadline of its own and was killed at 5 s"));
    let report: Value = serde_json::from_str(out.trim()).unwrap_or_else(|e| panic!("no report ({e}): {out}"));
    assert!(matches!(status.code(), Some(64 | 124)), "exit {status:?}, report {report}");
}

/// `docs/loom-bindings.md`: at the deadline "the try that runs is ended together with every process it started". A
/// process the command started that made a session of its own (`setsid`, as a daemonizing tool does) is not in the
/// try's process group, and the helper ends nothing else.
#[test]
fn a_process_the_command_started_in_a_session_of_its_own_is_ended_at_the_deadline() {
    let d = scratch("setsid");
    let mut helper = Command::new(BIN)
        .args(["--deadline", "1s", "--", "sh", "-c", "setsid sleep 20 & echo $! > started.pid; sleep 20"])
        .current_dir(&d)
        .env_remove("LOOM_TIMEOUT_SECONDS")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let started = pid_in(&d.join("started.pid"));
    let status = exited_within(&mut helper, Duration::from_secs(5));
    assert_eq!(status.and_then(|s| s.code()), Some(124), "the helper ends at its deadline");
    assert!(
        went_by_itself(started, Duration::from_secs(2)),
        "a process the command started ({started}) outlived the deadline that ended the try"
    );
}

/// `--deadline` is "the time of the whole step: every try and every wait between". The helper reads its standard input
/// to its end before the deadline starts, so a standard input that stays open holds it with no bound.
#[test]
fn a_standard_input_that_stays_open_does_not_hold_the_helper_past_its_deadline() {
    let d = scratch("stdin-open");
    let mut helper = Command::new(BIN)
        .args(["--deadline", "1s", "--", "true"])
        .current_dir(&d)
        .env_remove("LOOM_TIMEOUT_SECONDS")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let held_open = helper.stdin.take();
    let status = exited_within(&mut helper, Duration::from_secs(4));
    drop(held_open);
    assert!(status.is_some(), "the helper with --deadline 1s still ran 4 s after its start: it waits for the end of its standard input first");
}
