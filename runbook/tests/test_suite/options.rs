//! `runbook test --cycle-timeout <seconds>` and `--list` (Loom's `SuiteSettings::cycle_timeout`,
//! `DifferenceReason::CycleTimedOut`, `suite::list_suite`, `suite::slowest_line`). No model, no
//! network; the bound programs are POSIX sh.

use super::{err, lines, out, Rig};
use std::time::{Duration, Instant};

#[test]
fn help_names_the_cycle_timeout_its_range_and_the_listing() {
    let rig = Rig::new("options-help");
    let o = rig.run(&["--help"]);
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    let help = out(&o);
    assert!(help.contains("--cycle-timeout <SECONDS>"), "{help}");
    assert!(help.contains("1 to 60") && help.contains("CycleTimedOut"), "{help}");
    assert!(help.contains("[default: 60]"), "{help}");
    assert!(help.contains("--list"), "{help}");
    assert!(help.contains("waits for story:<id>"), "{help}");
}

#[test]
fn a_cycle_timeout_outside_one_to_sixty_is_refused_before_the_suite_is_read() {
    let rig = Rig::new("options-range");
    let scratch = rig.scratch();
    let missing = rig.home().join("suites/missing.suite.yaml");
    for bad in ["0", "61", "3600"] {
        let o = rig.run(&[
            missing.to_str().unwrap(),
            "--cycle-timeout",
            bad,
            "--scratch",
            scratch.to_str().unwrap(),
            "--confinement",
            "local",
        ]);
        assert_eq!(o.status.code(), Some(2), "--cycle-timeout {bad}: {}\n{}", out(&o), err(&o));
        assert!(
            err(&o).contains(&format!("--cycle-timeout {bad} is outside 1 to 60 seconds"))
                && !err(&o).contains("SuiteUnreadable"),
            "{}",
            err(&o)
        );
        assert_eq!(out(&o), "");
        assert!(!scratch.exists(), "nothing is written");
    }
}

#[test]
fn a_cycle_past_its_timeout_ends_cycle_timed_out_and_the_next_cycle_runs() {
    let rig = Rig::new("options-timeout");
    let scratch = rig.scratch();
    let started = Instant::now();
    let o = rig.run(&[
        rig.suite("timeout.suite.yaml").to_str().unwrap(),
        "--cycle-timeout",
        "1",
        "--scratch",
        scratch.to_str().unwrap(),
        "--confinement",
        "local",
    ]);
    assert_eq!(o.status.code(), Some(1), "{}\n{}", out(&o), err(&o));
    assert!(
        started.elapsed() < Duration::from_secs(15),
        "the program of the slow cycle was killed at the timeout: {:?}",
        started.elapsed()
    );
    let l = lines(&o);
    assert!(
        l[0].starts_with("sleeps-long: FAILED:") && l[0].contains("CycleTimedOut"),
        "{l:?}"
    );
    assert!(
        l[1].starts_with("writes-a-note: "),
        "the next cycle ran (it may end past a bound of 1 s on a loaded host): {l:?}"
    );
    assert!(l[0].contains("expected an end within 1 s"), "{l:?}");
    assert!(l.iter().any(|x| x.starts_with("slowest: ")), "{l:?}");
    assert!(l.iter().any(|x| x.starts_with("failed: sleeps-long")), "{l:?}");
}

#[test]
fn list_prints_a_line_per_cycle_and_runs_nothing() {
    let rig = Rig::new("options-list");
    let scratch = rig.scratch();
    let suite = rig.suite("good.suite.yaml");
    let o = rig.run(&[suite.to_str().unwrap(), "--list", "--scratch", scratch.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(0), "no --confinement is needed to list: {}\n{}", out(&o), err(&o));
    assert_eq!(lines(&o), ["writes-a-note", "reads-a-seed", "pauses", "later: pending"], "{}", out(&o));
    assert!(!scratch.exists(), "nothing is written");
    let o = rig.run(&[suite.to_str().unwrap(), "--list", "--only", "later,pauses"]);
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert_eq!(lines(&o), ["pauses", "later: pending"], "--only applies, in suite order: {}", out(&o));
    let o = rig.run(&[suite.to_str().unwrap(), "--list", "--only", "no-such-cycle"]);
    assert_eq!(o.status.code(), Some(2), "{}", err(&o));
    assert!(err(&o).contains("CycleUnknown"), "{}", err(&o));
    let o = rig.run(&[rig.suite("refused.suite.yaml").to_str().unwrap(), "--list"]);
    assert_eq!(o.status.code(), Some(2), "the listing reads the suite and refuses what Loom refuses: {}\n{}", out(&o), err(&o));
    assert!(err(&o).contains("StepKindUnknown"), "{}", err(&o));
    assert_eq!(out(&o), "");
}
