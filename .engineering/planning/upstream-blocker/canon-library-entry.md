---
format: aep.planning-md/3
id: upstream-blocker:canon-library-entry
kind: upstream-blocker
status: open
title: 'Canon: compile, evaluate, check, validate and simulate as library functions'
tags:
- canon
- one-binary
relations:
- blocks: story:engine-on-loom-library-entry
revision: 2
---
Capability requested from the Canon session: the operations Runbook calls through the `canon`
binary today (compile, evaluate, check, validate, simulate) as public library functions of a Canon
crate, so that `runbook` links Canon as a cargo dependency pinned by commit and needs no `canon`
binary at run time (the operator, 2026-11-22: "runbook should not rely on binaries [of] canon, loom.
Instead, it should just embed it as a dependency directly.").

Answer of the Canon session, 2026-11-22, collected in `docs/library-entries.md` (rows C1 to C8):

- Exist as public functions of `b10x-canon`, read at `7d1a28a20` on `wave/2026-11-21`, not run, state
  `on a branch`: parse (C1), validate (C2), compile and compose (C3), evaluate (C4), check (C5),
  simulate (C6).
- Not library entries and not planned: the exit-status mapping of `check` (private to `canon-cli`,
  C7) and import handling (CLI-only, C8; the library has `ir::compose`).

Not cleared on 2026-11-22. This blocker clears when every row Runbook needs says `exists` at a
named Canon commit; no row says `exists`. What remains:

- The six rows are on a branch, not in a release tag, and Canon has no written version rule for its
  public functions.
- The Canon Runbook links is the one Loom pins (tag `0.10.0` today; one copy, or Loom's types and
  Runbook's do not match). The rows were read at `7d1a28a20`; the lines of the same functions in tag
  `0.10.0` differ and the public items at `0.10.0` were not checked.
- C7 and C8 stay Runbook's: it maps exit statuses itself and passes imported protocols to
  `ir::compose` as values. Neither is requested.

The next move is a check of the six public items at the Canon version Loom pins, or a Canon
release that carries the branch and a Loom pin on it; then the rows say `exists` and this blocker
moves to `cleared` with `docs/library-entries.md` as its evidence.
