//! The state of run 1 of record `parity-loom-strong-4`, in small (`suite.notcalled@1`, hand-written
//! sh programs, no model): a check refuses the specification, the writer is due again through its
//! `retry_when`, finds a finding that waits for a person and ends with the not-called code (79,
//! `exits.not_called` of its binding), and the case is handed to the person (`handoff_when`).
//! Loom `db2726ff` took the writer again at once, about twice a second, until its 60 s bound: it
//! counts no run of a not-called exit, and the writer stayed due through its retry records. Loom
//! `c9b4c177` leaves the writer out of the due set after a not-called exit until the case is
//! evaluated again for a reason other than its own moves, and the run waits for the person.

use super::{err, lines, out, Rig};
use serde_json::Value;
use std::path::Path;
use std::time::{Duration, Instant};

const NOT_CALLED: i64 = 79;

/// The run log lines of the root case, and the observations of its store.
fn store(scratch: &Path) -> (Vec<Value>, Vec<Value>) {
    let store = scratch.join("store/1");
    let log = std::fs::read_dir(&store)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("runlog-case-"))
        })
        .expect("a run log in the store");
    let lines = std::fs::read_to_string(log)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let observations: Value =
        serde_json::from_str(&std::fs::read_to_string(store.join("observations.json")).unwrap())
            .unwrap();
    (lines, observations.as_array().cloned().unwrap_or_default())
}

#[test]
fn a_writer_that_is_not_called_while_a_finding_waits_is_started_once_per_evaluation_counts_no_run_and_the_run_waits_for_the_person(
) {
    let rig = Rig::new("notcalled");
    let scratch = rig.scratch();
    let started = Instant::now();
    let o = rig.run(&[
        rig.suite("notcalled.suite.yaml").to_str().unwrap(),
        "--scratch",
        scratch.to_str().unwrap(),
        "--confinement",
        "local",
    ]);
    let elapsed = started.elapsed();
    let order = std::fs::read_to_string(scratch.join("work/1/order.log")).unwrap_or_default();
    let starts = order.lines().filter(|s| *s == "author").count();
    assert_eq!(
        o.status.code(),
        Some(0),
        "the run waits for the person (exit 3 of the cycle); a run that ends Stuck (4) or on the cycle's time bound (5) is a failed cycle here; the writer was started {starts} times in {elapsed:?}: {}\n{}",
        out(&o),
        err(&o)
    );
    let l = lines(&o);
    assert!(
        l[0].starts_with("not-called-while-a-finding-waits: as expected ("),
        "{l:?}"
    );

    let steps: Vec<&str> = order.lines().collect();
    assert_eq!(
        steps.iter().filter(|s| **s == "author").count(),
        1,
        "the writer is started once: {steps:?}"
    );
    assert_eq!(&steps[..2], ["check", "author"], "{steps:?}");
    assert!(
        steps[2..].contains(&"wait"),
        "the handoff check asked for the finding after the writer: {steps:?}"
    );
    assert!(
        !steps.contains(&"amend"),
        "the amend step is handed to the person, not run: {steps:?}"
    );

    let (log, observations) = store(&scratch);
    // `tree run` writes no `ActionStarted` line: each start of a program ends in one `ActionFinished`.
    let started_of = |action: &str| {
        log.iter()
            .filter(|l| l["kind"] == "ActionFinished" && l["action"] == action)
            .count()
    };
    assert_eq!(
        started_of("spec.author"),
        1,
        "one start of the writer for the one evaluation: {log:?}"
    );
    assert_eq!(started_of("spec.check"), 1, "{log:?}");

    let runs: Vec<(i64, Option<String>)> = observations
        .iter()
        .filter(|o| o["source"] == "loom/bound-program" && o["payload"]["action"] == "spec.author")
        .map(|o| {
            (
                o["payload"]["exit"].as_i64().unwrap(),
                o["payload"]["class"].as_str().map(str::to_owned),
            )
        })
        .collect();
    assert_eq!(
        runs,
        vec![(NOT_CALLED, Some("not_called".to_string()))],
        "the not-called exit is no counted run against max_runs (2): {observations:?}"
    );
    let stops: Vec<&str> = log
        .iter()
        .filter(|l| l["kind"] == "Stopped")
        .filter_map(|l| l["stop"].as_str())
        .collect();
    assert_eq!(
        stops,
        ["AwaitingPerson"],
        "the run stops for the person, it does not end Stuck: {log:?}"
    );
    assert!(elapsed < Duration::from_secs(60), "slow: {elapsed:?}");
}
