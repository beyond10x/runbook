---
format: aep.planning-md/3
id: story:improvement-baseline
kind: story
status: proposed
title: Baseline on the first release with the launcher and the hermetic decider
summary: A descriptive record with at least 3 runs per setting on that release's tree, entered in the ledger
tags:
- eval
- improvement-loop
relations:
- decomposes: epic:improvement-loop
- informed_by: story:clean-unattended-run-compare
- informed_by: story:workflow-metrics
scope:
- confidence: inferred
  path: docs/progress.md
- confidence: inferred
  path: experiments
- confidence: inferred
  path: experiments/releases.yaml
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T12:38:20Z", actor: "human:operator", revision: 5, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: improvement-baseline

## Outcome

A prospective, descriptive experiment record on the tree of the first release that has both the
launcher (`sdc-experiment run`, branch `feat/run-metrics`) and the hermetic `claude-cli` decider
adapter (`f747085`, in 0.1.0), run with at least 3 runs in each of the settings `haiku`, `mixed`
and `strong` of `experiments/releases.yaml`, and entered in the ledger as that release's figures.

## Context

0.1.0 has the hermetic adapter but not the launcher, so the first release with both is the one
after it. Every figure of the ledger so far comes from retrospective records on pre-release trees
with one or two runs per setting (`docs/progress.md`). The candidates after this one compare
against this baseline.

## Hypothesis and metric

- No hypothesis: the record is descriptive (`decision_rule: descriptive`).
- Primary figure: clean passes of ended runs per setting, n ≥ 3 each; with it
  `run.cost_usd` and `run.wall_s` of the clean passes, and the looping factor and first-pass yield
  of `story/change.implement` and `spec/spec.author`, the nodes the later candidates target.

## Acceptance

`sdc-experiment check --all` passes with the record complete and every setting at 3 or more ended
runs, and the release's entry in `docs/progress.md` names the record and gives a rate for each of
the three settings.
