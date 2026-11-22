---
format: aep.planning-md/3
id: story:ui-goal-grounds
kind: story
status: proposed
title: UI shows the goal an answer rests on
summary: The Decisions drawer shows each goals ground's entry, or that no goal covers the question.
tags:
- requester
relations:
- decomposes: epic:requester-side-and-goals-context
- depends_on: story:decision-contract-goals-grounds
- depends_on: story:ui-decision-request
scope:
- confidence: inferred
  path: ui/decisions.mjs
- confidence: inferred
  path: ui/test/decisions.spec.mjs
- confidence: inferred
  path: ui/web/inbox.js
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:39Z", actor: "human:operator", revision: 3}
---
# Story: ui-goal-grounds

## Outcome

A person deciding a request, or reading an answer the agent derived, sees which goal entry it rests
on: its statement, kind, layer and source, linked to its line in `goals.yaml`, or "no goal covers
this".

## Acceptance

In a Playwright run over the trial-2 goals fixture, the drawer item for q3 shows for each goals
ground the entry's statement, kind, layer and source with a link that opens `goals.yaml` at the
entry's line, the item of a request marked `no_ground_in_goals` shows "no goal covers this", and a
screen-only check reading no other panel finds both.

## Scope

`ui/decisions.mjs`, `ui/web/inbox.js`, `ui/test/decisions.spec.mjs`. Builds on the drawer of
`story:ui-decision-request`.
