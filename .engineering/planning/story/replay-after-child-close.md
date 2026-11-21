---
format: aep.planning-md/3
id: story:replay-after-child-close
kind: story
status: proposed
title: Replay matches after a child closes
tags:
- conductor-succession
- trial-2
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: inferred
  path: driver/src/main.rs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:36:14Z", actor: "human:timo", revision: 3, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: replay-after-child-close

## Outcome

A replay of a run stopped at any point matches in every case that was evaluated.

## Context

Trial 2: root replay exit 1, 7 of 9 cases match; the epic case mismatched because closing the
access child rewrote its `case_outcome` record (event 34) after the epic's last evaluation
(event 33); the never-evaluated `IDEA-1.todo.offline` was listed, not failed
(`docs/trial-2.md:538-542`).

## Acceptance

`sdc-drive replay` of the trial-2 store copy matches every evaluated case, and lists each
never-evaluated child as such without failing the replay.

## Scope

`driver/src/main.rs` (child close and replay).
