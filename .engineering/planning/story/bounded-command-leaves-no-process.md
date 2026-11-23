---
format: aep.planning-md/3
id: story:bounded-command-leaves-no-process
kind: story
status: draft
title: A bounded command of a conformance observation leaves no process behind
summary: 'Security pass 3 of the driver test-budget unit, P1, pre-existing: run_bounded ends its direct child at the bound; a process that child started runs on. First of that pass''s backlog findings.'
relations:
- informed_by: story:driver-test-budget
revision: 1
---
## Outcome

Nothing a bounded command of a conformance observation started is still running when the observation has
recorded that the command was ended at its bound.

## Source

Security review pass 3 of `story:driver-test-budget` (2026-11-23), finding P1, in code older than that unit
(`driver/src/conform.rs`, `run_bounded`): the bound ends and reaps the direct child of a build step; a
process that child started is not ended with it. Reproduced on `main` at release 0.7.0. First of the four
backlog findings of that pass (the operator's delegate, D663 A).

## Acceptance

- A build step that starts a further process and then runs over `SDC_CONFORMANCE_BUILD_TIMEOUT`: when the
  observation ends with "killed after N s", no process the step started is alive. One test, red before; the
  reviewer's case is kept by the coordinator for the implementor.
- The same holds for every other caller of `run_bounded` (one test per caller, or one test of the function
  and a check that every caller goes through it).
- The bound itself does not move: the direct child is ended at the bound as today (the existing tests stay
  green with unchanged assertions).
- The driver test step stays within 120 s; no added test runs longer than 5 s.

## Not in scope

Confinement of a build step (named as a limit in `docs/conformance.md`).
