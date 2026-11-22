---
format: aep.planning-md/3
id: story:story-branch
kind: story
status: proposed
title: Each story works on its own branch
tags:
- conductor-succession
relations:
- decomposes: epic:conductor-succession
- depends_on: story:per-story-worktree
scope:
- confidence: inferred
  path: driver/src/bin/sdc-story-branch.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:25:44Z", actor: "human:operator", revision: 4, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: story-branch

## Outcome

A story's change is a branch with commits, not a copied directory.

## Acceptance

After `change.implement` on a fixture story, its worktree is on branch `sdc/<case id>` with one or
more commits on top of the mainline commit the story started from, and `git log` names the case id.

## Scope

A new Rust binary `sdc-story-branch` (creates the branch in the story's worktree, commits the
change).

## Source to port

- takt git module: `crates/takt/src/git.rs`, `workspace.rs`, `commit_style.rs` (case branch,
  worktree, base pin). Move: port into a Rust binary here; tied to takt's case layout (*inferred* in
  `docs/takt-comparison.md` § 5 answer 2). Paths are in the takt repository at `origin/main` 25c62df (tag 0.36.0), as cited there.
