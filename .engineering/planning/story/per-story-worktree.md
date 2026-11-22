---
format: aep.planning-md/3
id: story:per-story-worktree
kind: story
status: proposed
title: Give each story case its own worktree
tags:
- orchestration
relations:
- decomposes: epic:concurrent-child-cases
scope:
- confidence: inferred
  path: driver/src/main.rs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:37Z", actor: "human:operator", revision: 3}
---
# Story: per-story-worktree

## Outcome

Two story cases of one tree cannot overwrite each other's files: each has its own managed
worktree and build directory instead of the root's shared `work/` (`docs/orchestration.md` § 2.4;
`driver/src/main.rs:235-237`; `docs/driver.md:520-522` gap 22).

## Acceptance

Two sibling `sdc.story` cases that write the same relative path leave each other's worktree
unchanged, and `git worktree list` shows one worktree per open story case.

## Scope

`driver/src/main.rs` (case workspace resolution, `sync`). TEMPORARY copy of a runtime capability
(`docs/driver.md:43-45`).

## Out of Scope

Merging the worktrees back (`story:story-merge-step`).
