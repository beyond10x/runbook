---
format: aep.planning-md/3
id: upstream-blocker:canon-library-entry
kind: upstream-blocker
status: cleared
title: 'Canon: compile, evaluate, check, validate and simulate as library functions'
tags:
- canon
- one-binary
relations:
- blocks: story:engine-on-loom-library-entry
revision: 4
transitions:
- {from: "open", to: "cleared", at: "2026-11-22T04:14:26Z", actor: "agent:plan-onebin", revision: 4, correlation: "operator-requirement-2026-11-22-one-binary"}
---
Capability requested from the Canon session: the operations Runbook calls through the `canon`
binary today (compile, evaluate, check, validate, simulate) as public library functions of a Canon
crate, so that `runbook` links Canon as a cargo dependency pinned by commit and needs no `canon`
binary at run time (the operator, 2026-11-22: "runbook should not rely on binaries [of] canon, loom.
Instead, it should just embed it as a dependency directly.").

Answer of the Canon session, 2026-11-22, collected in `docs/library-entries.md` (rows C1 to C8):

- Exist as public functions of `b10x-canon`, read at `7d1a28a20` on `wave/2026-11-21`: parse (C1),
  validate (C2), compile and compose (C3), evaluate (C4), check (C5), simulate (C6).
- Not library entries and not planned: the exit-status mapping of `check` (private to `canon-cli`,
  C7) and import handling (CLI-only, C8; the library has `ir::compose`).

Check of 2026-11-22, by Runbook, read-only, nothing run or compiled: the six items at the Canon
version Loom pins. Loom's integration commit `1016c251` pins `b10x-canon` tag `0.10.0` in
`loom-bindings`, `loom-governor`, `loom-protocols` and `loom-tree` (`Cargo.toml`), and `Cargo.lock`
resolves it to `2c7a1ea6a9dc2e19633fb225290c826b1c9988ce`. At that tag (`2c7a1ea6a`) every item of
C1 to C6 is a public function of `b10x-canon` with the signature the Canon session gave: `parse`,
`validate`, `compile`, `compose` (re-exported as `ir::compose`), `Ir::canonical_json`, `evaluate`,
`evaluate_with` (fourth argument `Supplied<'_>`), `read_ir`, `read_case`, `read_evidence`, `render`,
`check`, `read_properties`, `simulate`. The lines differ from `7d1a28a20`. The readers, `render` and
`read_properties` are re-exported at `eval::` and `check::`; their modules are private, so the paths
the first answer gave (`eval::read::read_ir` and the like) do not compile. The page
`docs/library-entries.md` has each line at the tag and says `exists (tag 0.10.0, 2c7a1ea6a)` on
rows C1 to C6. Commands (Canon read with `git -C <canon> show "0.10.0:crates/canon/src/<file>"`
and `git grep -n <pattern> 0.10.0 -- crates/canon/src`; Loom with `git -C <loom> show
1016c251:<file>`).

Still true: Canon has no written version rule for its public functions, so `0.10.0` and the next
release are not interchangeable; Runbook links the one Canon Loom pins (one copy, or Loom's types and
Runbook's do not match). C7 and C8 stay Runbook's: it maps exit statuses itself and passes imported
protocols to `ir::compose` as values. Neither is requested.

Every row Runbook needs (C1 to C6) says `exists`; this blocker is cleared on 2026-11-22 with
`docs/library-entries.md` as its evidence. A later Canon release changes the rows only when Runbook
moves its pin.
