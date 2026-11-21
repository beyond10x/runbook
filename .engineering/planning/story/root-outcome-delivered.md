---
format: aep.planning-md/3
id: story:root-outcome-delivered
kind: story
status: proposed
title: Rename the root outcome operating to delivered
tags:
- orchestration
relations:
- decomposes: epic:orchestrate-area-case
scope:
- confidence: inferred
  path: cases/01-idea-only
- confidence: inferred
  path: cases/03-ready-to-release
- confidence: inferred
  path: protocol/compiled/sdc.ir.json
- confidence: inferred
  path: protocol/sdc.protocol.yaml
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:38Z", actor: "human:timo", revision: 3}
---
# Story: root-outcome-delivered

## Outcome

The root `sdc` outcome says what it means, "delivered once"; "operating" is left to the standing
area case (`docs/orchestration.md` § 3, last rule).

## Context

`operating` is terminal after one pass (`protocol/sdc.protocol.yaml:131-142`; § 1.2).

## Acceptance

`scripts/run-full-cycle.sh` exits 0 with the root case ended `delivered`, and
`grep -c operating protocol/sdc.protocol.yaml` prints 0.

## Scope

Root protocol, its IR, fixture cases whose expected outcome names it. *inferred*: the UI and
scripts that print the outcome may also need the new name.
