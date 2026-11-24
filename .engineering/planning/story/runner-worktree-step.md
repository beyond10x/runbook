---
format: aep.planning-md/3
id: story:runner-worktree-step
kind: story
status: draft
title: A run has a managed worktree that the runner creates, finishes and collects
summary: 'A worktree step calls the host''s worktree command: create at the start of a run, finish at its end whatever the outcome, gc of finished ones; id and path in the run directory.'
tags:
- upstream-asks-2026-11-24
refs:
- provider: consumer
  reference: upstream-asks-filed/A4
relations:
- informed_by: story:per-story-worktree
- informed_by: story:story-merge-step
- informed_by: story:bugfix-hands-back-a-branch-and-a-merge-request
scope:
- confidence: inferred
  path: bindings/tools
- confidence: inferred
  path: docs/loom-bindings.md
- confidence: cited
  path: docs/loom-run.md
- confidence: inferred
  path: runbook/src/loom_run
- confidence: inferred
  path: runbook/tests/loom_run
revision: 3
---
## Outcome

A run can have a managed worktree: the runner creates it for the run, finishes it when the run ends, and
collects finished ones with the host's worktree lifecycle. A runbook author writes no shell for it.

## Source

Ask A4 of a consumer, filed in its story `upstream-asks-filed` (read 2026-11-24): "create, finish and gc a
managed worktree for a run" as a runner feature; the consumer has it 3 to 5 times in shell. The deadline,
retry and long-step parts of the same ask are `story:program-commands-have-a-deadline-and-a-retry`.

## What exists

- `story:per-story-worktree` (proposed) gives each story case of the driver its own worktree, and
  `story:story-merge-step` merges a story's branch and removes the worktree. Both are written for the old
  driver (a TEMPORARY copy of a runtime capability), for `sdc.story` cases, and neither uses a managed
  worktree command or serves another runbook.
- `story:bugfix-hands-back-a-branch-and-a-merge-request` (draft) hands a fixed bug back as a branch; it needs
  a tree to work in.

## What this story does

1. A runbook declares a worktree step (a binding or a helper beside `runbook`, the unit decides) with the
   repository and the base revision. The runner calls the host's `worktree` command: `create` at the start of
   the run with an id derived from the run's name, `finish` when the run ends (ended well, stopped, refused or
   error alike), and `gc` for finished ones.
2. The id and the path of the worktree are written in the run directory, so a later step and a reader find
   them there. A step's working directory is the worktree.
3. The runner starts only the `worktree` command for this: it removes no directory itself. A run killed
   before `finish` leaves a worktree that the host's `worktree` lifecycle (`reconcile`, `gc`) handles; the
   next start of the same run name refuses by name while that worktree exists.

## What a guard must hold

- The id is built from the run's name and checked against a fixed character set before any call; a name that
  does not fit is refused, never quoted into a command.
- The base is a revision of the named repository; the repository path is named by the operator in the
  settings, never by a submit or an intake.
- No path of the host is in an event or a record that leaves the run directory.

## Acceptance

With a stand-in `worktree` command first on `PATH` that records its arguments (no real worktree, no network):

- A run with the step calls `create` once with the id and the base, before the first step that uses the
  tree, and its record names the id and the path.
- At the end of each of ended, stopped, refused and error the stand-in saw one `finish` with the same id.
- A run with the step that is ended by SIGTERM sees `finish`; a SIGKILL sees none and the next start of that
  run name is refused naming the id.
- `gc` is called with the finished ids only and removes nothing the runner named.
- A run name outside the character set is refused before the stand-in is called.
- A runbook without the step calls `worktree` never.
- Each case runs under 5 s.

## Scope

- `runbook/src/loom_run/`, `bindings/tools/`, `runbook/tests/loom_run/`, `docs/loom-bindings.md`,
  `docs/loom-run.md`, `CHANGELOG.md`.

## Not in this story

- Merging a story's branch back (`story:story-merge-step`).
- A worktree per case of one run (`story:per-story-worktree`).
