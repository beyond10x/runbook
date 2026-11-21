---
format: aep.planning-md/3
id: story:orchestrate-intake-triage
kind: story
status: proposed
title: Route and check intake entries
tags:
- orchestration
relations:
- decomposes: epic:orchestrate-area-case
- depends_on: story:orchestrate-protocol-adopt
scope:
- confidence: inferred
  path: cases/07-orchestrate-intake
- confidence: inferred
  path: driver/examples/sdc/tools/llm-node.sh
- confidence: inferred
  path: driver/src/bin/sdc-intake-check.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:36Z", actor: "human:timo", revision: 3}
---
# Story: orchestrate-intake-triage

## Outcome

Every arrival (idea text, ticket ref, planning-store id, finding, alert) is routed by
`intake.route` (LLM, writes the routing) and checked by `intake.check` (program, records `triage`)
(`docs/orchestration.md` § 2.1, § 2.2, § 4.1).

## Context

`triage` has no result field; it is written only when every entry is routed (§ 2.2 table). A new
`intake` revision makes it inapplicable by revision binding (§ 2.5).

## Acceptance

In `cases/07-orchestrate-intake` with three entries (idea, bug, `story:` id), `intake.triaged`
holds only after all three are routed, and appending a fourth entry makes `intake.route`
admissible again.

## Scope

A new program `sdc-intake-check`, the `intake.route` node in `llm-node.sh`, one fixture case.

## Out of Scope

The brownfield "reverse-engineer this area" entry (`story:intake-reconstruct`).

## Tooling note

- New tool: the Rust binary `sdc-intake-check` (`driver/src/bin/sdc-intake-check.rs`), in place of `driver/examples/sdc/tools/orchestrate-intake-check.sh` named before.
- Changes the existing bash tool `driver/examples/sdc/tools/llm-node.sh`: allowed, a fix to prototype glue; no new bash tool.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)
