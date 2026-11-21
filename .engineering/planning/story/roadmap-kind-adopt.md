---
format: aep.planning-md/3
id: story:roadmap-kind-adopt
kind: story
status: proposed
title: Use an AEP roadmap kind once it exists
tags:
- aep
- blocked-upstream
- orchestration
relations:
- decomposes: epic:brownfield-entry
- depends_on: story:roadmap-reconstruct
scope:
- confidence: inferred
  path: driver/examples/sdc/sdc-direction.bindings.json
- confidence: inferred
  path: driver/examples/sdc/tools/llm-node.sh
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:38Z", actor: "human:timo", revision: 3}
---
# Story: roadmap-kind-adopt

## Outcome

The SDC's roadmap is a roadmap in the store, not an `initiative` standing in for one.

## Context

Blocked upstream, AEP: no roadmap kind (`docs/orchestration.md` § 4.4 gap 1;
`docs/protocol-notes.md:192-194`).

## Acceptance

`roadmap.write` creates a `roadmap:` artifact through `aep plan artifact new`, and
`scripts/run-full-cycle.sh` exits 0 with it.

## Scope

`roadmap.write` in `llm-node.sh`, direction bindings. Capability requested from the AEP session.

## Tooling note

- Changes the existing bash tool `driver/examples/sdc/tools/llm-node.sh`: allowed, a fix to prototype glue; no new bash tool.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)
