---
format: aep.planning-md/3
id: story:independent-reviews
kind: story
status: proposed
title: Reviews by an independent principal that reads diff and spec
tags:
- conductor-succession
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: inferred
  path: driver/examples/sdc/sdc-spec.bindings.json
- confidence: inferred
  path: driver/examples/sdc/sdc-story.bindings.json
- confidence: inferred
  path: driver/src/bin/sdc-review-check.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:25:42Z", actor: "human:operator", revision: 4, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: independent-reviews

## Outcome

`change.review` and `spec.review` are judgements by a principal other than the writer, grounded in
the diff and the specification; an existence check no longer counts as a review.

## Context

Reviews are `test -f` checks by `reviewer-stub` (`docs/driver.md` gap 17).

## Acceptance

On a fixture change, the `code_review` record names a principal different from the implementer and
cites at least one diff hunk and one spec declaration, and a review record without those citations
is refused by `sdc-review-check`, leaving `change.reviewed` false.

## Scope

A new Rust binary `sdc-review-check`; the review bindings of story and spec.

## Source to port

- takt gate validator: `crates/takt/src/validator.rs:2482`, `:4983`, `:5264` (`prepare_with`) and
  `scripts/gate-exec`. Move: port. Waits on the data-only catalog change in engineering-protocols
  (the assertions' observation collection moves into Loom executors,
  `docs/engineering-protocols-boundary.md:1-10`). It covers tests and the policy gate, not ESS
  conformance. Paths are in the takt repository at `origin/main` 25c62df (tag 0.36.0), as cited there.
