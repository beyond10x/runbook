---
format: aep.planning-md/3
id: story:conformance-against-implementation
kind: story
status: proposed
title: Story conformance runs against the implementation
tags:
- conductor-succession
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: inferred
  path: driver/examples/sdc/sdc-story.bindings.json
- confidence: inferred
  path: driver/src/bin/sdc-conform-impl.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:25:41Z", actor: "human:timo", revision: 5, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: conformance-against-implementation

## Outcome

A story passes only when the ESS conformance suite holds against the code it delivered, not
against the specification's own interpreter.

## Context

Today `tests.run` and `acceptance.run` run the suite against `interpreted` plus the tree's own
`task check` (`docs/driver.md` gap 16); the eval's open decision D1 (`docs/eval-design.md:616`).

## Acceptance

On a fixture story whose `impl/` violates one scenario of its spec, `tests.run` records `failed`
naming that scenario, and on the corrected `impl/` it records `passed`, with the conformance
report's target naming `impl/`, not `interpreted`.

## Scope

A new Rust binary `sdc-conform-impl` (drives `impl/` for the suite and writes the report), the
story bindings.

## Ambiguities

- Runner or new ESS target: settled by the default above (section "Defaults").

## Defaults

- First a runner on the SDC side drives the generated suite against the built implementation. ESS
  already runs a suite against a target (`ess verify conform run --target …`); this story finds
  which target kinds exist and uses one that executes the implementation.
- Only if no existing target kind can execute the implementation: a request to the ESS session,
  recorded then as an `upstream-blocker` with a `blocks` edge to this story.
- Default taken by the coordinating session, 2026-11-21; the operator may change it.
