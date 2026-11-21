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
revision: 4
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

The candidates, in this order (each a story of this epic). The order follows the operator's
target, 2026-11-21: `run.cost_usd` 0.47 USD, about a tenth of today (`docs/improving.md`,
Target; `experiments/targets.yaml`). Stories are ranked by the saving per run toward it, mixed
setting first, strong in brackets (`experiments/baseline-0.1.0/costs.md`, `inputs.md`,
`models.md`). A story that a larger lever needs is ranked with that lever. Delivery and the product
check must not get worse.

0. `story:improvement-baseline`: done (`experiments/baseline-0.1.0`).
1. `story:review-minimal-facts`: the review on the smallest fact set, −0.36 (−0.46) USD per run.
   With the review on Haiku (record `review-haiku-replay`, testable today by recorded replay) up
   to −0.58 (−0.74): only the smaller input keeps every review prompt under 100K tokens. Record
   `review-minimal-facts`.
2. `story:structured-request-nodes`: one structured request per node for `spec.author`,
   `plan.decompose` and the intake and direction nodes, −0.14 (−0.30). Story 3 needs its mode.
3. `story:implement-one-request`: implement as one structured request per story with two
   follow-ups, −0.53 (−0.70). The largest input saving: conversation growth is half of
   implement's cost.
4. `story:replay-shadow-class`: one class on a cheaper model, tested by shadow calls in a replay.
   The Haiku work nodes are the only way below the floor of about 2.0 USD (up to −2.26 per mixed
   run), and this tests them without full runs.
5. `story:build-constraints-every-stage`: build constraints known to every stage
   (`experiments/build-constraints.yaml`, registered). Fixes the Haiku refusal class "a default
   against the build" (haiku-1's stop, 1.220 USD and 976 s).
6. `story:default-answers-within-build-constraints`: a non-interactive default passes the
   `answer_fault` check (haiku-1's q9). Same refusal class.
7. `story:spec-author-format-on-system-source`: the format-version refusal class, 5 of 12
   `spec.validate` refusals and haiku-3's stop; `spec.author` reruns −0.05 (−0.13).
8. `story:implement-cites-requirements`: the review rejection loop, at most −0.35 (−1.09) per run
   (5.25 USD and 1,873 s over nine runs).
9. `story:decider-program-approvals`: context, vision, roadmap and release approvals by program,
   −0.06 and −207 s (−0.64).
10. `story:effort-levels-per-class`: the Haiku decider at a lower effort
    (`experiments/effort-decider-mixed.yaml`). Wall time −450 s at most, cost about −0.1 at most
    (its thinking tokens); then the implement class.
11. `story:cycle-ends-at-delivered`: the release decision, −0.03 and 96 s (−0.18, overlaps 9).
12. `story:implement-runs-tree-check`: one `tests.run:failed` in nine runs, −0.04.

Parked, about zero loss in the 0.1.0 baseline (each story says why):

- `story:silent-check-output-to-writer`: every content refusal of the nine runs had its own text.
- `story:merge-rework-budget-ends-by-rule`: no run had more than one merge rejection.
- `story:intake-on-sonnet`: the intake nodes on Haiku already pass, at 0.04 instead of 0.36 USD
  per run.

## Out of Scope

Changes without an experiment record; any run started by hand (`AGENTS.md`, Rules).

## Risks

- A record names one tree: a code change that cannot sit behind a run setting has no variable the
  launcher knows (`docs/improving.md`, step 4).
- Paid runs: about 9 runs per baseline (3 settings × 3) and 6 per two-variant experiment.

## Done When

Every story is delivered or dropped with its experiment's verdict in the ledger.
