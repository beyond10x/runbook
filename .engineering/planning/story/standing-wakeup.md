---
format: aep.planning-md/3
id: story:standing-wakeup
kind: story
status: proposed
title: Runtime support for standing cases
tags:
- orchestration
relations:
- decomposes: epic:concurrent-child-cases
scope:
- confidence: inferred
  path: driver/src/main.rs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:38Z", actor: "human:operator", revision: 3}
---
# Story: standing-wakeup

## Outcome

A case that lives for months is not exhausted by its run counters, rests without holding a slot,
and wakes on a new artifact revision or at the earliest `max_age` expiry (`docs/orchestration.md`
§ 3, "Runtime changes needed").

## Context

`max_runs` (default 8) counts every `action_finished` of the case's whole life
(`driver/src/main.rs:366-368`). Wake-up source: file watch on `intake`, timer at the earliest
expiry (*inferred* in the doc).

## Acceptance

A standing fixture run for more than 8 intake revisions keeps `intake.route` admissible on each
new revision, and `sdc-drive` reports the verdict `quiescent` with the instant of the next expiry
when nothing is useful.

## Scope

`driver/src/main.rs`. TEMPORARY copy (`docs/driver.md:43-45`).
