---
format: aep.planning-md/3
id: story:default-answers-within-build-constraints
kind: story
status: proposed
title: Non-interactive defaults pass the build constraints' answer check
summary: A default naming a technology the build constraints rule out is refused like a decider's answer (haiku-1's q9 Postgres)
tags:
- eval
- improvement-loop
relations:
- decomposes: epic:improvement-loop
- depends_on: story:build-constraints-every-stage
scope:
- confidence: cited
  path: driver/src/bin/sdc-findings.rs
- confidence: cited
  path: scripts/run-finding-cycles.sh
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T16:04:35Z", actor: "human:operator", revision: 2, executor: "agent:sdc-improve", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: default-answers-within-build-constraints

## Outcome

A default that the non-interactive rule accepts passes the same `answer_fault` check as a
decider's answer, and a default naming a language, runtime, dependency or service that the build
constraints rule out is refused: the finding goes to the decider (or the writer is sent back) with
the refusal, instead of the default becoming a requirement.

## Context

In `trial-0.1.0-haiku-1` (`experiments/baseline-0.1.0/losses.md`) `spec.author` raised finding f8
with the default "Postgres database"; the non-interactive rule accepted it as q9
(`work/questions.yaml`: `answered_by: non-interactive`, "changes no scope"), and no decider saw
it. r1 then carried q9; the implement node was sent back by the review (r1 against a data file),
by `tests.run` failed and by the review again, 1.220 USD and 976 s, and the run stopped
`writer_spent`. `fix/superseded-answers` (435ab9b, D154) adds the build constraints and
`answer_fault` (its `ruled_out` list holds `Postgres` and `PostgreSQL`), but checks the decider's
answers only (`tools/decider.sh`); the non-interactive path in `sdc-findings` does not call it.

## Smallest change

In `driver/src/bin/sdc-findings.rs`, where a finding's default is accepted as an answer without a
decider, call `answer_fault(&build_constraints, default)`; on a fault, do not accept: put the
question to the decider with the fault as its refusal (the R3 path), or mark it open. One cycle
in `scripts/run-finding-cycles.sh`: a finding whose default names Postgres is not accepted
non-interactively. Driver change; no protocol change. Depends on `story:build-constraints-every-stage`.

## Hypothesis and metric

- Hypothesis: with the defaults checked, no Haiku run ends with a requirement that names a
  ruled-out technology, and the Haiku setting's `run.clean` rises.
- Primary metric: `run.clean` of the Haiku setting (baseline 0 of 3); secondary: the count of
  accepted answers naming a `ruled_out` entry (baseline haiku-1: 1).
- Maximum gain: one of the three Haiku stops (haiku-1) and its 1.220 USD and 976 s.
- Guards: clean passes of strong and mixed, `run.decider_calls` (refused defaults now reach the
  decider), `run.cost_usd`.

## Acceptance

The cycle is green, and an experiment record with the Haiku setting, n ≥ 3, reaches a verdict by
`sdc-experiment report`.
