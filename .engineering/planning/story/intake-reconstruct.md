---
format: aep.planning-md/3
id: story:intake-reconstruct
kind: story
status: proposed
title: Reconstruct intake from an existing repository
tags:
- orchestration
relations:
- decomposes: epic:brownfield-entry
scope:
- confidence: inferred
  path: cases/14-intake-reconstruct
- confidence: inferred
  path: driver/examples/sdc/sdc-intake.bindings.json
- confidence: inferred
  path: protocol/compiled/sdc-intake.ir.json
- confidence: inferred
  path: protocol/sdc-intake.protocol.yaml
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:36Z", actor: "human:timo", revision: 3}
---
# Story: intake-reconstruct

## Outcome

For an adopted repository, requirements and context are derived from code, docs and store, each
item citing `path:line`, and only what cannot be cited becomes an interview question
(`docs/orchestration.md` § 4.2; gap 6, § 4.4).

## Context

`sdc.intake` starts from `idea.yaml` only. Sources: `aep plan reverse scan` byte-stable facts
(`RE:28-29`), "no citation, no artifact" (`RE:12-13`).

## Acceptance

On `cases/14-intake-reconstruct` (a fixed copy of facts from `decider`), every requirement and
context item in the reconstructed output carries a `path:line` that resolves in the fixture, and
every item without one appears as a question instead.

## Scope

`sdc.intake` protocol (a reconstruct path), its IR and bindings, one fixture case.
