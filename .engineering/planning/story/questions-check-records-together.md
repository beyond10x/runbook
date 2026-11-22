---
format: aep.planning-md/3
id: story:questions-check-records-together
kind: story
status: proposed
title: questions.check records its two kinds together
summary: questions.check declares records_together for questions_shape and questions_settled and guards with all
tags:
- blocked-upstream
- canon
relations:
- decomposes: epic:kernel-and-runtime-uptake
scope:
- confidence: inferred
  path: driver/examples/sdc/sdc-intake.bindings.json
- confidence: inferred
  path: protocol/compiled/sdc-intake.ir.json
- confidence: inferred
  path: protocol/sdc-intake.protocol.yaml
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T06:26:12Z", actor: "human:operator", revision: 5, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: questions-check-records-together

## Outcome

`questions.check` declares its two record kinds (`questions_shape`, `questions_settled`) as recorded
together, and its guard becomes "both kinds have no record" (`all`).

## Context

Waits on Canon's `records_together` (Canon `story:simulate-record-groups`, wave 7). With guard
`any`, `canon simulate` reports 88 intake states for a second `questions_shape`. With the
declaration, Canon measured 405 reachable states and 35 findings (89 states) on `sdc.intake`.

## Acceptance

With a Canon release carrying `records_together` pinned, `canon simulate --path
protocol/sdc-intake.protocol.yaml` reports no state with a second `questions_shape` record, and
`scripts/check-protocols.sh` exits 0 with `questions.check` declaring both kinds recorded together
under an `all` guard.

## Scope

- `protocol/sdc-intake.protocol.yaml` (`questions.check`, :161-166)
- `protocol/compiled/sdc-intake.ir.json`
- `driver/examples/sdc/sdc-intake.bindings.json`

## Ambiguities

- Whether the bindings change at all depends on how Loom reads the declaration; checked when the
  Canon release lands.
