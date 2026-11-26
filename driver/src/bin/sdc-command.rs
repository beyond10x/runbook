//! `sdc-command`: runs one command of a bound program with a deadline, a retry and a poll, and
//! prints one report of every try (`docs/loom-bindings.md`, "A command's deadline, retry and poll").
//!
//! ```text
//! sdc-command [--deadline <time>] [--retry <n>] [--retry-delay <time>] [--retry-on <codes>]
//!             [--stop <code>=<name>]... [--keep-tries <n>] [--guard-stop <name>]
//!             [--guard <program> [<argument>...]] -- <program> [<argument>...]
//! ```
//!
//! A try runs the guard (when there is one) and then the command, each in a process group of its
//! own; when a try ends, what is left of its group is killed. The exit of the command gives the
//! try a state: `done` (0), `pending` (an exit of `--retry-on`, or any other than 0 when `--retry`
//! is given without `--retry-on`), the name an exit has in `--stop`, else `failed`. A pending try
//! is followed by another after `--retry-delay`, while the retries and the deadline allow.
//!
//! At the deadline the try that runs is ended (SIGTERM to its group, SIGKILL after 1 s) and the
//! helper exits 124. Standard output is one JSON object, the report (`runbook.command/1`): the
//! outcome, its text, the exit, and the last `--keep-tries` tries with the count of the others.
//! What the command and the guard print goes to standard error. Under Loom's program contract the
//! standard input (Loom's argument object) is read once and given to every try.
//!
//! Exits: 0 done; the command's own exit for a stop, a failure or spent retries; 124 the
//! deadline; 125 the guard stopped the step; 127 the command could not be started; 64 a command
//! line this helper refuses (nothing runs).

use serde_json::{json, Value};
use std::io::{Read, Write};
use std::os::fd::AsFd;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const FORMAT: &str = "runbook.command/1";
const EXIT_DEADLINE: i32 = 124;
const EXIT_GUARD: i32 = 125;
const EXIT_NOT_STARTED: i32 = 127;
const EXIT_USAGE: i32 = 64;
/// How long a try's group has to end after SIGTERM before SIGKILL.
const END_GRACE: Duration = Duration::from_secs(1);
/// What the step keeps of its own time after the deadline: the grace, then the report.
const STEP_MARGIN: Duration = Duration::from_secs(2);
const KEEP_TRIES: usize = 100;
const KEEP_TRIES_MAX: usize = 1000;
const STDIN_LIMIT: u64 = 1 << 20;
const POLL_STEP: Duration = Duration::from_millis(10);
/// State names the helper gives itself; a `--stop` or `--guard-stop` name is none of them.
const OWN_STATES: [&str; 6] = ["done", "pending", "failed", "deadline", "refused", "none"];

#[derive(Debug, Default)]
struct Options {
    deadline: Option<Duration>,
    deadline_text: String,
    retry: Option<u64>,
    retry_delay: Duration,
    retry_on: Option<Vec<i32>>,
    stops: Vec<(i32, String)>,
    keep_tries: usize,
    guard: Vec<String>,
    guard_stop: String,
    command: Vec<String>,
}

fn main() {
    sdc_tools::answer_version("sdc-command");
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--help" || a == "-h") {
        print!("{}", usage());
        return;
    }
    let started = Instant::now();
    let options = match parse(&args).and_then(|o| fits_the_step(o, std::env::var("LOOM_TIMEOUT_SECONDS").ok().as_deref())) {
        Ok(o) => o,
        Err(why) => {
            eprintln!("sdc-command: {why}");
            eprint!("{}", usage());
            finish(&Report { outcome: "refused".into(), text: why, exit: EXIT_USAGE, ..Report::default() }, started, None);
        }
    };
    let input = standard_input();
    let report = run(&options, input.as_deref());
    finish(&report, started, Some(&options));
}

fn usage() -> String {
    "usage: sdc-command [--deadline <time>] [--retry <n>] [--retry-delay <time>] [--retry-on <code>[,<code>...]]\n\
     \x20                  [--stop <code>=<name>]... [--keep-tries <n>] [--guard-stop <name>]\n\
     \x20                  [--guard <program> [<argument>...]] -- <program> [<argument>...]\n\
     a time is <n>ms, <n>s, <n>m or <n>h (a plain number is seconds)\n"
        .to_owned()
}

// ------------------------------------------------------------------------------- command line

fn parse(args: &[String]) -> Result<Options, String> {
    let mut o = Options { keep_tries: KEEP_TRIES, guard_stop: "guard-stopped".into(), ..Options::default() };
    let mut i = 0;
    let value = |i: usize, name: &str| args.get(i + 1).cloned().ok_or_else(|| format!("{name} needs a value"));
    loop {
        let Some(arg) = args.get(i) else { return Err("no `--` before the command".into()) };
        match arg.as_str() {
            "--" => {
                o.command = args[i + 1..].to_vec();
                break;
            }
            "--deadline" => {
                let v = value(i, arg)?;
                let d = time(&v).map_err(|why| format!("--deadline {v}: {why}"))?;
                if d.is_zero() {
                    return Err(format!("--deadline {v}: a deadline is longer than 0"));
                }
                o.deadline = Some(d);
                o.deadline_text = v;
                i += 2;
            }
            "--retry" => {
                let v = value(i, arg)?;
                o.retry = Some(v.parse::<u64>().map_err(|_| format!("--retry {v}: a whole number of retries from 0"))?);
                i += 2;
            }
            "--retry-delay" => {
                let v = value(i, arg)?;
                o.retry_delay = time(&v).map_err(|why| format!("--retry-delay {v}: {why}"))?;
                i += 2;
            }
            "--retry-on" => {
                let v = value(i, arg)?;
                let mut codes = Vec::new();
                for c in v.split(',') {
                    codes.push(exit_code(c).map_err(|why| format!("--retry-on {v}: {why}"))?);
                }
                o.retry_on = Some(codes);
                i += 2;
            }
            "--stop" => {
                let v = value(i, arg)?;
                let (c, name) = v.split_once('=').ok_or_else(|| format!("--stop {v}: <code>=<name>"))?;
                let c = exit_code(c).map_err(|why| format!("--stop {v}: {why}"))?;
                name_ok(name).map_err(|why| format!("--stop {v}: {why}"))?;
                if o.stops.iter().any(|(s, _)| *s == c) {
                    return Err(format!("--stop {v}: the exit {c} is named twice"));
                }
                o.stops.push((c, name.to_owned()));
                i += 2;
            }
            "--keep-tries" => {
                let v = value(i, arg)?;
                o.keep_tries = v.parse::<usize>().ok().filter(|n| (1..=KEEP_TRIES_MAX).contains(n)).ok_or_else(|| format!("--keep-tries {v}: a whole number from 1 to {KEEP_TRIES_MAX}"))?;
                i += 2;
            }
            "--guard-stop" => {
                let v = value(i, arg)?;
                name_ok(&v).map_err(|why| format!("--guard-stop {v}: {why}"))?;
                o.guard_stop = v;
                i += 2;
            }
            "--guard" => {
                let end = args[i + 1..].iter().position(|a| a == "--").map(|p| i + 1 + p).ok_or("--guard runs up to the `--` before the command; there is none")?;
                o.guard = args[i + 1..end].to_vec();
                if o.guard.is_empty() {
                    return Err("--guard names no program".into());
                }
                i = end;
            }
            other => return Err(format!("{other} is no option of sdc-command")),
        }
    }
    if o.command.is_empty() {
        return Err("no command after `--`".into());
    }
    if let Some(codes) = &o.retry_on {
        if codes.contains(&0) {
            return Err("--retry-on 0: an exit 0 is done, never retried".into());
        }
        if let Some((c, name)) = o.stops.iter().find(|(c, _)| codes.contains(c)) {
            return Err(format!("--stop {c}={name}: the exit {c} is also in --retry-on"));
        }
        if o.retry.is_none() && o.deadline.is_none() {
            return Err("--retry-on without --retry retries until the deadline; name --deadline".into());
        }
    }
    if let Some((_, name)) = o.stops.iter().find(|(c, _)| *c == 0) {
        return Err(format!("--stop 0={name}: an exit 0 is done"));
    }
    Ok(o)
}

/// `<n>ms`, `<n>s`, `<n>m`, `<n>h` or a plain `<n>` (seconds).
fn time(text: &str) -> Result<Duration, String> {
    let (digits, unit) = text.find(|c: char| !c.is_ascii_digit()).map_or((text, ""), |at| text.split_at(at));
    let n: u64 = digits.parse().map_err(|_| "a time is <n>ms, <n>s, <n>m or <n>h".to_owned())?;
    let ms = match unit {
        "ms" => Some(n),
        "" | "s" => n.checked_mul(1000),
        "m" => n.checked_mul(60_000),
        "h" => n.checked_mul(3_600_000),
        _ => return Err("a time is <n>ms, <n>s, <n>m or <n>h".into()),
    };
    ms.map(Duration::from_millis).ok_or_else(|| "too long".into())
}

fn exit_code(text: &str) -> Result<i32, String> {
    match text.parse::<i32>() {
        Ok(c) if (0..=255).contains(&c) && text == c.to_string() => Ok(c),
        _ => Err(format!("{text:?} is no exit code from 0 to 255")),
    }
}

fn name_ok(name: &str) -> Result<(), String> {
    let word = !name.is_empty() && name.len() <= 64 && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_');
    if !word {
        return Err(format!("{name:?} is no name (a-z, 0-9, - and _, at most 64)"));
    }
    if OWN_STATES.contains(&name) {
        return Err(format!("{name} is a state of the helper itself"));
    }
    Ok(())
}

/// Under Loom (`LOOM_TIMEOUT_SECONDS` set) the deadline and the margin to end fit in the step's
/// time: a longer deadline is refused, never cut.
fn fits_the_step(o: Options, timeout: Option<&str>) -> Result<Options, String> {
    let (Some(deadline), Some(timeout)) = (o.deadline, timeout) else { return Ok(o) };
    let Ok(seconds) = timeout.trim().parse::<u64>() else { return Ok(o) };
    let left = Duration::from_secs(seconds);
    if deadline + STEP_MARGIN > left {
        return Err(format!(
            "--deadline {} is longer than this step's {seconds} s (LOOM_TIMEOUT_SECONDS) less the {} s the helper takes to end the command and write its report; name a shorter deadline or raise the binding's timeout_seconds",
            o.deadline_text,
            STEP_MARGIN.as_secs()
        ));
    }
    Ok(o)
}

/// Standard input, read to its end once when it is no terminal (Loom writes its argument object
/// and closes it), at most [`STDIN_LIMIT`] bytes.
fn standard_input() -> Option<Vec<u8>> {
    // SAFETY: isatty only reads the descriptor's state.
    if unsafe { libc::isatty(0) } == 1 {
        return None;
    }
    let mut bytes = Vec::new();
    std::io::stdin().lock().take(STDIN_LIMIT).read_to_end(&mut bytes).ok()?;
    (!bytes.is_empty()).then_some(bytes)
}

// ----------------------------------------------------------------------------------------- run

#[derive(Default)]
struct Report {
    outcome: String,
    text: String,
    exit: i32,
    tries: Vec<Value>,
    total: u64,
}

/// How one program of a try ended.
enum Ended {
    Exited(ExitStatus),
    Deadline,
    NotStarted(String),
}

fn run(o: &Options, input: Option<&[u8]>) -> Report {
    let start = Instant::now();
    let deadline = o.deadline.map(|d| start + d);
    let mut report = Report::default();
    let mut last_state = "none".to_owned();
    let mut retries = 0u64;
    loop {
        report.total += 1;
        let n = report.total;
        let at = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let try_start = Instant::now();
        let mut entry = json!({"try": n, "at": at});
        let keep = |report: &mut Report, entry: Value| {
            report.tries.push(entry);
            if report.tries.len() > o.keep_tries {
                report.tries.remove(0);
            }
        };
        if !o.guard.is_empty() {
            match run_one(&o.guard, input, deadline) {
                Ended::Exited(status) if status.success() => {}
                Ended::Exited(status) => {
                    entry["guard"] = json!(code_of(status));
                    entry["state"] = json!(o.guard_stop);
                    entry["ms"] = json!(ms(try_start.elapsed()));
                    entry["exit"] = Value::Null;
                    entry["deadline"] = json!(false);
                    keep(&mut report, entry);
                    report.outcome = o.guard_stop.clone();
                    report.text = format!("the guard exited {} before try {n}", code_of(status));
                    report.exit = EXIT_GUARD;
                    return report;
                }
                Ended::Deadline => {
                    entry["state"] = json!("deadline");
                    entry["ms"] = json!(ms(try_start.elapsed()));
                    entry["exit"] = Value::Null;
                    entry["deadline"] = json!(true);
                    keep(&mut report, entry);
                    return at_deadline(report, o, &last_state);
                }
                Ended::NotStarted(why) => {
                    entry["state"] = json!(o.guard_stop);
                    entry["ms"] = json!(ms(try_start.elapsed()));
                    entry["exit"] = Value::Null;
                    entry["deadline"] = json!(false);
                    keep(&mut report, entry);
                    report.outcome = o.guard_stop.clone();
                    report.text = format!("the guard could not be started: {why}");
                    report.exit = EXIT_GUARD;
                    return report;
                }
            }
        }
        let ended = run_one(&o.command, input, deadline);
        entry["ms"] = json!(ms(try_start.elapsed()));
        let code = match ended {
            Ended::Deadline => {
                entry["exit"] = Value::Null;
                entry["state"] = json!("deadline");
                entry["deadline"] = json!(true);
                keep(&mut report, entry);
                return at_deadline(report, o, &last_state);
            }
            Ended::NotStarted(why) => {
                entry["exit"] = Value::Null;
                entry["state"] = json!("failed");
                entry["deadline"] = json!(false);
                keep(&mut report, entry);
                report.outcome = "failed".into();
                report.text = format!("the command could not be started: {why}");
                report.exit = EXIT_NOT_STARTED;
                return report;
            }
            Ended::Exited(status) => code_of(status),
        };
        let state = state_of(o, code);
        entry["exit"] = json!(code);
        entry["state"] = json!(state);
        entry["deadline"] = json!(false);
        keep(&mut report, entry);
        last_state = state.clone();
        if state != "pending" {
            report.exit = code;
            report.text = match state.as_str() {
                "done" => format!("done on try {n}"),
                "failed" => format!("try {n} exited {code}"),
                stop => format!("try {n} exited {code}, which stops the step as {stop}"),
            };
            report.outcome = state;
            return report;
        }
        if o.retry.is_some_and(|max| retries >= max) {
            report.outcome = "failed".into();
            report.text = format!("retries spent after {n} tries, last state pending");
            report.exit = code;
            return report;
        }
        retries += 1;
        let wake = Instant::now() + o.retry_delay;
        if let Some(d) = deadline {
            if wake >= d {
                std::thread::sleep(d.saturating_duration_since(Instant::now()));
                return at_deadline(report, o, &last_state);
            }
        }
        std::thread::sleep(o.retry_delay);
    }
}

fn at_deadline(mut report: Report, o: &Options, last_state: &str) -> Report {
    report.outcome = "deadline".into();
    report.text = format!("deadline after {} s, last state {last_state}", seconds(o.deadline.unwrap_or_default()));
    report.exit = EXIT_DEADLINE;
    report
}

fn state_of(o: &Options, code: i32) -> String {
    if code == 0 {
        return "done".into();
    }
    if let Some((_, name)) = o.stops.iter().find(|(c, _)| *c == code) {
        return name.clone();
    }
    let retried = match &o.retry_on {
        Some(codes) => codes.contains(&code),
        None => o.retry.is_some_and(|n| n > 0),
    };
    if retried {
        "pending".into()
    } else {
        "failed".into()
    }
}

fn code_of(status: ExitStatus) -> i32 {
    status.code().unwrap_or_else(|| 128 + status.signal().unwrap_or(0))
}

/// Runs `argv` in a process group of its own, its standard output on this helper's standard error,
/// until it exits or `deadline` comes; then what is left of the group is killed.
fn run_one(argv: &[String], input: Option<&[u8]>, deadline: Option<Instant>) -> Ended {
    let stdout = match std::io::stderr().as_fd().try_clone_to_owned() {
        Ok(fd) => Stdio::from(fd),
        Err(e) => return Ended::NotStarted(format!("no standard error to give it: {e}")),
    };
    let mut c = Command::new(&argv[0]);
    c.args(&argv[1..]).process_group(0).stdout(stdout).stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() });
    let mut child = match c.spawn() {
        Ok(child) => child,
        Err(e) => return Ended::NotStarted(format!("{}: {e}", argv[0])),
    };
    if let (Some(bytes), Some(mut pipe)) = (input, child.stdin.take()) {
        let bytes = bytes.to_vec();
        std::thread::spawn(move || {
            let _ = pipe.write_all(&bytes);
        });
    }
    let group = child.id() as libc::pid_t;
    let ended = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ended::Exited(status),
            Ok(None) => {}
            Err(e) => break Ended::NotStarted(format!("waiting for {}: {e}", argv[0])),
        }
        if deadline.is_some_and(|d| Instant::now() >= d) {
            end_group(group, &mut child);
            break Ended::Deadline;
        }
        std::thread::sleep(POLL_STEP);
    };
    // what the program started and left behind ends with its try
    signal_group(group, libc::SIGKILL);
    ended
}

fn signal_group(group: libc::pid_t, signal: libc::c_int) {
    // SAFETY: a signal to a process group this helper made; ESRCH (none left) is not an error here.
    unsafe {
        libc::kill(-group, signal);
    }
}

/// SIGTERM to the group, SIGKILL after [`END_GRACE`], and the leader collected.
fn end_group(group: libc::pid_t, child: &mut Child) {
    signal_group(group, libc::SIGTERM);
    let until = Instant::now() + END_GRACE;
    while Instant::now() < until {
        if matches!(child.try_wait(), Ok(Some(_))) {
            break;
        }
        std::thread::sleep(POLL_STEP);
    }
    signal_group(group, libc::SIGKILL);
    let _ = child.wait();
}

// -------------------------------------------------------------------------------------- report

fn ms(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// Whole seconds as a number, else seconds with up to three decimals.
fn seconds(d: Duration) -> String {
    if d.subsec_millis() == 0 {
        d.as_secs().to_string()
    } else {
        format!("{:.3}", d.as_secs_f64()).trim_end_matches('0').to_owned()
    }
}

fn seconds_value(d: Duration) -> Value {
    if d.subsec_millis() == 0 {
        json!(d.as_secs())
    } else {
        json!(d.as_secs_f64())
    }
}

fn finish(report: &Report, started: Instant, options: Option<&Options>) -> ! {
    let dropped = report.total.saturating_sub(report.tries.len() as u64);
    let out = json!({
        "format": FORMAT,
        "outcome": report.outcome,
        "text": report.text,
        "exit": report.exit,
        "deadline_s": options.and_then(|o| o.deadline).map_or(Value::Null, seconds_value),
        "elapsed_ms": ms(started.elapsed()),
        "tries_total": report.total,
        "tries_dropped": dropped,
        "tries": report.tries,
    });
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout, "{out}");
    let _ = stdout.flush();
    std::process::exit(report.exit);
}
