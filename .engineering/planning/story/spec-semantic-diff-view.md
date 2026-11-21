---
format: aep.planning-md/3
id: story:spec-semantic-diff-view
kind: story
status: proposed
title: ESS semantic diff beside the text diff in the live view
tags:
- later
- operator-request
relations:
- decomposes: epic:live-view
scope:
- confidence: inferred
  path: ui/server.mjs
- confidence: inferred
  path: ui/test/spec-diff.spec.mjs
- confidence: inferred
  path: ui/web/meaning.js
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:16:35Z", actor: "human:timo", revision: 3, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: spec-semantic-diff-view

## Outcome

A specification change in a run shows what it means for callers, readers and stored history, not
only which lines moved.

## Context

The operator, 2026-11-21: "in a similar way as how planning items change … and similar to how we
show diffs for code changes … for ESS, we might show a semantic diff. What has changed? Is it a
widening? Is it the opposite? … ESS has the feature of semantic diff, and it would be cool if we
could display that."

What exists (ess 0.96.0, checked 2026-11-21): `ess verify diff --from <before> --to <after>
--format json --compatibility` prints `ess-diff/14`: `before` and `after` (system,
`specification_version`, `spec_digest`) and `changes[]`, each with `id` (e.g.
`type/sdc.decision.Unblocks/field-removed/outcome`), `relation`, `change` (`category`, `subject`,
`changed.kind`, e.g. `field-removed`) and `compatibility` (`verdict` and per dimension `callers`,
`readers`, `history`: `breaking`, `unknown` or `compatible`; `uses`). `ess verify impact` reports the
conformance and generated artifacts a change invalidates. The live view's Specification class today
parses the YAML itself (`ui/README.md:52-53`).

## Acceptance

For a recorded run in which a file under an `ess/` or `spec/` specification changed, the change's
row shows beside the text diff one line per `ess-diff/14` change with its kind, subject and
compatibility verdict per dimension, produced by the server running `ess verify diff --format json
--compatibility` on the two revisions, never by an LLM.

## Scope

`ui/server.mjs` (runs `ess verify diff`), `ui/web/meaning.js`, a Playwright spec.

## Ambiguities

- "Widening" and "narrowing" are not words of the output; the change kinds (`field-removed` and
  others) and the compatibility verdicts carry that meaning. Whether ESS should name widening and
  narrowing as such is a question for the ESS session, not a blocker.
- Both revisions must be whole specification trees on disk; the run keeps file contents in
  `fs/changes.jsonl` blobs (`docs/fs-changes.md`), from which the server rebuilds them (inferable).
