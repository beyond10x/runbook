---
format: aep.planning-md/3
id: story:sdc-on-loom-tree-run
kind: story
status: proposed
title: The SDC cycle runs on Loom's tree run
tags:
- blocked-upstream
- conductor-succession
- loom
relations:
- decomposes: epic:conductor-succession
- depends_on: story:bindings-to-loom-bindings-1
scope:
- confidence: inferred
  path: scripts/run-full-cycle.sh
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:25:44Z", actor: "human:operator", revision: 3, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: sdc-on-loom-tree-run

## Outcome

The eight protocols run on Loom instead of `sdc-drive`'s runtime copy (case store, event log,
selection, budgets, tool invocation: `docs/boundary.md:30-33`).

## Acceptance

`scripts/run-full-cycle.sh` drives the stub cycle through Loom's tree run with the
`loom.bindings/1` files, the root case ends delivered, and the run starts no `sdc-drive` process.

## Scope

`scripts/run-full-cycle.sh` (existing bash, a fix); the runtime is Loom's. Follow-up deletion of the
driver's TEMPORARY code: `story:loom-concurrent-children`.
