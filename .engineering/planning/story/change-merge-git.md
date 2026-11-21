---
format: aep.planning-md/3
id: story:change-merge-git
kind: story
status: proposed
title: change.merge merges the story branch with git
tags:
- conductor-succession
relations:
- decomposes: epic:conductor-succession
- depends_on: story:story-branch
- depends_on: story:story-merge-step
scope:
- confidence: inferred
  path: driver/examples/sdc/sdc-story.bindings.json
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:25:40Z", actor: "human:timo", revision: 4, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: change-merge-git

## Outcome

`change.merge` is a real merge: the story branch lands on mainline as a merge commit.

## Context

Today `merge` is `cp -r impl/. mainline/` (`driver/examples/sdc/tools/sdc-tool.sh:255`).

## Acceptance

After `change.merge` on a fixture story, mainline has a `--no-ff` merge commit whose second parent
is the story branch head, and a conflicting branch leaves mainline unchanged and the action failed
naming the paths.

## Scope

The story bindings (`change.merge` calls `sdc-merge-step`).

## Source to port

- takt git module: `crates/takt/src/git.rs`, `workspace.rs`, `commit_style.rs` (case branch,
  worktree, base pin). Move: port into a Rust binary here; tied to takt's case layout (*inferred* in
  `docs/takt-comparison.md` § 5 answer 2). Paths are in the takt repository at `origin/main` 25c62df (tag 0.36.0), as cited there.
