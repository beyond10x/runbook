---
format: aep.planning-md/3
id: story:the-context-copy-skips-git-and-excluded-paths
kind: story
status: draft
title: The copy of a context into a run's workspace skips .git and excluded paths, within a bound
summary: copy_dir leaves out .git and an exclude list (target/, node_modules/ by default) and is bounded in entries and bytes
tags:
- consumer-asks-2026-11-27-agents
relations:
- informed_by: story:model-call-does-not-copy-the-tree
- informed_by: story:daemon-takes-a-delegation
revision: 1
---
## Outcome

The copy of a context directory into a run's workspace leaves out `.git` and every path an exclude list names
(`target/`, `node_modules/` by default). It is bounded in entries and bytes like the daemon's other copy. A consumer
can name a checkout as a delegation's `workspace` instead of exporting one.

## Context

Gap G6 of a consumer repository that dispatches agents through the daemon (2026-11-27, relayed by the host's
operator session).

- `copy_dir` (`runbook/src/commands.rs:3088-3101`) copies the context tree whole. It already skips a symbolic link:
  it copies only `is_dir` and `is_file` of `file_type`, which does not follow a link (doc comment
  `runbook/src/commands.rs:3087-3088`). So the "skip symlinks" part of the ask holds today. It has no exclusion, no
  bound and no depth limit.
- The run's thread calls it before the run starts (`runbook/src/commands.rs:2649-2650`). A failure ends the start as
  "the context could not be copied into the workspace".
- The daemon's copy of a runbook is bounded: `MAX_ENTRIES` 4096, `MAX_BYTES` 16 MiB, `MAX_DEPTH` 24
  (`runbook/src/engine/copy.rs:15-24`). The context copy has none of these.
- The free run's agent node then copies every top-level entry of the workspace again into the call's `in/` directory
  (`driver/src/bin/sdc-node/nodes.rs:185-190`, `Inputs::input` at `driver/src/bin/sdc-node/nodes.rs:26-38`). A `.git`
  or `target/` that came with the context is copied twice.

## What this story does

1. `copy_dir` leaves out:
   - every entry named `.git`, as a directory or a file (a linked worktree's `.git` is a file);
   - every path that matches the exclude list.

   It still skips links, as today.
2. The exclude list is the daemon's: `[delegation] copy_exclude = ["target/", "node_modules/"]` in `runbook.toml`,
   with those two as the default when the key is absent. Each entry is a directory name matched at any depth, or a
   relative path from the context root. A submit or a delegation may add names with `workspace_exclude` but may not
   remove the daemon's.
3. The copy is bounded by entries and bytes, from keys of the same table with defaults the unit measures. A context
   over a bound ends the start with a refusal that names the bound, and copies nothing.
4. The run's record names the excluded names and the number of entries and bytes it copied.

## Acceptance

- A context tree with `.git/`, a `.git` file, `target/`, `a/node_modules/`, a link and `src/lib.rs`: the workspace
  holds `src/lib.rs` only.
- `copy_exclude = ["build/"]` in the file: `build/` is left out, and `target/` is copied.
- A delegation with `workspace_exclude: ["fixtures/"]` leaves out `fixtures/` as well.
- A context over the entry bound ends the start with a refusal naming the bound, and the workspace holds nothing of it.
- Each case runs under 5 s on a fixture of at most a few hundred files.

## Evidence

- The consumer's design, section 8, item G6: `copy_dir` copies a delegation workspace whole, so the consumer exports
  a checkout before it delegates.

## Relations

- Informed by `story:model-call-does-not-copy-the-tree` (the per-call copy of `sdc-node`) and
  `story:daemon-takes-a-delegation` (`workspace`).
- `story:a-delegation-changes-a-repository-on-its-own-branch` gives a change agent a worktree in place. This story
  is for the read-only context copy.
