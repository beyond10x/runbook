---
format: aep.planning-md/3
id: story:the-free-run-gets-its-second-start-in-the-daemon
kind: story
status: draft
title: The free run gets its second start in the daemon
summary: The free run's max_runs of 2 never gives its node a second start in the daemon.
relations:
- informed_by: story:the-free-runs-turns-follow-the-runs-call-cap
revision: 1
---
## Outcome

The free run's `max_runs: 2` gives its agent node a second start in the daemon when the first ends without a
result, as the binding says.

## Source

- The adversary pass on the free run's turns (branch `impl/free-run-turns-follow-call-cap`), 2026-11-27: the
  free run's node is never started a second time in the daemon.
- On `origin/main` `6fd1d7cf`: the free run's binding names `budget: {max_runs: 2}`
  (`runbook/src/delegation.rs:391`) and maps a non-zero exit to `failed` evidence
  (`runbook/src/delegation.rs:393-398`); the daemon's step loop ends the run at the first failed step
  (`runbook/src/engine/mod.rs:597-601`, `refusal::failed_step`) when the tree reports `Stuck`, so the second
  run the budget allows is not taken (*inferred* from the code; the adversary observed the single start).

## Acceptance

- A daemon case whose stand-in free node fails its first start and writes the result on its second ends
  `delivered` with two starts recorded; red on the base.
- A node that fails both starts ends the run naming the failed step, after two starts.
