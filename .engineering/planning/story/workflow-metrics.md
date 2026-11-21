---
format: aep.planning-md/3
id: story:workflow-metrics
kind: story
status: proposed
title: Workflow metrics per node and edge, and A/B experiment records
summary: Objective workflow quality from the run records; adopt a change only on a pre-registered A/B experiment
tags:
- conductor-succession
- eval
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: docs/workflow-metrics.md
- confidence: cited
  path: driver/src/bin/sdc-experiment.rs
- confidence: cited
  path: driver/src/bin/sdc-run-metrics.rs
- confidence: cited
  path: driver/src/run_metrics.rs
- confidence: cited
  path: driver/tests/run_metrics.rs
- confidence: cited
  path: experiments
- confidence: cited
  path: scripts/check-bindings.sh
- confidence: cited
  path: scripts/run-finding-cycles.sh
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T11:09:58Z", actor: "human:timo", revision: 3, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: workflow-metrics

## Outcome

The workflow's quality is measured from the records of each run: per node, per edge, per
decision class, per stage and per run. A change to the workflow is adopted only on an A/B
experiment with a hypothesis written before its runs.

## Context

The operator, 2026-11-21: "Building these workflows has a lot to do with the eval actually testing
the workflow. During that testing we need a way to assess objectively the quality of the
workflow. Besides total duration: retries or reattempts or looping factor… these metrics are also
local in some way, on the specific nodes or edges. Based on those and how they change from trial
to trial, where we just change small things: we could run the same thing three times with a
different effort level and measure what's happening, and repeat that. Given those metrics we
derive from the transcript and all the tracking we have, we form a hypothesis, make an adjusted
change and run an A/B test, see whether it's true or not, and then adopt the change." No LLM
judges anything here.

Built on `feat/run-metrics`:

- `sdc-run-metrics` (`sdc.run-metrics/1`);
- `sdc-experiment compare|check|report`;
- `experiments/<id>.yaml` (`sdc.experiment/1`);
- `docs/workflow-metrics.md`.

## Acceptance

`sdc-run-metrics` on a run directory gives:

- per node: takings, real takes, not-called runs, retries by cause, looping factor, first-pass
  yield, seconds, cost, tokens, model and effort;
- per edge: count and loop cost;
- per decision class: asked, approved, rejected, suppressed, invalid, retried, refused;
- per run: end, clean, furthest stage and waiting time.

Every real run belongs to an experiment record. The operator, 2026-11-21: "make sure to organize
these trials in a somewhat scientific and organized, manageable and scaling manner. It shouldn't
end up messy."

- `sdc-experiment run <id>` starts a prospective record's variants × n runs from its pinned
  commit, built once, with the record's settings only, names them `<id>.<variant>.<n>` and writes
  `experiment.json` into each; `status` and `stop` list and stop them.
- `sdc-experiment report` writes per ended run a sanitized summary, the record's result (held,
  contradicted or inconclusive by the complete-separation rule; descriptive without a
  hypothesis), `experiments/<id>/report.md` and `experiments/INDEX.md`.
- `sdc-experiment check --all`, run by `scripts/check-bindings.sh`, fails on an invalid record or
  a stale report, verdict or index.
- The runs of 2026-11-21 are registered retrospectively (trial-3-attempts, trial-3-batch-1,
  trial-3-main-1, small-idea-1, bugfix-seeds-1), with two hypothesis records. The prospective
  effort record (`experiments/effort-implement-levels.yaml`) is run once `feat/effort-per-class`
  is on main.

## Scope

- `driver/src/run_metrics.rs`, `driver/src/bin/sdc-run-metrics.rs`, `driver/src/bin/sdc-experiment.rs`
- `driver/tests/run_metrics.rs`, `scripts/run-finding-cycles.sh` (cycle `run-metrics`), `scripts/check-bindings.sh`
- `experiments/`, `docs/workflow-metrics.md`, `AGENTS.md` (Rules)
