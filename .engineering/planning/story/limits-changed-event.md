---
format: aep.planning-md/3
id: story:limits-changed-event
kind: story
status: proposed
title: Change a limit during a run as a recorded event
tags:
- orchestration
relations:
- decomposes: epic:flow-limits
- depends_on: story:wip-limits-admission
- depends_on: story:per-case-lock
scope:
- confidence: inferred
  path: cases/20-limits-changed
- confidence: inferred
  path: driver/src/main.rs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:36Z", actor: "human:timo", revision: 3}
---
# Story: limits-changed-event

## Outcome

The WIP sweet spot can be tuned during a run, and every change is on the record
(`docs/orchestration.md` § 2.4a).

## Context

Event `limits_changed` (who, old, new), by a person or a rule, read at the next admission, never
retroactive; no LLM raises a limit (`docs/autonomy-levels.md:271-272`).

## Acceptance

A run where a person raises `max_in_progress` from 2 to 3 after wave 1 records one
`limits_changed` event, admits wave 2 under 3, leaves wave 1's `admission` record unchanged, and
refuses the same change from a `decider:` or LLM principal.

## Scope

`driver/src/main.rs` (event and command), one fixture case. TEMPORARY copy
(`docs/driver.md:43-45`).
