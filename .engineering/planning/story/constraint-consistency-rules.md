---
format: aep.planning-md/3
id: story:constraint-consistency-rules
kind: story
status: proposed
title: Consistency rules against constraints
summary: Requirements and context are checked against the requester's constraints; the constraint is never the amended side.
tags:
- requester
relations:
- decomposes: epic:requester-side-and-goals-context
- depends_on: story:goals-root-input
scope:
- confidence: inferred
  path: driver/examples/sdc/tools/consistency-rules.json
- confidence: inferred
  path: driver/examples/sdc/tools/llm-node.sh
- confidence: inferred
  path: driver/src/bin/sdc-findings.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:35Z", actor: "human:timo", revision: 3}
---
# Story: constraint-consistency-rules

## Outcome

Requirements and context are checked against the requester's constraints: a constraint may carry a
rule in the `sdc-consistency-rules/1` shape whose side is `goals.<id>`, and stage agents may raise a
contradiction citing a goals entry; the constraint is never the side that is amended.

## Acceptance

With `product/p2` carrying a rule against "last change wins" and a requirement that says it,
`sdc-findings consistency` exits 5 with a finding citing `goals.product/p2` and that requirement,
`amend.upstream` amends the requirement and leaves `goals.yaml` unchanged, and the check exits 0
afterwards.

## Scope

`driver/src/bin/sdc-findings.rs` (read `goals.yaml`, items `goals.<id>`, constraint rank above every
other), `driver/examples/sdc/tools/consistency-rules.json` only if a shared rule is added, and the
findings section of the stage prompts in `driver/examples/sdc/tools/llm-node.sh`. Waits for the
running trial to end.

## Tooling note

- Changes the existing bash tool `driver/examples/sdc/tools/llm-node.sh`: allowed, a fix to prototype glue; no new bash tool.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)
