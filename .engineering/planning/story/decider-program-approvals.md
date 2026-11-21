---
format: aep.planning-md/3
id: story:decider-program-approvals
kind: story
status: proposed
title: Context, vision, roadmap and release approvals by program when their checks passed
summary: '25 of 25 approved in six runs: strong -0.64 USD, mixed -0.06 USD and -207 s per run'
tags:
- eval
- improvement-loop
relations:
- decomposes: epic:improvement-loop
scope:
- confidence: cited
  path: driver/examples/sdc/tools/decider.sh
- confidence: cited
  path: driver/src/main.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T17:45:32Z", actor: "human:operator", revision: 4, executor: "agent:sdc-improve", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: decider-program-approvals

## Outcome

`context.approve`, `vision.approve`, `roadmap.approve` and `release.publish` are taken by a
program rule when every claim the action requires is true, no obligation is open and no earlier
rejection exists. Otherwise the decider model is asked, as now.

## Context

The operator's target, 2026-11-21: 0.47 USD per run (`docs/improving.md`, Target).

- In the six closed baseline runs these four were approved 25 of 25 times.
- They cost 0.64 USD per strong run (Opus) and 0.06 USD and 207 s per mixed run (Haiku).
- The decider's `checks.json` already lists the claims with their values and the open obligations
  (`experiments/baseline-0.1.0/models.md`).
- `rule:autopilot` takes the close the same way today.

## Smallest change

- A rule beside the autopilot close rule in the decider's program path, for these four actions.
- Its decision is recorded in `decisions.jsonl` with principal `rule:autopilot` and the claims it
  read.
- A check that fails still reaches the model.

## Hypothesis and metric

- Offline first: the rule, applied to the 25 recorded approvals, approves exactly those.
- Then a record on the strong setting: primary `run.decider_calls`, direction decrease.
- Guards: `run.delivered`, `run.clean`, `run.cost_usd`.
- Saving per run: strong 0.64 USD, mixed 0.06 USD and 207 s.

## Acceptance

The offline comparison is in the record's facts. The record reaches a verdict by
`sdc-experiment report`, and the verdict is in the ledger.
