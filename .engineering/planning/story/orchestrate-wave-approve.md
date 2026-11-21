---
format: aep.planning-md/3
id: story:orchestrate-wave-approve
kind: story
status: proposed
title: Approve a wave by its revision
tags:
- orchestration
relations:
- decomposes: epic:orchestrate-area-case
- depends_on: story:orchestrate-protocol-adopt
scope:
- confidence: inferred
  path: cases/10-orchestrate-wave-approve
- confidence: inferred
  path: driver/examples/sdc/tools/decider.sh
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:36Z", actor: "human:timo", revision: 3}
---
# Story: orchestrate-wave-approve

## Outcome

The one fork the orchestrator owns, `wave.approve`, is taken by its holder at the autonomy level,
and the approval holds only for the wave revision approved (`docs/orchestration.md` § 2.3).

## Context

`wave_approval` is a revision-bound record because Canon grants are not
(`docs/protocol-notes.md:174-178`). The holder is the autonomy level
(`docs/autonomy-levels.md:208-213`).

## Acceptance

In `cases/10-orchestrate-wave-approve`, an approval recorded for `wave` revision 1 satisfies
`wave.approved`, and after `wave` moves to revision 2 `wave.approved` is false and
`wave.approve` admissible again.

## Scope

The `wave.approve` class in `decider.sh`, one fixture case.

## Ambiguities

- Which levels give `wave.approve` to the decider: requires-stakeholder-input (the operator).

## Tooling note

- Changes the existing bash tool `driver/examples/sdc/tools/decider.sh`: allowed, a fix to prototype glue; no new bash tool.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)
