---
format: aep.planning-md/3
id: story:orchestrate-standing-horizon
kind: story
status: proposed
title: 'Standing horizon: health, quiescence, retired'
tags:
- orchestration
relations:
- decomposes: epic:orchestrate-area-case
- depends_on: story:orchestrate-backlog-survey
- depends_on: story:standing-wakeup
scope:
- confidence: inferred
  path: cases/13-orchestrate-standing
- confidence: inferred
  path: driver/src/bin/sdc-health-observe.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:36Z", actor: "human:operator", revision: 3}
---
# Story: orchestrate-standing-horizon

## Outcome

A standing area case never ends `done`: it rests quiescent and wakes on a new `intake` revision or
when `health` expires; only a `retired` decision ends it (decision 2, `docs/orchestration.md` § 3,
§ 7).

## Context

`health` about `release`, `max_age: 1h`, obligation `health.required` applicable only when
`horizon` is `standing` (§ 2.2). Loom completes a case with exactly one legitimate outcome, so the
standing case must have none except `retired` (§ 1.2).

## Acceptance

In `cases/13-orchestrate-standing`, evaluated at instant t0 the case has no useful action and no
legitimate outcome; evaluated at t0 + 61 min `health.observe` is admissible; after a `retired`
decision the outcome is `retired`.

## Scope

A new program `sdc-health-observe`, one fixture case.

## Tooling note

- New tool: the Rust binary `sdc-health-observe` (`driver/src/bin/sdc-health-observe.rs`), in place of `driver/examples/sdc/tools/orchestrate-health-observe.sh` named before.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)
