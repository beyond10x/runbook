---
format: aep.planning-md/3
id: story:plan-partial-activation
kind: story
status: proposed
title: Plan stage without every story active
tags:
- orchestration
relations:
- decomposes: epic:brownfield-entry
- depends_on: story:epic-stories-to-portfolio
scope:
- confidence: inferred
  path: cases/16-plan-partial
- confidence: inferred
  path: protocol/compiled/sdc-plan.ir.json
- confidence: inferred
  path: protocol/sdc-plan.protocol.yaml
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:37Z", actor: "human:operator", revision: 3}
---
# Story: plan-partial-activation

## Outcome

One draft story no longer blocks its epic: active stories go to the portfolio, drafts wait
(`docs/orchestration.md` § 4.4 gap 3).

## Context

`sdc.plan` needs every story `active` (`protocol/sdc-plan.protocol.yaml:73-78`).

## Acceptance

In `cases/16-plan-partial`, a plan with one active and one draft story passes `sdc.plan` and
hands only the active story to the portfolio.

## Scope

`sdc.plan` protocol, its IR, one fixture case.
