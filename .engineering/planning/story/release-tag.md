---
format: aep.planning-md/3
id: story:release-tag
kind: story
status: proposed
title: release.publish tags the released commit
tags:
- conductor-succession
relations:
- decomposes: epic:conductor-succession
- depends_on: story:change-merge-git
scope:
- confidence: inferred
  path: driver/examples/sdc/sdc-release.bindings.json
- confidence: inferred
  path: driver/src/bin/sdc-release-tag.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:25:43Z", actor: "human:timo", revision: 4, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: release-tag

## Outcome

A release is a tag on a mainline commit, not a copied directory.

## Context

Today `publish` copies `release/` to `published/` (`driver/examples/sdc/tools/sdc-tool.sh:260`).

## Acceptance

After `release.publish` on a fixture, an annotated tag `v<version>` points at the merged mainline
commit and the `release_record` names the tag and its commit.

## Scope

A new Rust binary `sdc-release-tag`; the release bindings.

## Source to port

- takt git module: `crates/takt/src/git.rs`, `workspace.rs`, `commit_style.rs` (case branch,
  worktree, base pin). Move: port into a Rust binary here; tied to takt's case layout (*inferred* in
  `docs/takt-comparison.md` § 5 answer 2). Paths are in the takt repository at `origin/main` 25c62df (tag 0.36.0), as cited there.
