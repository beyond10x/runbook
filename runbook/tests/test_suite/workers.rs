//! `runbook test --workers N`: cycles beside each other (Loom's `SuiteSettings.workers`). No model, no
//! network; the bound programs are POSIX sh. `meet.suite.yaml` has two cycles that each wait for the
//! other's marker file, so both end as expected only when they run at the same time.

use super::{err, lines, out, Rig};

#[test]
fn help_names_workers_its_default_and_its_range() {
    let rig = Rig::new("workers-help");
    let o = rig.run(&["--help"]);
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    let help = out(&o);
    assert!(help.contains("--workers <N>"), "{help}");
    assert!(
        help.contains("[default: 1]"),
        "one after another unless asked: {help}"
    );
    assert!(help.contains("1 to 64"), "{help}");
    assert!(help.contains("SubstrateWorkersUnserved"), "{help}");
}

#[test]
fn two_workers_run_two_cycles_beside_each_other_and_print_them_in_suite_order() {
    let rig = Rig::new("workers-two");
    let o = rig.run(&[
        rig.suite("meet.suite.yaml").to_str().unwrap(),
        "--workers",
        "2",
        "--confinement",
        "local",
    ]);
    assert_eq!(
        o.status.code(),
        Some(0),
        "each cycle waits for the other, so a pair that ended ran together: {}\n{}",
        out(&o),
        err(&o)
    );
    let l = lines(&o);
    assert!(l[0].starts_with("meet-first: as expected ("), "{l:?}");
    assert!(l[1].starts_with("meet-second: as expected ("), "{l:?}");
    assert_eq!(l.last().unwrap(), "2 of 2 as expected, 0 pending", "{l:?}");
}

#[test]
fn more_than_one_worker_under_substrate_is_refused_as_loom_gives_it_before_anything_is_written() {
    let rig = Rig::new("workers-substrate");
    let scratch = rig.scratch();
    let suite = rig.suite("meet.suite.yaml");
    let o = rig.run(&[
        suite.to_str().unwrap(),
        "--workers",
        "2",
        "--scratch",
        scratch.to_str().unwrap(),
        "--confinement",
        "substrate",
    ]);
    assert_eq!(o.status.code(), Some(2), "{}\n{}", out(&o), err(&o));
    assert!(
        err(&o).contains("SubstrateWorkersUnserved")
            && err(&o).contains("story:suite-runs-cycles-under-one-cgroup-root"),
        "{}",
        err(&o)
    );
    assert_eq!(out(&o), "");
    assert!(!scratch.exists(), "nothing is written");
    let o = rig.run(&[
        suite.to_str().unwrap(),
        "--workers",
        "1",
        "--scratch",
        scratch.to_str().unwrap(),
        "--confinement",
        "substrate",
    ]);
    assert_eq!(
        o.status.code(),
        Some(2),
        "one worker under substrate reaches the level's own refusal: {}",
        err(&o)
    );
    assert!(
        err(&o).contains("CgroupUndelegated") && !err(&o).contains("SubstrateWorkersUnserved"),
        "{}",
        err(&o)
    );
}

#[test]
fn a_count_outside_one_to_sixty_four_is_a_usage_error() {
    let rig = Rig::new("workers-range");
    let suite = rig.suite("meet.suite.yaml");
    for bad in ["0", "65", "many", "-1"] {
        let o = rig.run(&[
            suite.to_str().unwrap(),
            &format!("--workers={bad}"),
            "--confinement",
            "local",
        ]);
        assert_eq!(
            o.status.code(),
            Some(64),
            "--workers {bad}: {}\n{}",
            out(&o),
            err(&o)
        );
        assert!(err(&o).contains("--workers"), "{}", err(&o));
        assert_eq!(out(&o), "", "no cycle ran for --workers {bad}");
    }
}
