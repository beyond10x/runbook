---
format: aep.planning-md/3
id: story:forge-release-evidence
kind: story
status: proposed
title: Release published as evidence
tags:
- conductor-succession
relations:
- decomposes: epic:conductor-succession
- depends_on: story:release-tag
scope:
- confidence: inferred
  path: driver/src/bin/sdc-forge-release.rs
- confidence: inferred
  path: protocol/sdc-release.protocol.yaml
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:25:42Z", actor: "human:timo", revision: 5, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: forge-release-evidence

## Outcome

That a release was published on the forge is evidence about the tagged commit.

## Acceptance

Given a recorded forge response for a published release, `sdc-forge-release` writes one
`canon-evidence/1` record `published` whose `subject_revision` is the tag's commit, and `failed`
when the release names another commit.

## Scope

A new Rust binary `sdc-forge-release`; an evidence kind in `sdc.release`.

## Defaults

- Forge: GitHub, reached only through the bot route AGENTS.md names (`b10x-gates api` or
  `b10x-gates gh`, section "Commits"). This story reads forge state; it never writes to the forge.
  Default taken by the coordinating session, 2026-11-21; the operator may change it.

## Source to port

- takt forge adapters: `crates/takt/src/forge.rs` and `forge/*.rs`; takt's forge seam covers
  everything but release (`docs/takt-comparison.md` § 2, phases row), so no release reader exists
  to port: only the adapter plumbing. Merge validator `crates/takt/src/validator.rs:6655` belongs
  to the forge merge, a write. Paths are in the takt repository at `origin/main` 25c62df (tag 0.36.0), as cited there.
- Choice: done first as SDC program executors (Rust binaries in `driver/src/bin/`), with no
`loom-forge-effects` blocker. Why: these stories only read forge state (the default in
"Defaults"), and a read is an observation that a `program` executor produces in both
`sdc-drive` and `loom.bindings/1`; the comparison's "port into Loom as effects" fits the forge
writes (opening a pull request, merging on the forge), which no story here does. Those writes
become Loom effects when a story needs them. `docs/takt-comparison.md` § 5 answer 2.
