---
format: aep.planning-md/3
id: story:operating-record-report
kind: story
status: proposed
title: Cost and operator touches per merged item
tags:
- conductor-succession
relations:
- decomposes: epic:conductor-succession
- depends_on: story:flow-kpis
scope:
- confidence: inferred
  path: driver/src/bin/sdc-operating-record.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:25:43Z", actor: "human:operator", revision: 4, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: operating-record-report

## Outcome

A run's operating record states cost and operator touches per merged item against the thresholds
the comparison names: $11 per merged item, 0.245 operator touches per item.

## Acceptance

`sdc-operating-record <run-dir>` prints merged items, cost per merged item and operator touches per
merged item, each marked `under` or `over` its threshold, and prints the same bytes on two runs.

## Scope

A new Rust binary `sdc-operating-record` (reads the flow KPIs F13 and F16, `docs/kpis.md` § 5).

## Source to port

- takt held-out scenario runner and scorecard: `crates/takt-cli/src/main.rs:4691`,
  `crates/takt/src/scorecard.rs`, `crates/takt/src/loop_scorecard.rs`. Move: reuse as a library
  (or call the binary on SDC cases); the format `takt-scorecard/1` is already reused in
  `docs/kpis.md`. Comparison base: the 47 recorded takt scenario runs
  (`docs/metrics/scenario-runs.jsonl` in takt; `docs/takt-comparison.md` § 3). Paths are in the takt repository at `origin/main` 25c62df (tag 0.36.0), as cited there.
