---
format: aep.planning-md/3
id: story:confined-bound-commands
kind: story
status: proposed
title: Every bound command and LLM node runs confined
tags:
- blocked-upstream
- conductor-succession
- loom
relations:
- decomposes: epic:conductor-succession
- depends_on: story:sdc-on-loom-tree-run
scope:
- confidence: inferred
  path: driver/examples/sdc
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:25:41Z", actor: "human:timo", revision: 3, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: confined-bound-commands

## Outcome

No bound command or LLM node of an SDC run can reach files or network beyond what its binding
grants.

## Context

Today `sdc-drive` starts each bound action with an inherited environment and no confinement
(`docs/layers.md` § 4 row 1). Confinement belongs to Loom and Substrate (`docs/layers.md:47-52`).

## Acceptance

In an SDC run on Loom, a bound command that writes outside its `revises` paths and one that opens a
network connection are both refused and recorded as confinement refusals, while the full cycle
still ends delivered.

## Scope

SDC bindings (paths and grants); the capability lands in Loom and Substrate.
