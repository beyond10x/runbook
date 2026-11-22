---
format: aep.planning-md/3
id: story:wip-sweep-eval
kind: story
status: proposed
title: Eval sweep of the WIP limit
tags:
- orchestration
relations:
- decomposes: epic:flow-limits
- depends_on: story:flow-kpis
- depends_on: story:concurrent-sibling-drive
- depends_on: story:wip-limits-admission
scope:
- confidence: inferred
  path: driver/src/bin/sdc-wip-sweep.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:39Z", actor: "human:operator", revision: 3}
---
# Story: wip-sweep-eval

## Outcome

The default WIP limit per autonomy level is chosen by measurement (`docs/kpis.md` F17).

## Context

The WIP limit is one more factor of an eval cell beside node, level, adapter and model
(`docs/eval-design.md:436`); defaults go with the per-level limit defaults
(`docs/autonomy-levels.md:358-375`).

## Acceptance

`sdc-wip-sweep` runs the standard suite at `max_in_progress` 1, 2, 4 and 8 and writes one
report with F1-F16 per setting plus the sequential baseline, naming the setting with the best F7
and F8 at F4 = 0 and F12 = 0.

## Scope

A new `sdc-wip-sweep`.

## Tooling note

- New tool: the Rust binary `sdc-wip-sweep` (`driver/src/bin/sdc-wip-sweep.rs`), in place of `eval/wip-sweep.sh` named before.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)
