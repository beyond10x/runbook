---
format: aep.planning-md/3
id: story:forge-ci-evidence
kind: story
status: proposed
title: CI conclusion as evidence
tags:
- conductor-succession
relations:
- decomposes: epic:conductor-succession
- depends_on: story:forge-pr-evidence
scope:
- confidence: inferred
  path: driver/src/bin/sdc-forge-ci.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:25:42Z", actor: "human:timo", revision: 5, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: forge-ci-evidence

## Outcome

The CI conclusion for a story's head revision is evidence, so a red CI keeps the story from merging.

## Acceptance

Given recorded CI responses, `sdc-forge-ci` writes one `canon-evidence/1` record `passed` or
`failed` for the head commit it observed, and none while the run is still pending.

## Scope

A new Rust binary `sdc-forge-ci` (evidence kind declared with `forge-pr-evidence`'s protocol change).

## Defaults

- Forge: GitHub, reached only through the bot route AGENTS.md names (`b10x-gates api` or
  `b10x-gates gh`, section "Commits"). This story reads forge state; it never writes to the forge.
  Default taken by the coordinating session, 2026-11-21; the operator may change it.

## Source to port

- takt CI-read validator: `crates/takt/src/validator.rs:6434`; forge adapters as in
  `story:forge-pr-evidence`. Move: port into `sdc-forge-ci`. Paths are in the takt repository at `origin/main` 25c62df (tag 0.36.0), as cited there.
- Choice: done first as SDC program executors (Rust binaries in `driver/src/bin/`), with no
`loom-forge-effects` blocker. Why: these stories only read forge state (the default in
"Defaults"), and a read is an observation that a `program` executor produces in both
`sdc-drive` and `loom.bindings/1`; the comparison's "port into Loom as effects" fits the forge
writes (opening a pull request, merging on the forge), which no story here does. Those writes
become Loom effects when a story needs them. `docs/takt-comparison.md` § 5 answer 2.
