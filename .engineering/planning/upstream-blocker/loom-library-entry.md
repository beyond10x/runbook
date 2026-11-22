---
format: aep.planning-md/3
id: upstream-blocker:loom-library-entry
kind: upstream-blocker
status: open
title: 'Loom: library entry points for an in-process runtime (tree run, event sink, answer, cancel, confinement, agent node, suite runner)'
tags:
- loom
- one-binary
relations:
- blocks: story:engine-on-loom-library-entry
revision: 2
---
Capability requested from the Loom session: library entry points that Runbook can call in process,
so that `runbook` links Loom as a cargo dependency and starts no `loom` or `b10x-loom` process
(the operator, 2026-11-22: "runbook should not rely on binaries [of] canon, loom. Instead, it should
just embed it as a dependency directly.").

Answer of the Loom session, 2026-11-22, collected in `docs/library-entries.md` (rows L1 to L22).
Nothing in Loom is `exists` yet: 9 rows are `on a branch`, 10 are `requested`, 2 `not planned`, 1
`not checked`. Agreed with the Loom session on 2026-11-22, not built:

- `run_tree` becomes a public function of `crates/loom-tree` with plain-data arguments, an optional
  observer called per run-log line and a cancel handle checked between steps (L6, L7, L14);
- `tree status` prints `format: loom.tree-status/1` with per case evidence, runs, approvals, copies,
  link name, parent, protocol id and revision, outcome or stop and open obligations, and the same
  value is the return of a public library function (L9);
- a re-export of Canon in `loom-protocols`, so Runbook names no `b10x-canon` dependency (L22);
- a story for two `linux`-confined trees in one process, one cgroup root per run given by the
  embedder, ordered directly after the stub-cycle path (L19); a story for cancelling a running bound
  program after it (L15);
- the fixed text `UNCONFINED_FLAG_REASON` stays;
- Runbook pins the commit Loom names (the first with `run_tree`), then its tag, and calls the
  runtime from a plain thread per run because the calls block.

Rows this blocker waits on (`requested`): L2 protocols from a directory, L6 `run_tree`, L7 records as
written, L9 the status reading, L13 tree waits as decision requests, L14 cancel at step boundaries,
L15 cancel of a running bound program, L19 two `linux` trees in one process, L21 the suite runner,
L22 the Canon re-export. `not planned`: L5 open from an intent, L17 the list of confinement levels.
`not checked`: L20 the agent executor and the decider configuration (see
`upstream-blocker:loom-decider-per-class`).

This blocker is cleared when every row Runbook needs says `exists` at a named Loom commit and tag.
The first commit with `run_tree` is named by Loom after `tree-run-command` has merged
`child-case-runtime`, `record-copy-between-cases`, `bound-program-contract` and
`open-children-from-arguments-from`.
