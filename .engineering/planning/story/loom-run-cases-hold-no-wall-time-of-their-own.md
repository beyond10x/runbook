---
format: aep.planning-md/3
id: story:loom-run-cases-hold-no-wall-time-of-their-own
kind: story
status: draft
title: No case of the loom_run tests asserts a wall time of its own
summary: A 20 s assertion on the elapsed time of a run failed once at a load near 100. The runner's 60 s bound is the only clock. From security pass 1 of the zero-caps unit, pre-existing.
relations:
- informed_by: story:caps-are-optional
revision: 1
---
## Outcome

No case of `runbook/tests/loom_run/` asserts a wall time of its own. The only clock of a test is the runner's: a test over 60 s is killed and counts as failed.

## Source

Security pass 1 of `story:run-without-model-node-needs-no-caps` measured that an existing assertion of 20 s on the elapsed time of a run in `runbook/tests/loom_run/inproc.rs` failed once at a load average near 100 (20.05 s) and passed at 9.2 s in another run of the same suite. The assertion is a second, tighter clock; two such lines stand in that file (a 20 s bound twice) and one of 60 s.

## Scope

- `runbook/tests/loom_run/inproc.rs`.
- The test step of the crate `runbook`.

## Acceptance

- No `assert!` in `runbook/tests/loom_run/` compares an elapsed time with a constant, and no case loses another assertion. The suite passes in ten runs at a load average above 80. A check (a `grep` in the gate) fails when such a line is added.

## Origin

Pre-existing: the lines are on the default branch. Held by the review record of the security pass 1 of the zero-caps unit (finding F8, undecided there); carried by later passes as open.
