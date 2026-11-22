---
format: aep.planning-md/3
id: epic:concurrent-child-cases
kind: epic
status: proposed
title: Concurrent child cases in the runtime
summary: 'Siblings of one wave run at once: one worktree and one lock per case, a merge step, and a limit on how many run.'
tags:
- orchestration
relations:
- decomposes: initiative:roadmap
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:34Z", actor: "human:operator", revision: 2}
---
# Epic: concurrent-child-cases

## Status note

Accepted by the operator, 2026-11-21: "I do accept the orchestration and parallelism design and upcoming epics and stories. So write them, scope them, implement it, but when it's time for it. So we're probably focusing first on the non-orchestration bits." Drafted 2026-11-21 under that acceptance; implementation waits until the non-orchestration work (AGENTS.md, "Work order") is done.

## Outcome

Siblings opened together are also driven together. Today `sdc-drive` walks children depth first
in sorted order, a child that is not terminable stops the whole run (`driver/src/main.rs:791-826`),
one lock covers the tree (`main.rs:108-138`) and every case shares the root's `work/`
(`main.rs:235-237`; `docs/orchestration.md` § 1.1). Done shows as a wave of two admitted
stories running at the same time in two worktrees, merged by a merge step, under a limit.

## Why Now

Parallelism is declared in the protocols and executed nowhere (§ 1.1 conclusion). Design accepted
2026-11-21 (quote above); scheduled after the non-orchestration work.

## Scope

- Per-story worktree and build directory (`worktree create`, § 2.4), per-case lock, concurrent
  sibling drive, a limit on concurrently running cases, a merge step that removes the worktree,
  standing-case runtime support (`max_runs` per subject revision, wake-up, § 3).
- These land in `driver/` as the prototype runtime, marked TEMPORARY like the child-case copy
  (`docs/driver.md:43-45`). The Loom equivalents are requested upstream
  (`story:loom-concurrent-children`, blocked).

## Out of Scope

- Admission rules (`story:orchestrate-wave-admit`), WIP limits (`epic:flow-limits`).
- Substrate confinement of the worktrees: not named by the doc.

## Risks

- Every driver story lands on `driver/src/main.rs`, so they run one after another.
- Building runtime features in the driver duplicates Loom (`docs/boundary.md:30-33`).

## Ambiguities

- Whether these driver changes are wanted at all, or only the Loom request: decided here as
  driver prototype plus Loom request, by the precedent of `docs/driver.md:43-45`; the operator can
  reverse it (requires-stakeholder-input).

## Done When

Two sibling story cases of one wave run concurrently under `sdc-drive`, each in its own
worktree, merged by the merge step, and the Loom request is filed.
