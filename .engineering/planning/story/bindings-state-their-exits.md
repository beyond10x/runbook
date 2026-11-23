---
format: aep.planning-md/3
id: story:bindings-state-their-exits
kind: story
status: active
title: The bindings state which exit codes are a program error and which mean not called
summary: 'Found while wave 2026-11-22-w1 ran: Loom 70b0e6e0 loads the exits member; bindings/PENDING.md still lists D36, D37 and the pool-wait exits as open.'
tags:
- backlog-2026-11-22-w1
relations:
- informed_by: story:driver-runtime-removed
- depends_on: story:loom-bindings-files
scope:
- confidence: cited
  path: cycles
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T22:25:54Z", actor: "human:operator", revision: 4, executor: "agent:runbook", correlation: "d613-2026-11-22"}
- {from: "proposed", to: "active", at: "2026-11-22T22:25:55Z", actor: "human:operator", revision: 5, executor: "agent:runbook", correlation: "d613-2026-11-22"}
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
  (`LOOM_CASE` set) exits with that code where it prints "not called" today. One exception: the
  renewal of a derived tree by `spec.author` prints "not called" and exits 0 (with the not-called
  code the cycle `spec-own-default` ends stuck), so it counts as one run against `budget.max_runs`;
  `docs/loom-bindings.md` says so.
- A test binds a program that exits 75: the run ends `ProgramError`, the run log holds no result
  for the node, `max_runs` is not reduced. A second test does the same for the not-called code.
- The D36, D37 and pool-wait entries leave `bindings/PENDING.md`; `scripts/check-bindings.sh`
  exits 0.
- `runbook test cycles/tree-suite.yaml`: no entry changes from `expected` to another state.
- Added on acceptance (operator's pick on D613, 2026-11-22, after record `parity-loom-strong-4`):
  runs 1 and 3 of that record stopped at `RunsSpent` because a not-called `spec.author` and a
  not-called `change.implement` counted against `budget.max_runs`. Loom `70b0e6e0` treats a
  not-called exit as "records nothing, the step is performed", so the action is not taken again
  until the case moves. A stub entry of `cycles/tree-suite.yaml` (no model call) therefore holds
  the state of run 1: a finding with status `needs_person` in a spec case and a model node that
  is not called. Its expected end: the finding is decided (by the decider at A5, by a person
  below) and the model node is then called; the run ends neither `RunsSpent` nor `Stuck`. Where
  that end needs a change outside this story's scope (a rule of Loom), the entry is `pending`
  with the Loom story it waits for, and the unit reports the code line.

## Scope

- `bindings/*.yaml`, `bindings/PENDING.md`.
- `driver/src/bin/sdc-node.rs` (the exit code of "not called").
- `runbook/tests/` (the two tests).

## Not in this story

What Loom does after a pool wait (it does not run the step again): an open item of
`story:pool-wait-is-not-a-failure`.
