---
format: aep.planning-md/3
id: epic:improvement-loop
kind: epic
status: proposed
title: 'Improvement loop: one measured change per release'
summary: Each candidate is tested by an experiment record, merged when it holds, released, and entered in the progress ledger
tags:
- eval
- improvement-loop
relations:
- decomposes: initiative:roadmap
- informed_by: story:workflow-metrics
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T12:38:19Z", actor: "human:operator", revision: 2, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Epic: improvement-loop

## Outcome

The workflow improves one measured change at a time: each candidate below is tested by an
experiment record (hypothesis, one variable, at least 3 runs per variant, a decision rule fixed
before the runs, verdict held, contradicted or inconclusive), merged when it holds, released, and
entered in the progress ledger (`docs/progress.md`, `docs/img/progress.svg`) by
`sdc-experiment progress`. The procedure is `docs/improving.md`.

## Why Now

The operator, 2026-11-21: "Now steadily improve the workflow. Document your gains, document it
accurately, and prepare for putting this onto a nice image on the public documentation. Cut
releases after every improvement." Release 0.1.0 holds the first unattended passes, all on
pre-release trees and all registered retrospectively; no figure of the ledger decides anything yet.

## Scope

The candidates, in this order (each a story of this epic):

1. `story:improvement-baseline`: baseline on the first release with the launcher and the hermetic
   decider adapter.
2. `story:implement-runs-tree-check`: the implementing node may run the tree's own check.
3. `story:silent-check-output-to-writer`: no check fails without output of its own.
4. `story:build-constraints-every-stage`: build constraints known to every stage; superseded
   answers.
5. `story:merge-rework-budget-ends-by-rule`: a third merge rejection ends by rule.
6. `story:cycle-ends-at-delivered`: the cycle ends at `delivered`.
7. `story:intake-on-sonnet`: intake nodes on Sonnet against Haiku, a settings experiment.
8. `story:effort-levels-per-class`: effort levels per class (`experiments/effort-implement-levels.yaml`).

## Out of Scope

Changes without an experiment record; any run started by hand (`AGENTS.md`, Rules).

## Risks

- A record names one tree: a code change that cannot sit behind a run setting has no variable the
  launcher knows (`docs/improving.md`, step 4).
- Paid runs: about 9 runs per baseline (3 settings × 3) and 6 per two-variant experiment.

## Done When

Every story is delivered or dropped with its experiment's verdict in the ledger.
