---
format: aep.planning-md/3
id: story:late-findings-per-stage-kpi
kind: story
status: proposed
title: Late findings and rework per stage, with the stage that should have caught them
tags:
- conductor-succession
- trial-2
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: inferred
  path: docs/kpis.md
- confidence: inferred
  path: driver/src/bin/sdc-finding-kpis.rs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:36:14Z", actor: "human:timo", revision: 3, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: late-findings-per-stage-kpi

## Outcome

Each run reports where findings and rework arise, and which earlier stage should have caught each
late finding, so the eval can push them earlier.

## Context

Trial 2: 14 late findings (context 4, spec 2, story 8) and 58 rework revisions
(`docs/trial-2.md:534`, `:536`). L4 already counts late findings per stage and L6 rework per
artifact (`docs/kpis.md` § 4).

## Acceptance

`sdc-finding-kpis` on the trial-2 store prints late findings and rework revisions per stage, both
with direction down, and for each story-stage finding names the stage that should have caught it
(a goals entry or an earlier question that would have answered it), with that rule defined in
`docs/kpis.md`.

## Scope

`sdc-finding-kpis` (existing Rust binary); `docs/kpis.md` (a new row or a note under L4).
