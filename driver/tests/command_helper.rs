//! `sdc-command`, the helper a bound program runs one command through (`docs/loom-bindings.md`,
//! "A command's deadline, retry and poll"): a deadline that ends the command and what it started
//! (exit 124), a retry with its delay and the exits it retries, a poll that maps exit codes to
//! states with a guard before each try, and one report on standard output that holds every try.
//! The commands are POSIX sh stand-ins; times are fractions of a second to two seconds.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_sdc-command");

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("command-helper").join(name);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// Runs the helper in `dir` with `args`, with no `LOOM_TIMEOUT_SECONDS` unless `env` names it.
fn helper(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> (Output, Duration) {
    let mut c = Command::new(BIN);
    c.args(args).current_dir(dir).env_remove("LOOM_TIMEOUT_SECONDS").stdin(Stdio::null());
    for (k, v) in env {
        c.env(k, v);
    }
    let start = Instant::now();
    let o = c.output().unwrap();
    (o, start.elapsed())
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap_or(-1)
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// The one JSON object the helper prints on standard output.
fn report(o: &Output) -> Value {
    serde_json::from_slice(&o.stdout).unwrap_or_else(|e| panic!("standard output is no one JSON object ({e}):\n{}\nstderr:\n{}", String::from_utf8_lossy(&o.stdout), err(o)))
}

fn tries(r: &Value) -> Vec<Value> {
    r["tries"].as_array().unwrap_or_else(|| panic!("no tries in {r}")).clone()
}

fn exits(r: &Value) -> Vec<Value> {
    tries(r).iter().map(|t| t["exit"].clone()).collect()
}

fn states(r: &Value) -> Vec<String> {
    tries(r).iter().map(|t| t["state"].as_str().unwrap_or("?").to_owned()).collect()
}

/// A sh command that counts its tries in `count` and exits with the n-th code of `codes` (the last
/// one from then on).
fn counted(codes: &[i32]) -> String {
    let list = codes.iter().map(i32::to_string).collect::<Vec<_>>().join(" ");
    format!("n=$(cat count 2>/dev/null || echo 0); n=$((n + 1)); echo $n > count; set -- {list}; i=1; c=$1; for x in \"$@\"; do [ $i -le $n ] && c=$x; i=$((i + 1)); done; echo \"try $n\"; exit $c")
}

/// Whether process `pid` is gone (or a zombie no one has collected yet).
fn gone(pid: u32) -> bool {
    match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Err(_) => true,
        Ok(stat) => stat.rfind(')').and_then(|i| stat[i + 1..].split_whitespace().next().map(|s| s == "Z" || s == "X")).unwrap_or(true),
    }
}

#[test]
fn a_command_past_its_deadline_is_ended_with_what_it_started_and_the_helper_exits_124() {
    let d = scratch("deadline");
    let (o, took) = helper(&d, &["--deadline", "1s", "--", "sh", "-c", "sleep 30 & echo $! > child.pid; sleep 30"], &[]);
    assert_eq!(code(&o), 124, "{}", err(&o));
    assert!(took < Duration::from_secs(4), "the helper ended {took:?} after its start, not at its 1 s deadline");
    let child: u32 = fs::read_to_string(d.join("child.pid")).unwrap().trim().parse().unwrap();
    let until = Instant::now() + Duration::from_secs(2);
    while !gone(child) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(gone(child), "the process the command started ({child}) outlived the deadline");
    let r = report(&o);
    assert_eq!(r["outcome"], "deadline", "{r}");
    assert_eq!(r["exit"], 124, "{r}");
    assert_eq!(r["deadline_s"], 1, "{r}");
    assert_eq!(r["text"], "deadline after 1 s, last state none", "{r}");
    let t = tries(&r);
    assert_eq!(t.len(), 1, "{r}");
    assert_eq!(t[0]["try"], 1, "{r}");
    assert_eq!(t[0]["deadline"], true, "{r}");
    assert_eq!(t[0]["state"], "deadline", "{r}");
}

#[test]
fn a_command_that_fails_twice_then_passes_with_retry_2_exits_0_with_three_tries_recorded() {
    let d = scratch("retry");
    let script = counted(&[1, 1, 0]);
    let (o, _) = helper(&d, &["--retry", "2", "--", "sh", "-c", &script], &[]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let r = report(&o);
    assert_eq!(r["outcome"], "done", "{r}");
    assert_eq!(exits(&r), [1, 1, 0], "{r}");
    assert_eq!(states(&r), ["pending", "pending", "done"], "{r}");
    assert!(tries(&r).iter().all(|t| t["deadline"] == false && t["ms"].is_u64() && t["at"].is_u64()), "{r}");
    assert!(err(&o).contains("try 3"), "the command's output goes to standard error: {}", err(&o));
}

#[test]
fn with_retry_on_7_an_exit_1_is_not_retried_and_an_exit_7_is() {
    let d = scratch("retry-on");
    let (o, _) = helper(&d, &["--retry", "2", "--retry-on", "7", "--", "sh", "-c", &counted(&[1, 0])], &[]);
    assert_eq!(code(&o), 1, "{}", err(&o));
    let r = report(&o);
    assert_eq!(exits(&r), [1], "{r}");
    assert_eq!(r["outcome"], "failed", "{r}");

    let d = scratch("retry-on-7");
    let (o, _) = helper(&d, &["--retry", "2", "--retry-on", "7", "--retry-delay", "50ms", "--", "sh", "-c", &counted(&[7, 0])], &[]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert_eq!(exits(&report(&o)), [7, 0]);
}

#[test]
fn retries_spent_end_with_the_last_exit_and_say_so() {
    let d = scratch("spent");
    let (o, _) = helper(&d, &["--retry", "1", "--", "sh", "-c", "exit 3"], &[]);
    assert_eq!(code(&o), 3, "{}", err(&o));
    let r = report(&o);
    assert_eq!(r["outcome"], "failed", "{r}");
    assert_eq!(r["text"], "retries spent after 2 tries, last state pending", "{r}");
}

#[test]
fn a_deadline_longer_than_the_time_the_step_has_left_is_refused_by_name() {
    let d = scratch("refused");
    let (o, _) = helper(&d, &["--deadline", "10s", "--", "sh", "-c", "touch ran"], &[("LOOM_TIMEOUT_SECONDS", "5")]);
    assert_eq!(code(&o), 64, "{}", err(&o));
    assert!(err(&o).contains("--deadline 10s") && err(&o).contains("LOOM_TIMEOUT_SECONDS"), "{}", err(&o));
    assert_eq!(report(&o)["outcome"], "refused");
    assert!(!d.join("ran").exists(), "a refused command ran");

    let (o, _) = helper(&d, &["--deadline", "3s", "--", "sh", "-c", "touch ran"], &[("LOOM_TIMEOUT_SECONDS", "5")]);
    assert_eq!(code(&o), 0, "a deadline that leaves the step 2 s to end runs: {}", err(&o));
    assert!(d.join("ran").exists());
}

#[test]
fn a_poll_takes_8_as_pending_stops_on_1_with_its_name_and_fails_on_any_other_exit() {
    let poll = |codes: &[i32], tag: &str| {
        let d = scratch(tag);
        let script = counted(codes);
        helper(&d, &["--deadline", "5s", "--retry-on", "8", "--retry-delay", "100ms", "--stop", "1=red-checks", "--", "sh", "-c", &script], &[]).0
    };
    let o = poll(&[8, 8, 0], "poll-green");
    assert_eq!(code(&o), 0, "{}", err(&o));
    let r = report(&o);
    assert_eq!((r["outcome"].as_str(), states(&r)), (Some("done"), vec!["pending".to_owned(), "pending".into(), "done".into()]), "{r}");

    let o = poll(&[8, 1], "poll-red");
    assert_eq!(code(&o), 1, "{}", err(&o));
    let r = report(&o);
    assert_eq!((r["outcome"].as_str(), states(&r)), (Some("red-checks"), vec!["pending".to_owned(), "red-checks".into()]), "{r}");

    let o = poll(&[8, 5], "poll-other");
    assert_eq!(code(&o), 5, "{}", err(&o));
    let r = report(&o);
    assert_eq!((r["outcome"].as_str(), states(&r)), (Some("failed"), vec!["pending".to_owned(), "failed".into()]), "{r}");
}

#[test]
fn a_guard_that_fails_stops_the_poll_with_its_name_before_the_command_runs_again() {
    let d = scratch("guard");
    // the command moves the head on its first try and says pending
    let (o, _) = helper(
        &d,
        &["--deadline", "5s", "--retry-on", "8", "--retry-delay", "50ms", "--guard-stop", "head-moved", "--guard", "sh", "-c", "test ! -f moved", "--", "sh", "-c", "echo run >> runs; touch moved; exit 8"],
        &[],
    );
    assert_eq!(code(&o), 125, "{}", err(&o));
    let r = report(&o);
    assert_eq!(r["outcome"], "head-moved", "{r}");
    assert_eq!(states(&r), ["pending", "head-moved"], "{r}");
    assert_eq!(tries(&r)[1]["guard"], 1, "the guard's exit is recorded: {r}");
    assert!(tries(&r)[1]["exit"].is_null(), "the command did not run on the stopped try: {r}");
    assert_eq!(fs::read_to_string(d.join("runs")).unwrap(), "run\n", "the command ran once");
}

#[test]
fn at_the_deadline_a_poll_ends_naming_the_state_of_its_last_try() {
    let d = scratch("poll-deadline");
    let (o, took) = helper(&d, &["--deadline", "1s", "--retry-on", "8", "--retry-delay", "200ms", "--", "sh", "-c", "exit 8"], &[]);
    assert_eq!(code(&o), 124, "{}", err(&o));
    assert!(took >= Duration::from_millis(900) && took < Duration::from_secs(3), "{took:?}");
    let r = report(&o);
    assert_eq!(r["outcome"], "deadline", "{r}");
    assert_eq!(r["text"], "deadline after 1 s, last state pending", "{r}");
    assert!(tries(&r).len() >= 3, "{r}");
}

#[test]
fn the_record_keeps_the_last_tries_up_to_its_bound_and_counts_the_rest() {
    let d = scratch("bound");
    let (o, _) = helper(&d, &["--retry", "9", "--keep-tries", "3", "--", "sh", "-c", &counted(&[1])], &[]);
    assert_eq!(code(&o), 1, "{}", err(&o));
    let r = report(&o);
    assert_eq!(r["tries_total"], 10, "{r}");
    assert_eq!(r["tries_dropped"], 7, "{r}");
    let numbers: Vec<Value> = tries(&r).iter().map(|t| t["try"].clone()).collect();
    assert_eq!(numbers, [8, 9, 10], "{r}");
}

#[test]
fn a_command_line_the_helper_cannot_take_is_refused_with_64_and_runs_nothing() {
    let d = scratch("usage");
    for args in [
        vec!["sh", "-c", "touch ran"],
        vec!["--deadline", "soon", "--", "sh", "-c", "touch ran"],
        vec!["--retry-on", "8", "--", "sh", "-c", "touch ran"],
        vec!["--stop", "0=green", "--deadline", "1s", "--", "sh", "-c", "touch ran"],
        vec!["--stop", "8=x", "--retry-on", "8", "--deadline", "1s", "--", "sh", "-c", "touch ran"],
        vec!["--colour", "--", "sh", "-c", "touch ran"],
    ] {
        let (o, _) = helper(&d, &args, &[]);
        assert_eq!(code(&o), 64, "{args:?}: {}", err(&o));
        assert_eq!(report(&o)["outcome"], "refused", "{args:?}");
        assert!(!d.join("ran").exists(), "{args:?} ran the command");
    }
}

/// The helper's own exits (64 usage, 124 deadline, 125 guard, 127 not started) never stand for a
/// command's exit: a command that exits with one of them makes the helper exit 1, and its try and
/// the report's text keep the command's own code.
#[test]
fn a_command_exit_that_is_one_of_the_helpers_own_is_reported_as_1_and_kept_in_its_try() {
    for own in [64, 124, 125, 127] {
        let d = scratch(&format!("own-exit-{own}"));
        let (o, _) = helper(&d, &["--", "sh", "-c", &format!("exit {own}")], &[]);
        assert_eq!(code(&o), 1, "a command that exits {own}: {}", err(&o));
        let r = report(&o);
        assert_eq!((r["outcome"].as_str(), r["exit"].as_i64()), (Some("failed"), Some(1)), "{r}");
        assert_eq!(exits(&r), [own], "{r}");
        assert!(r["text"].as_str().unwrap_or("").contains(&format!("exited {own}")), "{r}");
    }
    let d = scratch("own-exit-stop");
    let (o, _) = helper(&d, &["--stop", "124=late", "--", "sh", "-c", "exit 124"], &[]);
    assert_eq!(code(&o), 1, "{}", err(&o));
    let r = report(&o);
    assert_eq!((r["outcome"].as_str(), exits(&r)), (Some("late"), vec![Value::from(124)]), "{r}");

    let d = scratch("own-exit-spent");
    let (o, _) = helper(&d, &["--retry", "1", "--", "sh", "-c", "exit 125"], &[]);
    assert_eq!(code(&o), 1, "{}", err(&o));
    assert_eq!(exits(&report(&o)), [125, 125]);
}

/// A guard that cannot be started is its own outcome, `guard-not-started` (exit 127), never the
/// `--guard-stop` name; the command does not run. The name is the helper's own.
#[test]
fn a_guard_that_cannot_be_started_is_its_own_outcome_and_not_the_guard_stop_name() {
    let d = scratch("guard-not-started");
    let (o, _) = helper(&d, &["--guard-stop", "head-moved", "--guard", "./no-such-guard", "--", "sh", "-c", "touch ran"], &[]);
    assert_eq!(code(&o), 127, "{}", err(&o));
    let r = report(&o);
    assert_eq!(r["outcome"], "guard-not-started", "{r}");
    assert_eq!(states(&r), ["guard-not-started"], "{r}");
    assert!(!d.join("ran").exists(), "the command ran after its guard could not start");
    for args in [["--stop", "3=guard-not-started"], ["--guard-stop", "guard-not-started"]] {
        let mut args = args.to_vec();
        args.extend(["--deadline", "1s", "--", "sh", "-c", "touch ran"]);
        let (o, _) = helper(&d, &args, &[]);
        assert_eq!(code(&o), 64, "{args:?}: {}", err(&o));
    }
}

/// Under Loom's step limit a helper with no `--deadline` takes the step's time less the 2 s it
/// keeps to end the command and write its report; a step of 2 s or less leaves it none and is
/// refused by name.
#[test]
fn under_a_step_limit_with_no_deadline_the_deadline_is_the_steps_time_less_2_s() {
    let d = scratch("step-deadline");
    let (o, took) = helper(&d, &["--retry-on", "8", "--retry", "1000", "--retry-delay", "100ms", "--", "sh", "-c", "exit 8"], &[("LOOM_TIMEOUT_SECONDS", "3")]);
    assert_eq!(code(&o), 124, "{}", err(&o));
    assert!(took < Duration::from_secs(3), "{took:?}");
    let r = report(&o);
    assert_eq!((r["deadline_s"].as_u64(), r["text"].as_str()), (Some(1), Some("deadline after 1 s, last state pending")), "{r}");

    let (o, _) = helper(&d, &["--", "sh", "-c", "touch ran"], &[("LOOM_TIMEOUT_SECONDS", "2")]);
    assert_eq!(code(&o), 64, "{}", err(&o));
    assert!(err(&o).contains("LOOM_TIMEOUT_SECONDS"), "{}", err(&o));
    assert!(!d.join("ran").exists(), "a refused command ran");
}
