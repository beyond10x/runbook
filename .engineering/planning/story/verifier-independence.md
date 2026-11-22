---
format: aep.planning-md/3
id: story:verifier-independence
kind: story
status: proposed
title: A story cannot pass on its implementer's own evidence
tags:
- conductor-succession
relations:
- decomposes: epic:conductor-succession
- depends_on: story:conformance-against-implementation
- depends_on: story:independent-reviews
scope:
- confidence: inferred
  path: protocol/compiled/sdc-story.ir.json
- confidence: inferred
  path: protocol/sdc-story.protocol.yaml
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:25:45Z", actor: "human:operator", revision: 4, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: verifier-independence

## Outcome

The verifier hole is closed: no story reaches `passed` on evidence produced by its own
implementing step.

## Context

The eval's scripted false done (`story-readme` adversary, `eval/run-adversaries.sh:71`) passes
today: R2 scripted false done = 1 (`docs/eval-design.md:616`).

## Acceptance

`eval/run-adversaries.sh` with `story-readme` ends its story case not `passed`, and the eval report
shows R2 scripted false done = 0.

## Scope

`sdc.story` protocol (producer independence of the verification kinds) and its IR.

## Source to port

- takt gate validator: `crates/takt/src/validator.rs:2482`, `:4983`, `:5264` (`prepare_with`) and
  `scripts/gate-exec`. Move: port. Waits on the data-only catalog change in engineering-protocols
  (the assertions' observation collection moves into Loom executors,
  `docs/engineering-protocols-boundary.md:1-10`). It covers tests and the policy gate, not ESS
  conformance. Paths are in the takt repository at `origin/main` 25c62df (tag 0.36.0), as cited there.
