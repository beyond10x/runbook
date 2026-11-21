---
format: aep.planning-md/3
id: story:requester-decider-loom
kind: story
status: proposed
title: Requester decider configuration on Loom
summary: The host passes a decider configuration per class to Loom; the trial script stops routing classes.
tags:
- blocked-upstream
- loom
- requester
relations:
- decomposes: epic:requester-side-and-goals-context
- depends_on: story:requester-decider
scope:
- confidence: inferred
  path: driver/examples/sdc/tools/decider.sh
- confidence: inferred
  path: scripts/run-trial.sh
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:38Z", actor: "human:timo", revision: 3}
---
# Story: requester-decider-loom

## Outcome

The requester decider runs on Loom: the host passes, per decision class, a decider configuration
(principal, inputs, prompt) and the eligible principals of the class's side, plus the resolved goals
document, and `scripts/run-trial.sh` no longer routes classes itself.

## Acceptance

A run on Loom of the trial-2 fixture at A3 decides its scope-changing questions through the decider
configuration the host passed for class `scope_changing_answer`, recorded with principal
`decider:requester` and `goals.yaml` among its inputs, while `scripts/run-trial.sh` contains no
per-class routing.

## Scope

The SDC host configuration passed to Loom and the removal of routing from `scripts/run-trial.sh`
and `driver/examples/sdc/tools/decider.sh`. Blocked upstream: `upstream-blocker:loom-decider-per-class`.

## Tooling note

- Changes the existing bash tool `driver/examples/sdc/tools/decider.sh`: allowed, a fix to prototype glue; no new bash tool.
- Changes the existing bash tool `scripts/run-trial.sh`: allowed, a fix to prototype glue; no new bash tool.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)
