---
format: aep.planning-md/3
id: story:forge-pr-evidence
kind: story
status: proposed
title: Pull request state as evidence
tags:
- conductor-succession
relations:
- decomposes: epic:conductor-succession
- depends_on: story:story-branch
scope:
- confidence: inferred
  path: driver/src/bin/sdc-forge-pr.rs
- confidence: inferred
  path: protocol/sdc-story.protocol.yaml
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:25:42Z", actor: "human:operator", revision: 6, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: forge-pr-evidence

## Outcome

A story's pull request (opened, head revision) is evidence a program observed, bound to the revision
it saw.

## Acceptance

Given a recorded forge response for an open pull request, `sdc-forge-pr` writes one
`canon-evidence/1` record with result `opened` whose `subject_revision` is the pull request's head
commit, and writes none when the head differs from the story branch head.

## Scope

A new Rust binary `sdc-forge-pr`; an evidence kind in `sdc.story`.

## Ambiguities

- State budget: each kind costs at least ×4 (`docs/protocol-notes.md:138-140`); `sdc.story` is at
  12,288 states.

## Defaults

- Forge: GitHub, reached only through the bot route AGENTS.md names (`b10x-gates api` or
  `b10x-gates gh`, section "Commits"). This story reads forge state; it never writes to the forge.
  Default taken by the coordinating session, 2026-11-21; the operator may change it.

## Source to port

- takt forge adapters: `crates/takt/src/forge.rs`, `forge/{local,gitlab,connectors,command,relay}.rs`;
  PR-open validator: `crates/takt/src/validator.rs:5783`. Move: port the read side (PR state and
  head revision) into `sdc-forge-pr`. Paths are in the takt repository at `origin/main` 25c62df (tag 0.36.0), as cited there.
- Choice: done first as SDC program executors (Rust binaries in `driver/src/bin/`), with no
`loom-forge-effects` blocker. Why: these stories only read forge state (the default in
"Defaults"), and a read is an observation that a `program` executor produces in both
`sdc-drive` and `loom.bindings/1`; the comparison's "port into Loom as effects" fits the forge
writes (opening a pull request, merging on the forge), which no story here does. Those writes
become Loom effects when a story needs them. `docs/takt-comparison.md` § 5 answer 2.
