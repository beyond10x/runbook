---
format: aep.planning-md/3
id: story:bugfix-runs-the-reproducing-test-before-the-suite
kind: story
status: active
title: tests.run of the bug-fix runbook runs the reproducing test before the suite and stops when it is red
summary: The reproducing test alone runs first; a red one ends the step without the suite; the record names the deciding command and both times.
relations:
- decomposes: epic:ticket-to-merge-request-on-a-real-repository
- depends_on: story:bugfix-checks-are-one-action-each
- informed_by: story:bugfix-check-names-its-pool-time
scope:
- confidence: cited
  path: driver/src/bin/sdc-bugfix.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T12:21:21Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
- {from: "proposed", to: "active", at: "2026-11-23T12:21:21Z", actor: "human:operator", revision: 3, executor: "agent:runbook"}
---
## Defect

From the same run as `story:bugfix-keeps-the-output-of-a-failed-check`: `tests.run` took 7 minutes 50
seconds. It ran the whole suite first (about 5 minutes) and the reproducing test alone after it. Once the
suite had failed, the second command could not change the step's result.

## Outcome

1. `tests.run` runs the reproducing test alone first. When it is still red, the step ends there as
   `not_fixed` and the suite does not run.
2. When it is green, the suite runs. The step's result names which of the two commands decided it.
3. The time of each command is in the step's record.

## Acceptance

With a stand-in check tool that records the order of its calls: a red reproducing test gives one call and
`not_fixed`; a green one gives two calls in that order; the record names the deciding command and both
times.

## Origin

A consumer's run report of 2026-11-23, relayed by the operator's host session.

## Second report: one pool job where the suite holds the reproducing test

A consumer's review of a later run, 2026-11-23: `tests.run` ran the suite on the tree and then the
reproducing test alone as a second pool job. The suite already contained the reproducing test, and the
`green` pattern of the checks file matched the suite's own line. In that run the second job waited in the
pool's queue until the step's time ran out, so a green suite ended `not_fixed`.

A pool job's wait in the queue (10 to 30 minutes measured on the same day) is longer than the suite's run
time, so the number of jobs counts before their order.

4. Where the suite holds the reproducing test, `tests.run` is one pool job: the suite. The reproducing
   test's result is read from the suite's output with the checks file's own patterns, and no second job
   follows.
5. Outcomes 1 and 2 (the reproducing test alone first) hold where the suite does not hold the reproducing
   test, or where the checks file names no pattern that reads its result from the suite's output.
6. A suite job that did not end by itself records no verdict, as in
   `story:bugfix-check-names-its-pool-time`.

Acceptance, added: with a stand-in check tool whose suite output carries the reproducing test's line, a
green suite gives one call and `fixed`; a suite whose output shows the reproducing test red gives one call
and `not_fixed`; no case starts a second job after a suite that showed the result.
