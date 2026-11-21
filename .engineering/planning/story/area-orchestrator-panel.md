---
format: aep.planning-md/3
id: story:area-orchestrator-panel
kind: story
status: proposed
title: Orchestrator panel and wave decisions
tags:
- orchestration
relations:
- decomposes: epic:area-board
- depends_on: story:area-board-shell
- depends_on: story:orchestrate-wave-propose
- depends_on: story:orchestrate-wave-approve
scope:
- confidence: inferred
  path: ui/decisions.mjs
- confidence: inferred
  path: ui/test/area-orchestrator.spec.mjs
- confidence: inferred
  path: ui/web/area-orchestrator.js
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:35Z", actor: "human:timo", revision: 3}
---
# Story: area-orchestrator-panel

## Outcome

A reader sees where the model proposed and where a rule decided: the LLM's ranking with reasons
beside the admission verdict, and `wave.approve` as one more kind in the Decisions drawer,
grouped by lane (`docs/orchestration.md` § 5).

## Acceptance

Against a recorded run, the panel shows each ranked story with its reason next to the admission
result for it, and the Decisions drawer lists a pending `wave.approve` under the area lane.

## Scope

A new `ui/web/area-orchestrator.js`, `ui/decisions.mjs`, a new spec.
