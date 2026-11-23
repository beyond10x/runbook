---
format: aep.planning-md/3
id: story:runbook-test-step-budget
kind: story
status: active
title: The test step of the crate runbook ends within 120 s
summary: Measured 374 s whole and 126 s for a filter of 380 tests under load; expensive products made once per run, the runner ends a test at 60 s, the stand-in of the bug-fix tests wins over an old helper.
tags:
- backlog-2026-11-23
relations:
- informed_by: story:driver-test-budget
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: runbook/.config/nextest.toml
- confidence: cited
  path: runbook/src/assets.rs
- confidence: cited
  path: runbook/tests
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T03:12:06Z", actor: "human:operator", revision: 2, executor: "agent:runbook", correlation: "goal-2026-11-23-task-check-budget"}
- {from: "proposed", to: "active", at: "2026-11-23T03:12:07Z", actor: "human:operator", revision: 3, executor: "agent:runbook", correlation: "goal-2026-11-23-task-check-budget"}
---
## Outcome

The test step of the crate `runbook/` (`task test:runbook`, part of `task check`) ends within 120 s
on the shared host and no test of it runs over 60 s; the runner ends a test at 60 s and the step
prints its ten slowest tests.

## Source

- The rule "Tests stay fast" of the organisation's `AGENTS.md`: a step over the budget is a defect,
  it gets a story and is fixed before the next feature; raising a timeout is not a fix.
- Measured on branch `impl/loom-pin-db2726ff`, 2026-11-23: the whole test step of `runbook/` took
  374 s at a load average of 139; a filter of 380 of its tests took 126.3 s at a load of 70 to 83.
- Seven `loom_run::bugfix::*` tests fail on a host where an earlier build left a real `sdc-bugfix`
  in `runbook/target/debug/`: the stand-in of the tests loses to a helper beside the executable
  (`runbook/src/assets.rs` lines 158 to 171, `runbook/tests/loom_run/bugfix.rs` lines 75 to 79).
- The goal's done test: `task check` exits 0 within 120 s (`story:driver-runtime-removed`).

## Acceptance

- A diagnosis read from the code and from one timed run names the tests and the shared products
  that take the time (a product built per test, a process started per test, a wait by design).
- An expensive product is made once per test run (a committed generated file with a digest, or a
  cache on disk keyed by the digest of its sources) and read by every test.
- The step runs within 120 s at the load the host has in a working hour (the report names the load
  average) and no test over 60 s; `runbook/.config/nextest.toml` ends a test at 60 s; the task
  prints the ten slowest tests and fails when the step runs over 120 s.
- The stand-in of the bug-fix tests wins over a helper of an earlier build.
- No test is deleted or weakened to get there: a test moved to the slow lane is named with its
  reason, and the fast lane keeps a test of the same rule on a smaller input.

## Scope

`runbook/tests/`, `runbook/.config/nextest.toml`, `runbook/src/assets.rs`, `Taskfile.yml`,
`CHANGELOG.md`.
