---
format: aep.planning-md/3
id: story:child-way-gives-substrate-or-refuses-before-a-run-directory
kind: story
status: draft
title: A run with --loom under substrate runs under it or is refused before a run directory exists
summary: The child way cannot give substrate and refuses with exit 12 after the run directory is made. No bound program starts. From security pass 3 of the zero-caps unit, pre-existing.
relations:
- informed_by: story:caps-are-optional
revision: 1
---
## Outcome

A run through the Loom child process (`--loom`) under the level `substrate` either runs under that level or is refused by name before any run directory is made.

## Source

Security pass 3 of `story:run-without-model-node-needs-no-caps` measured that the child way cannot give `substrate`: the runner passes `PATH`, `HOME` and `XDG_DATA_HOME` to Loom, so Loom cannot reach the session bus to delegate a cgroup and refuses with exit 12 (`ConfinementRefused`) after the run directory has been made. No bound program starts, so the guard holds; the refusal comes later than the one of the library way, which writes nothing (`runbook/src/loom_run/child.rs:33`).

## Scope

- `runbook/src/loom_run/child.rs`, `runbook/src/loom_run/child_host.rs` (the environment given to Loom, or a check before the directory is made).
- `docs/loom-run.md` (the sentence on the child way and the levels).
- `runbook/tests/loom_run/`: the ignored measurement of the child way becomes a case of the slow lane.

## Acceptance

- With `--loom` and `--confinement substrate` on a host that delegates a cgroup, the bound programs run under the level and see no network, or the run is refused with the named cause and no run directory exists afterwards. A case of the slow lane shows the outcome chosen, and `docs/loom-run.md` states it.

## Origin

Pre-existing: the environment of the child is set in code that the zero-caps unit did not change; measured with the Loom binary of the prior pin, and by reading at the current pin. Held by the review record of the unit's security pass 3 (finding F9).
