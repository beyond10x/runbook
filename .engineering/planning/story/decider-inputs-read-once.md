---
format: aep.planning-md/3
id: story:decider-inputs-read-once
kind: story
status: draft
title: What the decider is told and starts is fixed at the start of the run
summary: The runner reads the decider instruction files and starts its adapter from the run copy on every decision; under local confinement a bound program can write them. They are fixed at the start like the routing table.
tags:
- from-finding
relations:
- informed_by: review-result:security-line2-units-3-4-pass-1
revision: 1
---
## Outcome

What the decider of a run is told and which program it starts are fixed at the start of the run,
like the routing table: a change of the run copy during the run changes neither.

## Finding

`review-result:security-line2-units-3-4-pass-1`, finding A (origin pre-existing, verdict
NEEDS-CHANGE): `runbook/src/loom_run/decider.rs:619` (`instructions_root`), `:399`, `:467`,
`runbook/src/loom_run/decider/ask.rs:85`. The runner reads the decider's instruction files from
the run copy on every decision and starts the decider's adapter and the declared answer programs
from the run copy on every decision. `instructions_root` walks up to any ancestor directory
that has `instructions/`. Under `--confinement local` a bound program can write those files;
under `--confinement substrate` the run directory is neither visible nor writable to it. The
routing table is read once since wave `2026-11-22-w1` (`Decider::pin_routing`); these files
are not. The code is the same on `main` at `17c00ab57f`. Also named by
`review-result:adversary-line2-fix-rounds-1-2-pass-2` (finding 4).

## Acceptance

- A decision is written down first: the runner reads the instruction text once and keeps it in
  memory, or it copies the instructions, the adapter and the answer programs to a directory of
  its own that a bound program is not given, or a decider is refused under `local`. The choice
  names what it costs a run that is resumed.
- `instructions_root` takes the run's own instruction directory only; it does not walk above the
  run directory. A run copy without `instructions/` is refused at the start with the path.
- The four cases of the reviewer's patch that record today's behaviour under `local` are turned
  around and pass: a program that appends to the decider's instructions does not change what the
  model is told; a program that replaces the decider's adapter is not started by the runner.
- The case under `substrate` stays green.
- `docs/loom-run-decisions.md` says what is read once and what is not.

## Scope

- `runbook/src/loom_run/decider.rs`, `runbook/src/loom_run/decider/`, `runbook/src/loom_run/run_dir.rs`.
- `runbook/tests/loom_run/`, `docs/loom-run-decisions.md`, `docs/loom-run.md`.

## Not in this story

A confinement for `local`: `local` is declared as no confinement. The replay plan and the call
log: `story:replay-plan-out-of-the-run-copy`.
