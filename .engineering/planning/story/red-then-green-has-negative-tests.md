---
format: aep.planning-md/3
id: story:red-then-green-has-negative-tests
kind: story
status: proposed
title: red_then_green is shown to refuse bad record sets
summary: No test feeds the expression an out-of-order, missing or later-failing record set (review rb2 S3-3).
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: scripts/run-finding-cycles.sh
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T02:42:21Z", actor: "human:operator", revision: 2, executor: "agent:rb2-driver-fixes", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: red-then-green-has-negative-tests

## Outcome

The red-then-green expression of the bug-fix cycles is shown to refuse an out-of-order, a missing
and a later-failing record set, not only to accept the good one.

## Context

Source: review rb2, driver, S3-3: `scripts/run-finding-cycles.sh:2313-2316` (`red_then_green`) looks at
the last record of each kind. A `test_run` that fails after the green one, or a `repro_run` after it,
is not looked at, and no test feeds the expression an out-of-order or missing-record set and expects
red.

## Acceptance

Fed a record set with the fix before the red run, with no green run, and with a failed `test_run`
after the green one, the expression exits non-zero for each, in a test that runs without a model.
