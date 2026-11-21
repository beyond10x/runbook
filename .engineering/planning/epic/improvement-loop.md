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
revision: 3
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

The candidates, in this order (each a story of this epic). Order rule, from the 0.1.0 baseline
(`experiments/baseline-0.1.0/losses.md`): first what needs no new code (a setting, then an existing
branch), then new code; within each group, a stop of the Haiku setting before money and time, then
the larger loss the change could at most remove in the nine baseline runs.

1. `story:improvement-baseline`: baseline on the first release with the launcher and the hermetic
   decider adapter (done: `experiments/baseline-0.1.0`).
2. `story:effort-levels-per-class`: effort levels per class, first the decider
   (`experiments/effort-decider-mixed.yaml`): the Haiku decider spent 717, 952 and 693 s per mixed
   run, 45 to 48 % of wall time; then the implement class (`experiments/effort-implement-levels.yaml`).
3. `story:build-constraints-every-stage`: build constraints known to every stage; superseded
   answers (branch `fix/superseded-answers`, on 0.1.2 as `fix/superseded-answers-0.1.2`): haiku-1's
   stop, 1.220 USD and 976 s of implement reruns in IDEA-1.list.shared.
4. `story:cycle-ends-at-delivered`: the cycle ends at `delivered` (branch
   `proto/cycle-ends-delivered`): the release stage is one decision, 0.158 to 0.195 USD (strong) or
   about 96 s (mixed) per run.
5. `story:default-answers-within-build-constraints`: a non-interactive default passes the same
   `answer_fault` check as a decider's answer (haiku-1's q9, which no decider saw).
6. `story:spec-author-format-on-system-source`: the `spec.author` prompt says that `format:` is
   read only on the source that declares the system (haiku-3's stop; 5 of 12 `spec.validate`
   refusals were a format version).
7. `story:implement-cites-requirements`: the implementing node names, per requirement of its
   story, the file and line that builds it, checked by program before `tests.run`: the review
   rejection loop, 5.25 USD and 1,873 s over the nine runs, the largest loss.
8. `story:implement-runs-tree-check`: the implementing node may run the tree's own check: one
   `tests.run:failed` in nine runs, 0.319 USD and 242 s.

Parked, about zero loss in the 0.1.0 baseline (each story says why):

- `story:silent-check-output-to-writer`: every content refusal of the nine runs had its own text.
- `story:merge-rework-budget-ends-by-rule`: no run had more than one merge rejection.
- `story:intake-on-sonnet`: one intake model rerun (`questions.write`, 15 s) and no stop in intake.

## Out of Scope

Changes without an experiment record; any run started by hand (`AGENTS.md`, Rules).

## Risks

- A record names one tree: a code change that cannot sit behind a run setting has no variable the
  launcher knows (`docs/improving.md`, step 4).
- Paid runs: about 9 runs per baseline (3 settings × 3) and 6 per two-variant experiment.

## Done When

Every story is delivered or dropped with its experiment's verdict in the ledger.
