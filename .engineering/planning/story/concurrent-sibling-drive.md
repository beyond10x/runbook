---
format: aep.planning-md/3
id: story:concurrent-sibling-drive
kind: story
status: proposed
title: Drive the siblings of a wave concurrently
tags:
- orchestration
relations:
- decomposes: epic:concurrent-child-cases
- depends_on: story:per-story-worktree
- depends_on: story:per-case-lock
- depends_on: story:concurrency-limit
scope:
- confidence: inferred
  path: driver/src/main.rs
- confidence: inferred
  path: scripts/run-trial.sh
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:35Z", actor: "human:timo", revision: 3}
---
# Story: concurrent-sibling-drive

## Outcome

Siblings opened together run together, and one stuck child no longer stops the whole run
(`docs/orchestration.md` § 1.1, § 2.4).

## Context

Today `drive` walks `children()` in sorted order depth first and stops on the first child that is
not terminable (`driver/src/main.rs:791-826`, sort `:288-293`); trial scripts loop sequentially
(`scripts/run-trial.sh:238-240`).

## Acceptance

A recorded run of a wave of two stories shows `action_started` of the second story before
`action_finished` of the first, and a child ending `stuck` leaves its sibling running to its
outcome.

## Scope

`driver/src/main.rs` (`drive`), `scripts/run-trial.sh`. TEMPORARY copy (`docs/driver.md:43-45`).

## Tooling note

- Changes the existing bash tool `scripts/run-trial.sh`: allowed, a fix to prototype glue; no new bash tool.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)
