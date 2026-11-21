---
format: aep.planning-md/3
id: story:intake-on-sonnet
kind: story
status: proposed
title: Intake nodes on Sonnet against Haiku, a settings experiment
summary: One variable, the intake nodes' model, everything else fixed; primary metric the first-pass yield of spec.author
tags:
- eval
- improvement-loop
relations:
- decomposes: epic:improvement-loop
- depends_on: story:improvement-baseline
- informed_by: story:model-per-node-variable
scope:
- confidence: inferred
  path: driver/examples/sdc/tools/llm-node.sh
- confidence: inferred
  path: experiments
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T12:38:22Z", actor: "human:operator", revision: 4, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: intake-on-sonnet

## Outcome

A settings experiment: the intake nodes on Sonnet against Haiku, everything else fixed, on the
baseline release's tree.

## Context

The retrospective figures show the first-pass yield of `spec.author` at 0 with Haiku intake and 1
in the strong setting, with too few runs to claim anything (`docs/progress.md`). The settings today
set a model per class (writer, implement, review, other: `docs/driver.md`, "Model per node"), not
per stage; whether the intake nodes can be varied alone is the first question
(`story:model-per-node-variable`).

## Hypothesis and metric

- Hypothesis: with the intake nodes on Sonnet, the first-pass yield of `spec/spec.author` rises.
- Primary metric: `node.spec/spec.author.first_pass_yield`, direction increase.
- Guards: `run.cost_usd` of a clean pass, the clean-pass count.

## Acceptance

An experiment record whose one variable is the intake nodes' model, with at least 3 runs per
variant, reaches a verdict by `sdc-experiment report`.

## Parked by the 0.1.0 baseline

Parked (2026-11-21), not dropped. In the nine runs of `experiments/baseline-0.1.0` the intake
stage had one model rerun (`questions.write` in strong-1, 15 s, no cost worth a line) and no stop;
none of the three Haiku stops was in intake (`experiments/baseline-0.1.0/losses.md`). The loss
this story addresses directly is about 0. Its indirect claim, fewer late findings sent back to
intake (`finding@spec -> intake` 19 in the Haiku runs against 4 in the strong ones), is not
attributable to the intake model from these runs, since the writer model changed with it.
