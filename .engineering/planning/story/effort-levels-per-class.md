---
format: aep.planning-md/3
id: story:effort-levels-per-class
kind: story
status: proposed
title: Effort levels per node class
summary: Run and report experiments/effort-implement-levels.yaml on the baseline tree, then the other classes
tags:
- eval
- improvement-loop
relations:
- decomposes: epic:improvement-loop
- depends_on: story:improvement-baseline
- informed_by: story:model-per-node-variable
scope:
- confidence: inferred
  path: experiments
- confidence: inferred
  path: experiments/effort-implement-levels.yaml
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T12:38:22Z", actor: "human:operator", revision: 4, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: effort-levels-per-class

## Outcome

The planned record `experiments/effort-implement-levels.yaml` (low, medium, high effort on the
implement class, 3 runs each) is run on the baseline release's tree and reported; further classes
follow the same way.

## Context

The effort is a run setting per node class since `24129d0` and `6c1f649` (in 0.1.0). The record
is planned with no pinned commit; it gets the baseline release's tree commit before it runs.

## Hypothesis and metric

- Hypothesis (the record's): with effort high instead of low on the implement class, the looping
  factor of `story/change.implement` falls by at least 0.5.
- Primary metric: `node.story/change.implement.looping_factor`, direction decrease, minimum effect
  0.5.
- Guards: as in the record.

## Acceptance

`sdc-experiment report effort-implement-levels` gives the record a verdict with 3 ended runs per
level, and the ledger lists it whatever the verdict.
