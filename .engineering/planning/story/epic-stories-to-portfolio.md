---
format: aep.planning-md/3
id: story:epic-stories-to-portfolio
kind: story
status: proposed
title: sdc.epic hands its stories to the area portfolio
tags:
- orchestration
relations:
- decomposes: epic:orchestrate-area-case
- depends_on: story:orchestrate-protocol-adopt
scope:
- confidence: inferred
  path: cases/06-epic-awaits-delivery
- confidence: inferred
  path: driver/examples/sdc/sdc-epic.bindings.json
- confidence: inferred
  path: protocol/compiled/sdc-epic.ir.json
- confidence: inferred
  path: protocol/sdc-epic.protocol.yaml
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:35Z", actor: "human:operator", revision: 3}
---
# Story: epic-stories-to-portfolio

## Outcome

Overlap between stories of different epics is checked in one place, the area case (decision 1,
`docs/orchestration.md` § 7).

## Context

Today `sdc.epic` opens one `sdc.story` per story itself (`stories.open`,
`protocol/sdc-epic.protocol.yaml:133-159`). Proposed in § 2.4: it hands its accepted stories to the
portfolio and waits for a copied `stories_delivered` record, the way `roadmap.read` copies today.

## Acceptance

In `cases/06-epic-awaits-delivery`, an `sdc.epic` with accepted stories has no `stories.open`
action, is not terminable, and becomes terminable `passed` only once a copied `stories_delivered`
record for its stories is present.

## Scope

`sdc.epic` protocol, its compiled IR and bindings, one fixture case.

## Out of Scope

The area case side of the hand-off (`story:orchestrate-wave-integrate`).
