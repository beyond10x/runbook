---
format: aep.planning-md/3
id: story:story-merge-step
kind: story
status: proposed
title: Merge a story's branch and remove its worktree
tags:
- orchestration
relations:
- decomposes: epic:concurrent-child-cases
- depends_on: story:per-story-worktree
scope:
- confidence: inferred
  path: driver/src/bin/sdc-merge-step.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:38Z", actor: "human:operator", revision: 3}
---
# Story: story-merge-step

## Outcome

A finished story's branch is merged serially `--no-ff` onto the area's wave branch with one gate,
and its worktree is removed by that merge; a conflict leaves the branch unmerged and reports it
(`docs/orchestration.md` § 2.4, § 2.5, § 2.4a; `BM:26-37`).

## Acceptance

For two story branches where the second conflicts with the first, the step merges the first,
exits non-zero for the second naming the conflicting paths, and `git worktree list` no longer
lists the first story's worktree.

## Scope

A new program `sdc-merge-step`.

## Tooling note

- New tool: the Rust binary `sdc-merge-step` (`driver/src/bin/sdc-merge-step.rs`), in place of `driver/examples/sdc/tools/merge-step.sh` named before.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)
