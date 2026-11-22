---
format: aep.planning-md/3
id: story:area-intake-strip
kind: story
status: proposed
title: Intake strip
tags:
- orchestration
relations:
- decomposes: epic:area-board
- depends_on: story:area-board-shell
- depends_on: story:orchestrate-intake-triage
scope:
- confidence: inferred
  path: ui/test/area-intake.spec.mjs
- confidence: inferred
  path: ui/web/area-intake.js
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:35Z", actor: "human:operator", revision: 3}
---
# Story: area-intake-strip

## Outcome

A reader sees what arrived, how each entry was routed, and whether triage holds at the current
`intake` revision (`docs/orchestration.md` § 5).

## Acceptance

Against a recorded run with one unrouted and two routed entries, the strip lists the unrouted one
under "new", the two under "routed" with their targets, and shows triage not holding.

## Scope

A new `ui/web/area-intake.js` and its spec.
