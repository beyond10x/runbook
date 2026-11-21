---
format: aep.planning-md/3
id: story:merge-rework-budget-ends-by-rule
kind: story
status: proposed
title: A third merge rejection ends by rule
summary: The third rejection of a merge ends the story on a rework_budget record instead of stopping the run (proto/rework-budget)
tags:
- eval
- improvement-loop
relations:
- decomposes: epic:improvement-loop
- depends_on: story:improvement-baseline
- informed_by: story:release-rejection-plans-fix-stories
scope:
- confidence: inferred
  path: driver/src/main.rs
- confidence: inferred
  path: experiments
- confidence: inferred
  path: protocol/sdc-story.protocol.yaml
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T12:38:21Z", actor: "human:operator", revision: 5, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: merge-rework-budget-ends-by-rule

## Outcome

A third rejection of a merge ends the story by rule on a `rework_budget` record (branch
`proto/rework-budget`, `7bc2d9c`, D152, R4) instead of stopping the run, so the run continues to
a truthful outcome.

## Context

Today a third merge rejection stops the run `rework_spent` (`trial-3-mixed-8`,
`experiments/trial-3-main-2.yaml`); a stopped run is no clean pass and no close.

## Hypothesis and metric

- Hypothesis: runs that stop with `rework_spent` today end closed `not_delivered` instead.
- Primary metric: the count of runs ending `stopped: rework_spent` (`run.end`), direction decrease
  to 0, read with the count ending `closed: not_delivered`.
- Guards: the clean-pass count, `run.cost_usd`, `run.wall_s`.

## Acceptance

An experiment record with this hypothesis and at least 3 runs per variant reaches a verdict by
`sdc-experiment report`, and the ledger lists it whatever the verdict.

## Parked by the 0.1.0 baseline

Parked (2026-11-21), not dropped. In the nine runs of `experiments/baseline-0.1.0` no run had
more than one merge rejection (mixed-2 and haiku-1 one each, both reworked and approved), so the
third rejection this story ends by rule never happened: the loss it addresses there is 0 USD and
0 s (`experiments/baseline-0.1.0/losses.md`). 0.1.2 has one effective rework cap (D184). It comes
back when a run reaches a second merge rejection of one case.
