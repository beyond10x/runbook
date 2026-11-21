---
format: aep.planning-md/3
id: story:docs-evaluation
kind: story
status: proposed
title: 'Evaluation: every metric per release'
summary: The cases, the settings and rules, one chart per metric over the releases, and every run
tags:
- docs
relations:
- decomposes: epic:public-docs-site
scope:
- confidence: cited
  path: driver/src/bin/sdc-experiment/progress.rs
- confidence: cited
  path: website/src/components/MetricChart.tsx
- confidence: cited
  path: website/src/components/evaluation.ts
- confidence: cited
  path: website/src/routes/Evaluation.tsx
- confidence: cited
  path: website/tests/fixtures/ledger
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T15:09:50Z", actor: "human:operator", revision: 2, executor: "agent:sdc-docs-site", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: docs-evaluation

## Outcome

The evaluation page describes the cases (the to-do idea, the bug-fix seeds, the settings and
which class runs on which model, how a run is started, a clean pass, what stops a run, n and the
rule for a difference) and draws one chart per metric over the releases: the median per setting
with a min-max whisker and n, the direction in words, "no data" for a release without runs, and
a per-run table per release.

## Context

The operator, 2026-11-21: "draw the first dot in the graph ... throughout the improvement ... we
will then see the metrics improve." Each run in `docs/progress.json` carries its metrics
(`sdc-experiment progress`, `run_metrics_j`).

## Acceptance

The fixture build (`SDC_SITE_FIXTURES=tests/fixtures/ledger`, three releases, one without runs)
passes `tests/fixture.spec.ts`: the cost chart's releases read 0.1.0, 0.1.1, 0.2.0 and 0.1.1
says "no data".

## Scope

`website/src/routes/Evaluation.tsx`, `website/src/components/evaluation.ts`,
`website/src/components/MetricChart.tsx`, `website/tests/fixtures/ledger/`,
`driver/src/bin/sdc-experiment/progress.rs`.
