---
format: aep.planning-md/3
id: story:cycle-verdict-independent-of-slot-wait
kind: story
status: proposed
title: A cycle's verdict does not depend on the wait for a build slot
summary: the driver's step limit counts the run time only, not the time queued in the build pool; bugfix-full ended not_fixed on 2026-11-22 after 1800 s queued
tags:
- test-budget
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: inferred
  path: bindings/tools/sdc-tool.sh
- confidence: inferred
  path: driver/src/main.rs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T04:05:25Z", actor: "human:operator", revision: 2, correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: cycle-verdict-independent-of-slot-wait

## Outcome

A cycle's verdict does not depend on how long a bound command waited for a build slot. The driver's
step limit counts only the time the command runs, not the time it is queued in the shared build pool.

## Context

On 2026-11-22 the cycle `bugfix-full` of the release gate ended `not_fixed`: its test step's only log
line was `slot: job ... queued` and the driver killed the step after 1800 s, the limit
(`sdc-drive: killed after 1800 s`). The step had waited the whole time in the pool of a busy
machine and never ran. A verdict that changes with the load of the host is not a verdict of the
protocol or the code; the same cycle passes on a free machine.

## Acceptance

- A bound command that waits N seconds for a slot and then runs within its limit is not killed for
  the wait, shown by a test with a stub pool (a stand-in for `slot` that holds the command N seconds
  before running it, with the step limit set below N and the run time below the limit).
- A command that runs past its limit after getting its slot is still killed, with the same log line
  as today.
