---
format: aep.planning-md/3
id: story:pool-wait-is-not-a-failure
kind: story
status: active
title: A step that waited for the build pool is not a failed test run
summary: a test step that got no slot ends with exit 69, never as red tests and never with a model call; the wait is bounded apart from the run
tags:
- test-budget
relations:
- decomposes: epic:conductor-succession
- informed_by: story:cycle-verdict-independent-of-slot-wait
scope:
- confidence: cited
  path: bindings
- confidence: cited
  path: bindings/tools/sdc-tool.sh
- confidence: cited
  path: docs/driver.md
- confidence: cited
  path: driver/src/bin/sdc-replay.rs
- confidence: cited
  path: driver/src/main.rs
- confidence: cited
  path: driver/src/run_metrics.rs
- confidence: cited
  path: driver/tests
- confidence: cited
  path: scripts/run-finding-cycles.sh
- confidence: cited
  path: scripts/run-trial.sh
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T07:09:32Z", actor: "human:operator", revision: 3, executor: "agent:rb3-pool-wait", correlation: "operator-standing-acceptance-2026-11-22"}
- {from: "proposed", to: "active", at: "2026-11-22T07:09:32Z", actor: "human:operator", revision: 4, executor: "agent:rb3-pool-wait", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: pool-wait-is-not-a-failure

## Outcome

A test step that never got a slot in the shared build pool is not recorded as red tests, and no
model is called to repair tests that never ran. It ends with its own exit, 69, which the driver and
Loom map to a stop that says the pool gave no slot. The time a step may wait for the pool is bounded
by the tool, apart from the time its commands may run, and the step's limit covers both.

## Context

On 2026-11-22 a registration run of release 0.3.0 reached `tests.run` while the pool had no free
slot and 60 queued requests. Each of the step's two pool jobs waited about 15 minutes. The driver
killed the step at its limit of 1800 s, which counts the queue wait, and recorded `exit: timeout`.
The binding maps every other exit to `failed`, so the tree went on to `amend.upstream` and a paid
`change.implement` call to repair tests that never ran. The release gate cycle `bugfix-full` ended
`not_fixed` the same way earlier that day (`story:cycle-verdict-independent-of-slot-wait`).

Two faults: a step that never got a slot is recorded like a failed test run, and the limit counts
the wait. This story fixes both in the tool, the bindings and the driver, with the arithmetic in
`docs/driver.md`, "The build pool wait".

## Design

- `bindings/tools/sdc-tool.sh` runs each pool command of `conform-run` and `impl-gate` through
  `pool_run`: `slot` in the background, the job's id read from slot's own `queued` line, its state
  read from `slot status`. A job still queued when the step's wait bound is spent is cancelled
  (`slot cancel`) and the tool exits 69. One bound per step, shared by its pool commands through a
  ledger file. The bound is the argv option `--pool-wait <seconds>` (the environment variable
  `SDC_POOL_WAIT` wins, for tests); without either, no bound, as before.
- The run bound stays the pool's: 10 minutes, or a named `slot timeout <s>`. The tool does not name
  it, because `slot timeout ...` is another toolchain class than `slot cargo ...`.
- The driver retries a step whose exit is 69 up to 3 attempts, records nothing, counts no run, and
  then stops with the verdict `pool_wait_spent` (exit 5). Rejected: recording a result such as
  `not_run`, which conflicts with the later real record and leaves `tests.run` unknown for Canon.
- Loom: the tool exits 69 under Loom too. The loader the YAML follows (`8a8fad3b`) has no `exits`
  member, so the YAML still maps 69 through `*` to `failed` until it has one; `exits: {error: [69,
  75, 126, 127]}` then ends the run `ProgramError`, and Loom has no count of retries for an `error`
  exit. Both are lines of `bindings/PENDING.md`.
- `sdc-run-metrics` counts such takes as `pool_waits`, apart from `could_not_run` and the real takes.

## Acceptance

- With a stand-in `slot` that prints its `queued` line and never starts the job, `sdc-tool.sh
  impl-gate` with a wait bound of 1 s exits 69 within 5 s, cancels the job and names the bound; no
  test result is read.
- With a stand-in `slot` that starts the job after a short wait and the job exits 0, `impl-gate`
  exits 0 as before; when the job exits 101 it exits 1 as before.
- With a stand-in `slot` that reports a job cancelled after it started (exit 130), `impl-gate`
  exits 1 and its log names the pool's cancel; it does not exit 69. A job cancelled while still
  queued exits 69.
- The wait bound is one per step: when the first pool command of `story-tests-run` waited the whole
  bound, the second is not started and the step exits 69.
- `bindings/sdc-story.yaml` (`tests.run`), `bindings/sdc-release.yaml` (`acceptance.run`) and the
  JSON bindings of the driver name `--pool-wait`, and each states a timeout of at least the wait
  bound plus the run bounds of the step's pool commands plus 600 s, the arithmetic written in
  `docs/driver.md`. The YAML cannot list exit 69 in `exits.error` while the loader it follows has no
  `exits` member: the exit is a line of `bindings/PENDING.md` for both bindings.
- `sdc-drive run` on a case whose `tests.run` tool exits 69 takes 3 attempts, records no evidence of
  kind `conformance_run`, starts no second `change.implement`, and ends with the verdict
  `pool_wait_spent` and exit 5 (finding cycle `pool-wait`).
- `sdc-run-metrics` reports a take with exit 69 under `pool_waits` and not under `could_not_run`;
  `scripts/run-trial.sh` ends such a run with cause `pool_wait_spent` and calls no `run.unstick`.

## Out of scope, open

- `sdc-bugfix` (`repro-run`, `tests-run`, `gate-run`) runs its commands through `slot` itself; the
  same fault is there and is not fixed here.
- Loom never retries an `error` exit; a count of retries is a Loom story.
