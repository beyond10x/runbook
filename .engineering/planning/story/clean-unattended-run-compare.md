---
format: aep.planning-md/3
id: story:clean-unattended-run-compare
kind: story
status: proposed
title: 'Trial 3: clean unattended run, Haiku against a mixed model setting'
tags:
- conductor-succession
- eval
relations:
- decomposes: epic:conductor-succession
- depends_on: story:model-per-node-variable
- depends_on: story:decider-finding-decision
scope:
- confidence: inferred
  path: driver/src/bin/sdc-run-compare.rs
- confidence: inferred
  path: runs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:25:40Z", actor: "human:timo", revision: 3, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: clean-unattended-run-compare

## Outcome

Trial 3 answers the operator's question of 2026-11-21: is the time waiting on a person in trial 2
a defect of that run, or what a full clean run does; and does a stronger model on the writing,
implementing and reviewing nodes change the result.

## Context

Figures of trial-2-train (`docs/trial-2.md`): 15 driver legs totalling 1,009 s, wall clock 76 min
from first leg start to last leg end, so about 59 min outside the driver (waiting on a person or
stopped during repairs), although decisions were meant to go to an agent.

Runs: the same idea, end to end, at A5 with no person (decider final; findings taken by the
decider), in two settings: (a) every node on Haiku; (b) Sonnet for the specification, plan and
implementing nodes and for reviews, Haiku for the rest.

## Acceptance

Both runs close or stop with a named reason, and one report gives per setting the elapsed wall
time, the time waiting on a person (0 in both), LLM calls and cost, findings raised and how each
was resolved, rework revisions, the oracle check of `docs/eval-design.md` where one exists, and
the replay result.

## Scope

The two runs under `runs/` and a new Rust binary `sdc-run-compare` that writes the report.

## Ambiguities

- `story:requester-decider` is a dependency only if it is implemented by the time this runs; until
  then the delivering-side decider takes the requester classes (A5, decider final).
