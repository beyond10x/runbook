---
format: aep.planning-md/3
id: story:concurrency-limit
kind: story
status: proposed
title: Limit concurrently running cases
tags:
- orchestration
relations:
- decomposes: epic:concurrent-child-cases
scope:
- confidence: inferred
  path: driver/src/main.rs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:35Z", actor: "human:timo", revision: 3}
---
# Story: concurrency-limit

## Outcome

The number of child cases running at once never exceeds a configured ceiling, and a binding can
only lower it (`docs/orchestration.md` § 2.4 rule 3; Loom `action-limits-enforced`).

## Acceptance

With a ceiling of 2 and a wave of 3 admitted stories, `sdc-drive` never has more than 2 cases
with an action in flight (checked from `action_started`/`action_finished` events), and a binding
value of 3 is refused.

## Scope

`driver/src/main.rs` (engine config, ceiling readable by the admission program).
