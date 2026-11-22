---
format: aep.planning-md/3
id: story:pilot-real-repository
kind: story
status: proposed
title: 'Pilot: real stories on a real repository at A3'
tags:
- conductor-succession
relations:
- decomposes: epic:conductor-succession
- depends_on: story:operating-record-report
- depends_on: story:conformance-against-implementation
- depends_on: story:independent-reviews
- depends_on: story:change-merge-git
scope:
- confidence: inferred
  path: runs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:25:43Z", actor: "human:operator", revision: 7, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: pilot-real-repository

## Outcome

An operating record exists: N real stories delivered on a real repository at autonomy level A3,
with cost and operator touches per merged item (`docs/eval-design.md` § 5; `docs/kpis.md` § 3).

## Acceptance

A recorded run of 10 real stories on `decider` at A3, stopped at 15 USD in total, has its
`sdc-operating-record` report checked in under `runs/`, with every merged item's cost and operator
touches stated against 11 USD and 0.245.

## Scope

A run under `runs/`; no code.

## Ambiguities

- N, repository, level and budget: settled by the defaults above (section "Defaults").

## Defaults

- N = 10 stories; repository `decider` (the brownfield dry run of `docs/orchestration.md` § 4.3
  was done on it); autonomy level A3; budget cap 15 USD in total (`docs/eval-design.md` § 5);
  thresholds 11 USD per merged item and 0.245 operator touches per item.
- Default taken by the coordinating session, 2026-11-21; the operator may change it.

## Source to port

- takt held-out scenario runner and scorecard: `crates/takt-cli/src/main.rs:4691`,
  `crates/takt/src/scorecard.rs`, `crates/takt/src/loop_scorecard.rs`. Move: reuse as a library
  (or call the binary on SDC cases); the format `takt-scorecard/1` is already reused in
  `docs/kpis.md`. Comparison base: the 47 recorded takt scenario runs
  (`docs/metrics/scenario-runs.jsonl` in takt; `docs/takt-comparison.md` § 3). Paths are in the takt repository at `origin/main` 25c62df (tag 0.36.0), as cited there.
