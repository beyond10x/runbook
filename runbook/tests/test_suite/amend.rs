//! The state of run 2 of record `parity-loom-strong-4`, in small (`suite.amend@1`, hand-written sh
//! programs, no model): an action (`amend.upstream`) runs, a second action (`requirements.normalise`)
//! writes, one `evaluated` line follows, a check (`requirements.check`) exits non-zero, and the first
//! action's handoff check (`handoff_when`) says there is nothing to wait for. The first amend writes
//! what the seeded questions already hold, so no revision moves and no `evaluated` line follows it
//! (the shape of the run's log: amend, normalise, one `evaluated`); the second amend adds a line.
//! Loom `70b0e6e0` dated an
//! action by the next `evaluated` line, found the amend step done at the revision the check left, and
//! ended the run `Stuck`; Loom `db2726ff` dates it by the revision of its step and the revision the
//! step left, offers it again, and the run goes on.

use super::{err, lines, out, Rig};
use std::time::{Duration, Instant};

#[test]
fn a_check_that_fails_after_a_later_write_takes_the_amend_step_again_and_the_run_does_not_end_stuck(
) {
    let rig = Rig::new("amend");
    let scratch = rig.scratch();
    let started = Instant::now();
    let o = rig.run(&[
        rig.suite("amend.suite.yaml").to_str().unwrap(),
        "--scratch",
        scratch.to_str().unwrap(),
        "--confinement",
        "local",
    ]);
    assert_eq!(
        o.status.code(),
        Some(0),
        "a run that ends Stuck is exit 4 in the suite and a failed cycle here: {}\n{}",
        out(&o),
        err(&o)
    );
    assert!(
        started.elapsed() < Duration::from_secs(60),
        "slow: {:?}",
        started.elapsed()
    );
    let l = lines(&o);
    assert!(
        l[0].starts_with("amend-again-after-a-later-write: as expected ("),
        "{l:?}"
    );
    assert_eq!(l.last().unwrap(), "1 of 1 as expected, 0 pending", "{l:?}");
    let order = std::fs::read_to_string(scratch.join("work/1/order.log")).unwrap();
    let steps: Vec<&str> = order.lines().collect();
    let first_amend = steps
        .iter()
        .position(|s| *s == "amend")
        .expect("the amend step ran");
    let write_after = steps[first_amend + 1..]
        .iter()
        .position(|s| *s == "normalise")
        .map(|p| p + first_amend + 1)
        .expect("a later step wrote");
    let failed_check = steps[write_after + 1..]
        .iter()
        .position(|s| *s == "check")
        .map(|p| p + write_after + 1)
        .expect("a check ran after the write");
    assert!(
        steps[failed_check + 1..].contains(&"amend"),
        "the amend step is taken again after the check that exited non-zero: {steps:?}"
    );
}
