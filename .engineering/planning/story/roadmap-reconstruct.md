---
format: aep.planning-md/3
id: story:roadmap-reconstruct
kind: story
status: proposed
title: Reconstruct a roadmap for a store that has none
tags:
- orchestration
relations:
- decomposes: epic:brownfield-entry
- depends_on: story:intake-reconstruct
scope:
- confidence: inferred
  path: cases/18-roadmap-reconstruct
- confidence: inferred
  path: driver/examples/sdc/sdc-direction.bindings.json
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:38Z", actor: "human:operator", revision: 3}
---
# Story: roadmap-reconstruct

## Outcome

An adopted store with epics but no roadmap gets a reconstructed roadmap (an `initiative`) that
goes to `roadmap.approve`, so `epics.open` is no longer blocked (`docs/orchestration.md` § 4.3,
§ 4.4 gap 1).

## Context

AEP 0.80.0 has no roadmap kind; the roadmap is an `initiative` (`docs/protocol-notes.md:192-194`).
`epics.open` needs `roadmap.approved` (`protocol/sdc.protocol.yaml:103-125`).

## Acceptance

On `cases/18-roadmap-reconstruct` (one active epic, no initiative), the direction case writes an
initiative citing the epic and stops awaiting `roadmap.approve`.

## Scope

`sdc.direction` bindings, one fixture case.
