---
format: aep.planning-md/3
id: story:bindings-state-their-exits
kind: story
status: draft
title: The bindings state which exit codes are a program error and which mean not called
summary: 'Found while wave 2026-11-22-w1 ran: Loom 70b0e6e0 loads the exits member; bindings/PENDING.md still lists D36, D37 and the pool-wait exits as open.'
tags:
- backlog-2026-11-22-w1
relations:
- informed_by: story:driver-runtime-removed
- depends_on: story:loom-bindings-files
revision: 1
---
## Outcome

Every program binding of `bindings/*.yaml` states which exit codes are a program error and which
mean "not called", with Loom's member `exits: {error: [...], not_called: [...]}`. A tool that
could not run ends the run as `ProgramError` and records nothing; it is no longer mapped to
`failed` by `results: "*": failed`.

## Source

- `bindings/PENDING.md`, the entries marked D36 (program-error exits 75, 126, 127 on the program
  bindings of all eight files), D37 (`sdc-node` not called) and `story:pool-wait-is-not-a-failure`
  (exits 69 and 70 on `tests.run`, `acceptance.run`, `repro.run`, `gate.run`).
- Loom session, 2026-11-22: the member is loaded at Loom `70b0e6e0` (the commit `runbook/Cargo.toml`
  pins on line 2). An exit in `error` ends the run `ProgramError`, records nothing and is not
  counted against `max_runs`; an exit in `not_called` records nothing and is not counted.
- Found while wave `2026-11-22-w1` ran; not a unit of it.

## Acceptance

- Each program binding of `bindings/*.yaml` carries `exits.error`: `[75, 126, 127]`, and
  `[69, 70, 75, 126, 127]` on the bindings `bindings/PENDING.md` lists for the pool wait.
- Each model-node binding carries `exits.not_called` with one code, and `sdc-node` under Loom
  (`LOOM_CASE` set) exits with that code where it prints "not called" today.
- A test binds a program that exits 75: the run ends `ProgramError`, the run log holds no result
  for the node, `max_runs` is not reduced. A second test does the same for the not-called code.
- The D36, D37 and pool-wait entries leave `bindings/PENDING.md`; `scripts/check-bindings.sh`
  exits 0.
- `runbook test cycles/tree-suite.yaml`: no entry changes from `expected` to another state.

## Scope

- `bindings/*.yaml`, `bindings/PENDING.md`.
- `driver/src/bin/sdc-node.rs` (the exit code of "not called").
- `runbook/tests/` (the two tests).

## Not in this story

What Loom does after a pool wait (it does not run the step again): an open item of
`story:pool-wait-is-not-a-failure`.
