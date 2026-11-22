---
format: aep.planning-md/3
id: story:decider-finding-decision
kind: story
status: proposed
title: The decider or requester agent takes a finding decision
tags:
- conductor-succession
relations:
- decomposes: epic:conductor-succession
- depends_on: story:requester-decider
scope:
- confidence: inferred
  path: driver/examples/sdc/tools/decider.sh
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:25:41Z", actor: "human:operator", revision: 3, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: decider-finding-decision

## Outcome

A finding that needs a decision can be decided by the decider or the requester agent at the levels
that grant it, instead of always stopping for a person.

## Context

Today an `amend.upstream` finding classed for a decision goes to a person only (`docs/trial-2.md`,
"The person's part").

## Acceptance

On the trial-2 fixture at A3 with a scope-changing finding open, the requester agent records the
decision in `findings.yaml` with `decided_by: decider:requester` and a ground citing a goals entry,
`amend.upstream` runs once, and at A2 the same finding still stops for a person.

## Scope

`driver/examples/sdc/tools/decider.sh` (existing bash: a fix to prototype glue).
