---
format: aep.planning-md/3
id: story:daemon-tests-back-when-loom-unlocks-the-store
kind: story
status: draft
title: The quarantined daemon tests come back when Loom releases its store lock explicitly
summary: Un-ignore the drain race case and the forked-lock engine case once the Loom pin unlocks tree.lock before closing it
relations:
- informed_by: story:flaky-daemon-step-and-drain-tests
revision: 1
---
## Outcome

When the Loom pin of `runbook/Cargo.toml` carries a release of the tree store's lock that does not
wait for every copy of its descriptor to close, the two cases quarantined by story
`flaky-daemon-step-and-drain-tests` run again and pass:

1. `daemon::drain::a_submit_racing_the_signal_leaves_no_run_submitted_after_draining_and_none_unended_at_drained`
   (`runbook/tests/daemon/drain.rs`).
2. `daemon::engine::a_forked_copy_of_the_store_lock_held_past_the_step_does_not_end_the_run`
   (`runbook/tests/daemon/engine.rs`).

## The cause

Loom `ed644a4` (`crates/loom-tree/src/lock.rs`) holds a store with `File::try_lock` (an
`flock`, lines 78-82) on the open file `tree.lock` and lets go only by dropping the `File`
(`Drop`, line 129: the holder file is removed, no unlock is called). `flock(2)` releases the lock
on `LOCK_UN` on any duplicate of the descriptor, or when the last duplicate is closed. A process
the daemon forks for the program of one run holds a copy of every descriptor open at that instant,
the lock of another run's store among them, until it execs. Until then the lock outlives the call
that took it, and the next Loom call on that store is refused
`StoreInUse: tree is held by another tree command, which has not named its process yet`; the
daemon ends the run `runtime-failure`.

Evidence, 2026-11-25:

- Gate run 2 of the R1 branch (head 25792bec, log `lane-check.run2.log`, line about 2544): the
  drain race case failed with 22 runs `delivered` and one `runtime-failure`.
- The drain race case, six at once in one process with one busy thread per CPU, failed 3 of 10 and
  1 of 10 runs, each with the refusal above (log `probe3`, the reason printed by the case's
  assertion).
- The engine case above forks a process that keeps its copy of the lock's descriptor until the case
  releases it, after the run's next step: on head `1312e394` the run ends `runtime-failure` with the
  refusal above in 10 of 10 runs (log `new-case-red`).

## Acceptance

- The Loom pin releases the store lock explicitly (`File::unlock` or `flock(LOCK_UN)`) before the
  descriptor is closed; the Loom commit and its lines are named.
- Both cases lose their `#[ignore]` and pass: the engine case 10 of 10 and the drain race case under
  the load of the full `test:runbook` step, each by its nextest summary line.
- The daemon keeps no retry or wait of its own around a refused store call.
