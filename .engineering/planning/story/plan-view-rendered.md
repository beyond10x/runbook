---
format: aep.planning-md/3
id: story:plan-view-rendered
kind: story
status: proposed
title: Planning items and the roadmap rendered in the live view
tags:
- later
- operator-request
relations:
- decomposes: epic:live-view
scope:
- confidence: inferred
  path: ui/server.mjs
- confidence: inferred
  path: ui/test/plan.spec.mjs
- confidence: inferred
  path: ui/web/index.html
- confidence: inferred
  path: ui/web/plan.js
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:16:34Z", actor: "human:operator", revision: 3, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: plan-view-rendered

## Outcome

A reader opens a planning item or the roadmap as a rendered page: state, history, relations,
outcome and acceptance; the raw file stays one switch away.

## Context

The operator, 2026-11-21: "we have special views now also for whenever the planning items change.
That's wonderful, I love it. Maybe it would also be nice to actually be able to view the planning
items or view the roadmap … we can look at the raw file … but … visually render the actual thing.
So for the planning items it could be the state. We could basically track the acceptance criteria
and so on."

What exists: the "What changed" list has a Planning class (AEP created, status moves, edges, scope,
acceptance, sections; `ui/README.md:49-53`). The roadmap is `initiative:roadmap` and
`docs/roadmap.md`; an AEP roadmap kind is being added upstream (branch `feat/roadmap-kind` of the
`aep` repository).

## Acceptance

From a planning change in the feed, and from a new Plan view, an artifact opens rendered with
title, kind, status and its transitions, relations as links, the Outcome and Acceptance sections,
and each acceptance line with its state where one is known, with a switch to the raw file; the
roadmap renders as milestones with their stories and each story's status.

## Scope

A new `ui/web/plan.js`, `ui/web/index.html`, `ui/server.mjs` (reads `aep plan artifact show
--format json`), a Playwright spec.

## Ambiguities

- Which roadmap source to render first: `initiative:roadmap` now; the AEP roadmap kind once released
  (inferable: `upstream-blocker:aep-roadmap-kind`).
- "Each acceptance line with its state": known only where acceptance is a checkable claim
  (`story:acceptance-as-checkable-claims`); text lines show as unchecked.
