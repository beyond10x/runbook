---
format: aep.planning-md/3
id: story:cycle-ends-at-delivered
kind: story
status: proposed
title: The cycle ends at delivered
summary: sdc.release runs alone; the run of an idea ends at delivered (proto/cycle-ends-delivered)
tags:
- eval
- improvement-loop
relations:
- decomposes: epic:improvement-loop
- depends_on: story:improvement-baseline
- informed_by: story:root-outcome-delivered
scope:
- confidence: inferred
  path: driver/examples/sdc
- confidence: inferred
  path: experiments
- confidence: inferred
  path: protocol
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T12:38:21Z", actor: "human:operator", revision: 5, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: cycle-ends-at-delivered

## Outcome

The cycle ends at `delivered`; `sdc.release` runs alone (branch `proto/cycle-ends-delivered`,
`c0ebccb`, D150). The release and operate stages leave the run of an idea.

## Context

The 0.1.0 notes list the split as not in the release. Related: `story:root-outcome-delivered`
(the outcome's name); the stories `stage-runs-alone` and `sdc-operate-protocol` live on other
branches.

## Hypothesis and metric

- Hypothesis: wall time and cost of a clean pass fall by the release stage's share of them.
- Primary metric: `run.wall_s` of a clean pass, direction decrease, by at least the release
  stage's median share of `driver_s` in the baseline (`story:improvement-baseline`); with it
  `run.cost_usd` of a clean pass by the release stage's share of cost.
- Guards: the clean-pass count; `run.delivered`.

## Acceptance

An experiment record with this hypothesis and at least 3 runs per variant reaches a verdict by
`sdc-experiment report`, and the ledger lists it whatever the verdict.
