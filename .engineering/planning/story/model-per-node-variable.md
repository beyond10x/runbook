---
format: aep.planning-md/3
id: story:model-per-node-variable
kind: story
status: proposed
title: Model per LLM node is a run setting and an eval variable
tags:
- conductor-succession
- eval
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: inferred
  path: driver/examples/sdc/tools/decider.sh
- confidence: inferred
  path: driver/examples/sdc/tools/llm-node.sh
- confidence: inferred
  path: eval
- confidence: inferred
  path: scripts/run-trial.sh
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:25:43Z", actor: "human:timo", revision: 3, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: model-per-node-variable

## Outcome

Which model runs each LLM node, and the decider, is a setting of the run, recorded with the run and
varied by the eval, so a quality question about a model can be answered by measurement.

## Context

The operator, 2026-11-21, on the live view: "you're using Haiku a lot; is that a quality issue?"
Figures of the trials (`docs/trial-2.md`): trial-todo-1 made 15 work calls, 280 s of LLM time, all
on `claude-haiku-5-5`; trial-2-train so far 26 work calls, 947 s of LLM time. The model is a
binding value today (`llm.model`, `docs/driver.md` "LLM executor") and an eval factor M1/M2
(`docs/eval-design.md:436`).

## Acceptance

A run started with one model for all nodes, and a run started with a model per node class
(specification and plan writers, the implementing node, reviewers, the decider), each record the
resolved model per node in the run's settings record and in every call line, and a run with no
setting uses each binding's `llm.model` unchanged.

## Scope

`driver/examples/sdc/tools/llm-node.sh`, `driver/examples/sdc/tools/decider.sh` and
`scripts/run-trial.sh` (existing bash tools: fixes allowed, no new bash tool), `eval/` (the model
setting as a cell factor; KPIs reported per setting).

## Ambiguities

- The settings record's name and place: *inferred* as a run event beside `autonomy_resolved`
  (`docs/autonomy-levels.md:393`); inferable once the run-settings record exists.
