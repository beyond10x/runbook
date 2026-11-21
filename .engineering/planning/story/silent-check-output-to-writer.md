---
format: aep.planning-md/3
id: story:silent-check-output-to-writer
kind: story
status: proposed
title: No check sends a writer back without output of its own
summary: Every failing check gives its own output to the writer; the stop message names the check (audit/reject-routes 97024ce)
tags:
- eval
- improvement-loop
relations:
- decomposes: epic:improvement-loop
- depends_on: story:improvement-baseline
scope:
- confidence: inferred
  path: driver/examples/sdc/tools/llm-node.sh
- confidence: inferred
  path: driver/examples/sdc/tools/sdc-tool.sh
- confidence: inferred
  path: driver/src/main.rs
- confidence: inferred
  path: experiments
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T12:38:21Z", actor: "human:operator", revision: 6, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: silent-check-output-to-writer

## Outcome

No check that sends a writer back fails without output of its own, and the stop message of a
spent writer says which check sent it back (branch `audit/reject-routes`, `97024ce`: a writer sent
back reads the failing command's own output, R13; `5297ef0`, R9).

## Context

Corrected 2026-11-21: in `runs/trial-3-haiku-8` a failed check's own output did reach the writer.
What was wrong was the stop message, and several checks that fail without any output of their own,
so the writer was sent back with nothing but a line of the build pool. The expected gain is modest.

## Hypothesis and metric

- Hypothesis: with every failing check giving its own output, the writer runs sent back by a check
  that gave none fall.
- Primary metric: the rework takes of writers whose cause is one of the checks that failed without
  output before `97024ce` (their check edges, `edge.<stage>: <check>:failed -> <writer>.count`),
  direction decrease.
- Guards: the clean-pass count on Haiku, `run.cost_usd`.

## Acceptance

An experiment record with this hypothesis and at least 3 runs per variant reaches a verdict by
`sdc-experiment report`, and the ledger lists it whatever the verdict.
