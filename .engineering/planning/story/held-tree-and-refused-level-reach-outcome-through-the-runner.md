---
format: aep.planning-md/3
id: story:held-tree-and-refused-level-reach-outcome-through-the-runner
kind: story
status: draft
title: A held tree and a refused level reach the files of a run through a case that drives the runner
summary: The two ends are held only by a hand-built event and a test of the function that names the reason. No command path reaches them today. From adversary pass 1 of the Loom pin unit db2726ff.
relations:
- informed_by: story:loom-pin-db2726ff
revision: 1
---
## Outcome

The two ends of a run that Loom names `tree_stopped` and `confinement_refused` are shown to reach `outcome.json` and `run-stopped.json` by a case that drives the runner, not by a hand-built event.

## Source

Adversary pass 1 of `story:loom-pin-db2726ff` measured that these two ends are held only by a hand-built `Event::Ended` (`runbook/tests/loom_run/run_dir.rs:677`) and by a test of the function that returns the reason (`held.rs`). No test drives a held tree or a held cgroup root through the runner to the files of the run. No command path holds a tree or runs two runs in one process, so nothing reaches it today; the daemon's engine ends a stopped tree as a plain failure text (`runbook/src/engine/mod.rs:200`).

## Scope

- `runbook/tests/loom_run/`, `runbook/tests/engine/`: a double of the host that answers with the two refusals.
- `runbook/src/engine/mod.rs` (the end of a stopped tree), if the case shows the cause is lost there.

## Acceptance

- With a host double that answers a step with `tree_stopped`, and one that answers with `confinement_refused`, the runner ends with the outcome `refused` and the named cause in both files of the run directory. A case for each is in the default step. The daemon's engine states the same cause for a stopped tree, or a case names the difference.

## Origin

Introduced by the pin unit `story:loom-pin-db2726ff`. Held by the review record of its adversary pass 1 (finding 9; no reach today), not taken by fix round 1.
