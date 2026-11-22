---
format: aep.planning-md/3
id: story:goals-root-input
kind: story
status: proposed
title: Goals as an input artifact of the root case
summary: The resolved goals document is the root case's artifact goals, with a revision, read by intake and direction.
tags:
- requester
relations:
- decomposes: epic:requester-side-and-goals-context
- depends_on: story:goals-format-check
scope:
- confidence: inferred
  path: driver/examples/sdc/sdc-direction.bindings.json
- confidence: inferred
  path: driver/examples/sdc/sdc-intake.bindings.json
- confidence: inferred
  path: driver/examples/sdc/sdc.bindings.json
- confidence: inferred
  path: protocol/sdc-direction.protocol.yaml
- confidence: inferred
  path: protocol/sdc-intake.protocol.yaml
- confidence: inferred
  path: protocol/sdc.protocol.yaml
- confidence: inferred
  path: scripts/run-trial.sh
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:36Z", actor: "human:operator", revision: 3}
---
# Story: goals-root-input

## Outcome

A run receives the resolved goals document as the artifact `goals` of the root case, with a
revision, and the intake and direction children read the same file.

## Acceptance

A stub trial run started with `scripts/run-trial.sh --goals decision/examples/goals/trial-2.yaml`
records artifact `goals` in the root `case.json` with the sha256 of `work/goals.yaml` as its
revision, the intake and direction children's `work/` hold the identical file, and
`scripts/check-protocols.sh` reports unchanged state counts for all eight protocols.

## Scope

`protocol/sdc.protocol.yaml` (artifact beside `idea`, `:7-11`), the intake and direction protocols
(artifact only, no invalidation: `docs/trial-2.md:85-88`), their bindings in
`driver/examples/sdc/`, and `scripts/run-trial.sh`. Bound to `driver/`, `scripts/`, `protocol/`:
waits for the running trial to end.

## Tooling note

- Changes the existing bash tool `scripts/run-trial.sh`: allowed, a fix to prototype glue; no new bash tool.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)
