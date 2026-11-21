---
format: aep.planning-md/3
id: story:area-board-flow-kpis
kind: story
status: proposed
title: Flow KPIs and limits on the area board
tags:
- orchestration
relations:
- decomposes: epic:flow-limits
- depends_on: story:area-board-shell
- depends_on: story:flow-kpis
- depends_on: story:limits-changed-event
scope:
- confidence: inferred
  path: ui/test/area-flow.spec.mjs
- confidence: inferred
  path: ui/web/area-flow.js
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:35Z", actor: "human:timo", revision: 3}
---
# Story: area-board-flow-kpis

## Outcome

A reader sees unfinished work against its limits and every limit change, so a run piling up
worktrees is visible before it is a reconcile.

## Acceptance

Against a recorded run, the board shows F1, F3, F4, F5 and F6 with their limits, marks a value
over its limit, and lists each `limits_changed` event in the timeline with who, old and new.

## Scope

A new `ui/web/area-flow.js` and its spec.
