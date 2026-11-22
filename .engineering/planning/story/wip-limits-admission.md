---
format: aep.planning-md/3
id: story:wip-limits-admission
kind: story
status: proposed
title: Enforce WIP limits at wave admission
tags:
- orchestration
relations:
- decomposes: epic:flow-limits
- depends_on: story:orchestrate-wave-admit
- depends_on: story:flow-kpis
- depends_on: story:story-merge-step
scope:
- confidence: inferred
  path: cases/19-wip-limit-refused
- confidence: inferred
  path: driver/src/bin/sdc-wave-admit.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:39Z", actor: "human:operator", revision: 3}
---
# Story: wip-limits-admission

## Outcome

Finish before start: no new wave is admitted while unfinished work exceeds a limit
(`docs/orchestration.md` § 2.4a, rule 5 of § 2.4).

## Context

Limits `limits.wip.max_in_progress`, `max_open_worktrees`, `max_unmerged`, `max_item_age`, read
from the area's configuration by the admission program, never by an LLM. An orphan worktree is
filed into `intake` as a finding.

## Acceptance

In `cases/19-wip-limit-refused`, with `max_open_worktrees: 2` and two open story worktrees,
`admission` for a new wave is `refused` naming `max_open_worktrees` and both stories, and after
one story's merge step it is `admitted`.

## Scope

`sdc-wave-admit` (rule 5), one fixture case.

## Tooling note

- New tool: the Rust binary `sdc-wave-admit` (`driver/src/bin/sdc-wave-admit.rs`), in place of `driver/examples/sdc/tools/orchestrate-wave-admit.sh` named before.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)
