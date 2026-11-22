---
format: aep.planning-md/3
id: story:flow-kpis
kind: story
status: proposed
title: Compute the flow KPIs from records
tags:
- orchestration
relations:
- decomposes: epic:flow-limits
- depends_on: story:per-story-worktree
scope:
- confidence: inferred
  path: driver/src/bin/sdc-flow-kpis.rs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:36Z", actor: "human:operator", revision: 3}
---
# Story: flow-kpis

## Outcome

F1-F16 of `docs/kpis.md` § 5 are computed from records only (case events, planning store,
`git worktree list`, fs-change record), so a run's flow is measured, not estimated.

## Acceptance

`sdc-flow-kpis <run-dir>` on a recorded run prints F1-F16 as JSON, twice the same bytes for the
same run, and reports an orphan worktree (F4 = 1) when a worktree is added for a closed case.

## Scope

A new binary `driver/src/bin/sdc-flow-kpis.rs` (beside `sdc-finding-kpis`).
