---
format: aep.planning-md/3
id: story:build-constraints-every-stage
kind: story
status: proposed
title: Build constraints known to every stage; superseded answers
summary: The build constraints are one run file read by every writer and held by requirements.check (fix/superseded-answers 6c74bf1)
tags:
- eval
- improvement-loop
relations:
- decomposes: epic:improvement-loop
- depends_on: story:improvement-baseline
- informed_by: story:intake-asks-what-stories-will-need
- informed_by: story:constraint-consistency-rules
scope:
- confidence: inferred
  path: driver/examples/sdc/tools/llm-node.sh
- confidence: inferred
  path: driver/src/bin/sdc-findings.rs
- confidence: inferred
  path: experiments
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T12:38:21Z", actor: "human:operator", revision: 5, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: build-constraints-every-stage

## Outcome

The constraints of the build (what the build can and cannot provide, for example no hosted
database) are one run file read by every writer and held by `requirements.check`, and a decision
that replaces an answer supersedes it (branch `fix/superseded-answers`, `6c74bf1`; the
supersession part, `460b486` and `b4aa0d1`, is in 0.1.0).

## Context

`trial-3-mixed-8` stopped `rework_spent` after the decider rejected a merge three times for a
hosted managed database against a local data file (r5), while the review approved every call
(`experiments/trial-3-main-2.yaml`, facts). `trial-3-haiku-4` was rejected for a data file instead
of Postgres (`experiments/trial-3-attempts.yaml`, facts).

## Hypothesis and metric

- Hypothesis: with the build constraints known to every stage, merge rejections caused by a
  requirement the build cannot meet go to zero.
- Primary metric: `decision.approval/merge.approve.rejected` of rejections that cite a requirement
  in conflict with a build constraint, direction decrease to 0. Open: that count needs a program
  that joins a rejection's cited requirement ids with the constraints file; no LLM classifies
  reasons.
- Guards: the clean-pass count, `run.cost_usd`.

## Acceptance

An experiment record with this hypothesis and at least 3 runs per variant reaches a verdict by
`sdc-experiment report` from a metric a program computes.
