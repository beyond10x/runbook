---
format: aep.planning-md/3
id: story:area-board-shell
kind: story
status: proposed
title: Area header, lanes, drill-down and timeline
tags:
- orchestration
relations:
- decomposes: epic:area-board
- depends_on: story:orchestrate-protocol-adopt
scope:
- confidence: inferred
  path: ui/server.mjs
- confidence: inferred
  path: ui/test/area.spec.mjs
- confidence: inferred
  path: ui/web/area.js
- confidence: inferred
  path: ui/web/index.html
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:35Z", actor: "human:timo", revision: 3}
---
# Story: area-board-shell

## Outcome

A reader opens an area and sees its header state and one lane per open child case; clicking a
lane opens today's view of that case (`docs/orchestration.md` § 5).

## Context

Header: area, repositories, store, horizon, quiescent since, health and its expiry, Decisions
count. Lane: phase pips from the child's claims as today (`ui/README.md:28-29`). Timeline: area
events (arrivals, waves, expiries).

## Acceptance

`slot npx playwright test --workers=2 ui/test/area.spec.mjs` passes against a recorded area run
with two open children, asserting the header horizon, two lanes, and that a lane click shows that
case's phase strip.

## Scope

`ui/web/index.html`, a new `ui/web/area.js`, `ui/server.mjs` (area endpoint), a new spec.
