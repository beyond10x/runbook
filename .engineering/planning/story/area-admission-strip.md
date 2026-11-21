---
format: aep.planning-md/3
id: story:area-admission-strip
kind: story
status: proposed
title: Admission strip
tags:
- orchestration
relations:
- decomposes: epic:area-board
- depends_on: story:area-board-shell
- depends_on: story:orchestrate-wave-admit
scope:
- confidence: inferred
  path: ui/test/area-admission.spec.mjs
- confidence: inferred
  path: ui/web/area-admission.js
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:35Z", actor: "human:timo", revision: 3}
---
# Story: area-admission-strip

## Outcome

A reader sees the queue: which stories are admitted, which are held and why (collision,
dependency, unassessed, slots, budget, WIP limit) (`docs/orchestration.md` § 5).

## Acceptance

Against a recorded `admission` record with one admitted and one held story, the strip shows the
admitted id and the held id with the rule that held it, taken from the record.

## Scope

A new `ui/web/area-admission.js` and its spec.
