---
format: aep.planning-md/3
id: story:orchestrate-wave-propose
kind: story
status: proposed
title: Propose a ranked wave
tags:
- orchestration
relations:
- decomposes: epic:orchestrate-area-case
- depends_on: story:orchestrate-protocol-adopt
scope:
- confidence: inferred
  path: cases/08-orchestrate-wave-propose
- confidence: inferred
  path: driver/examples/sdc/tools/llm-node.sh
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:36Z", actor: "human:timo", revision: 3}
---
# Story: orchestrate-wave-propose

## Outcome

The LLM ranks the ready stories and gives a reason per story; a program, not the LLM, decides
what runs (`docs/orchestration.md` § 2.3).

## Context

Artifact `wave`: ranked story ids, reason per story, worktree count (§ 2.2). Input:
`aep plan artifact waves --kind story --status active --format json` (§ 2.4 rule 1).

## Acceptance

In `cases/08-orchestrate-wave-propose` over a store with three active scoped stories,
`wave.propose` writes a new `wave` revision listing ranked ids with one reason each and a worktree
count, and `wave.admitted` stays false until `wave.admit` runs.

## Scope

The `wave.propose` node in `llm-node.sh`, one fixture case.

## Tooling note

- Changes the existing bash tool `driver/examples/sdc/tools/llm-node.sh`: allowed, a fix to prototype glue; no new bash tool.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)
