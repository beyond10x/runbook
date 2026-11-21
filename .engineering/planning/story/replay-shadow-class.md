---
format: aep.planning-md/3
id: story:replay-shadow-class
kind: story
status: proposed
title: 'sdc-replay tests one node class on another model: shadow calls and a per-class live policy'
summary: --shadow <class>=<model> checks the cheaper model's output and continues with the recorded reply; --live-class keeps other divergences from going live
tags:
- eval
- improvement-loop
relations:
- decomposes: epic:improvement-loop
scope:
- confidence: cited
  path: driver/src/bin/sdc-replay.rs
- confidence: cited
  path: scripts/run-finding-cycles.sh
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T17:45:31Z", actor: "human:operator", revision: 4, executor: "agent:sdc-improve", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: replay-shadow-class

## Outcome

`sdc-replay` can test one node class on another model without letting the rest of the run diverge:

- `--shadow <class>=<model>` calls that model live in a copy of each call's directory, runs the
  node's own checks on its `out/`, and records pass or fail, cost and tokens in `replay.json`.
  The run then continues with the recorded reply.
- `--live-class <class>` sends that class's divergences live and stops on any other divergence.

## Context

The operator, 2026-11-21: "figure out where we can use cheeper models"
(`experiments/baseline-0.1.0/models.md`). Today a model the caller sets wins
(`SDC_LLM_MODEL_<CLASS>`), and every call of that class diverges, because `model` is in the
request digest. But `--on-divergence` is global: once a writer or implement call goes live, every
later call whose inputs changed goes live too, on its source model, and is paid. So only the
review and the decider, which reach later steps through a verdict, can be tested on one class
today (record `review-haiku-replay`).

## Smallest change

- `driver/src/bin/sdc-replay.rs`:
  - the shadow call: the live adapter in a scratch copy of the call's directory;
  - the class's checks run on the copy: `spec.validate` and `spec.gate` for `spec.author`;
    `cargo build` and `tests.run` for implement;
  - `replay.json` gains `shadow: {model, checks, passed, cost_usd, tokens}`;
  - `sdc-replay seal` counts the shadow passes per class.
- `--live-class` as a second divergence rule in the same file.
- Cycles in `scripts/run-finding-cycles.sh` on the committed recording, with the `dry` adapter.

## Hypothesis and metric

- What it measures: the cheapest valid test of implement and `spec.author` on Haiku, about 15 and
  8 live calls over three strong sources, against a full run at 2 to 5 USD.
- Pass criterion of such a test: the node's checks pass at the first try at least as often as the
  recorded first tries.

## Acceptance

The new cycles pass in `scripts/run-finding-cycles.sh`. One shadow replay of a strong run with
implement on Haiku reports, per call, whether `cargo build` and `tests.run` passed.
