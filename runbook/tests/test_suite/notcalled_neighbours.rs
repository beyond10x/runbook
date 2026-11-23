//! The neighbours of the not-called fix (`notcalled.rs` holds the fix itself). Each suite
//! (`suites/notcalled-*.suite.yaml`) is one cycle with sh programs and invented text. A cycle that does not
//! end in a named outcome ends on the cycle's time bound (`--cycle-timeout 8`, `CycleTimedOut`), which
//! is the failure here: a program that is not called must not spin until a bound.

use super::{err, lines, out, Rig};
use std::time::{Duration, Instant};

struct Ran {
    line: String,
    order: Vec<String>,
    elapsed: Duration,
    report: String,
}

fn run(tag: &str, suite: &str) -> Ran {
    let rig = Rig::new(tag);
    let scratch = rig.scratch();
    let started = Instant::now();
    let o = rig.run(&[
        rig.suite(suite).to_str().unwrap(),
        "--scratch",
        scratch.to_str().unwrap(),
        "--confinement",
        "local",
        "--cycle-timeout",
        "8",
    ]);
    let elapsed = started.elapsed();
    let order = std::fs::read_to_string(scratch.join("work/1/order.log")).unwrap_or_default();
    let report = format!("exit {:?}\n{}\n{}", o.status.code(), out(&o), err(&o));
    Ran { line: lines(&o).first().cloned().unwrap_or_default(), order: order.lines().map(str::to_owned).collect(), elapsed, report }
}

fn starts(r: &Ran, who: &str) -> usize {
    r.order.iter().filter(|s| *s == who).count()
}

#[test]
fn a_program_that_exits_79_while_its_binding_does_not_list_it_is_counted_and_the_run_ends_by_name() {
    let r = run("nn-unlisted", "notcalled-unlisted-exit.suite.yaml");
    assert!(!r.report.contains("CycleTimedOut"), "spun until the bound ({} starts in {:?}): {}", starts(&r, "author"), r.elapsed, r.report);
    assert!(starts(&r, "author") <= 2, "an exit the binding does not list is a counted run, `max_runs` is 2: {} starts: {}", starts(&r, "author"), r.report);
    eprintln!("UNLISTED {:?} {}", r.elapsed, r.line);
}

#[test]
fn a_program_that_is_not_called_on_every_evaluation_with_nothing_waiting_ends_by_name() {
    let r = run("nn-every-evaluation", "notcalled-every-evaluation.suite.yaml");
    assert!(!r.report.contains("CycleTimedOut"), "spun until the bound ({} starts in {:?}): {}", starts(&r, "author"), r.elapsed, r.report);
    assert!(starts(&r, "author") <= 3, "{} starts: {}", starts(&r, "author"), r.report);
    eprintln!("ALWAYS79 {:?} {} starts={}", r.elapsed, r.line, starts(&r, "author"));
}

#[test]
fn two_writers_not_called_at_once_are_each_started_once_and_the_run_ends_by_name() {
    let r = run("nn-two-writers", "notcalled-two-writers.suite.yaml");
    assert!(!r.report.contains("CycleTimedOut"), "spun until the bound ({:?}): {}", r.elapsed, r.report);
    assert!(starts(&r, "author") <= 1 && starts(&r, "author2") <= 1, "author {} author2 {}: {}", starts(&r, "author"), starts(&r, "author2"), r.report);
    eprintln!("TWO {:?} {} author={} author2={}", r.elapsed, r.line, starts(&r, "author"), starts(&r, "author2"));
}

#[test]
fn a_writer_that_writes_its_artifact_and_then_exits_not_called_does_not_loop() {
    let r = run("nn-writer-writes", "notcalled-writer-writes.suite.yaml");
    assert!(!r.report.contains("CycleTimedOut"), "spun until the bound ({} starts in {:?}): {}", starts(&r, "author"), r.elapsed, r.report);
    assert!(starts(&r, "author") <= 2, "{} starts: {}", starts(&r, "author"), r.report);
    eprintln!("WRITES79 {:?} {} starts={}", r.elapsed, r.line, starts(&r, "author"));
}

