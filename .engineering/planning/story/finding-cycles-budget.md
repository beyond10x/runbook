---
format: aep.planning-md/3
id: story:finding-cycles-budget
kind: story
status: proposed
title: The finding-cycles step ends within the 120 s budget
summary: '100 sequential stub cycles take 1,492-2,540 s; options: parallel through the pool, or a fast subset with the full set in a slow lane'
tags:
- test-budget
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: scripts/run-finding-cycles.sh
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T21:47:10Z", actor: "human:operator", revision: 2, executor: "agent:ci-test-budget", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: finding-cycles-budget

## Outcome

The finding-cycles step of the gate ends within the 120 s step budget. Measured on 2026-11-21 in
the merger's gate summaries: 2,540 s and 1,492 s for 100 sequential stub-adapter cycles.

Rule served (`../AGENTS.md`, "Tests stay fast"): "The test step of a repository's gate ends within
2 minutes of wall time." "A step over the budget is a defect. It gets a story and is fixed before
the next feature. Raising a timeout, adding shards or waiting longer is not a fix." "Full
regeneration from sources runs in a separate lane named `slow`, only when the sources changed".

## Scope note

`scripts/run-finding-cycles.sh` is prototype glue that Loom's `b10x-loom tree test` replaces (the
operator's goal: "runbook runs on loom entirely"). The fix is a bridge until then, not a new tool.

## Options

- (a) Run the independent stub cycles in parallel through the build pool (one `slot` job per
  cycle or per batch) until Loom's suite runner exists. Keeps all 100 cycles in the gate; the wall
  time depends on free slots, so the 120 s bound is not guaranteed on a busy machine.
- (b) A fast subset of cycles as the gate step, and the full set in a `slow` lane that runs only
  on changes under `protocol/`, `driver/` or `scripts/`.

Recommended: (b). It meets the budget whatever the pool's load, matches the rule's `slow` lane,
and needs no parallel runner in shell, which Loom's `b10x-loom tree test` replaces anyway.

## Acceptance

- The gate's finding-cycles step ends within 120 s on the workstation, measured from the job's
  start, and fails when it runs longer.
- With option (b): the `slow` lane runs the full cycle set on a change under `protocol/`,
  `driver/` or `scripts/` and is skipped on any other change; its last result is linked from the
  gate summary.

## Scope

- `scripts/run-finding-cycles.sh`, `Taskfile.yml`, the gate configuration that runs the cycles.
  Drafted only (2026-11-21): `scripts/` is being edited on `merge/queue-15`.
