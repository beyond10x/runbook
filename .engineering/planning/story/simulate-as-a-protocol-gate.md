---
format: aep.planning-md/3
id: story:simulate-as-a-protocol-gate
kind: story
status: proposed
title: canon simulate as a protocol gate
summary: check-protocols.sh runs canon simulate on all eight protocols and fails on a rising finding count
tags:
- blocked-upstream
- canon
relations:
- decomposes: epic:kernel-and-runtime-uptake
scope:
- confidence: cited
  path: docs/protocol-notes.md
- confidence: cited
  path: protocol/simulate
- confidence: inferred
  path: scripts/check-protocols.sh
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T06:26:12Z", actor: "human:timo", revision: 4, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: simulate-as-a-protocol-gate

## Outcome

`scripts/check-protocols.sh` runs `canon simulate --path <protocol.yaml>` on all eight protocols
and fails when the set of findings of any protocol changes (a finding appears, goes, or changes).

## Context

Canon session, 2026-11-21. `canon simulate` exits 0 on no finding, 1 on a finding or refusal, 2 on
an unreadable protocol. Output: the findings; then, on a protocol with a `case_open` action, one
line starting `limit:`; then `simulated: protocol ... N reachable states, T terminable (...),
[W waiting on supplied evidence or expiry,] F findings (S stuck or cycle states)`. Canon's
released binary 0.9.0 has no `simulate`; it is on Canon branch `wave/2026-11-20` (PR 27).

The gate runs now, on the pinned unreleased commit `a1a937ec` (decision D151, the operator,
2026-11-21): `protocol/simulate/CANON_REV` holds the commit, the check builds it once into
`${XDG_CACHE_HOME:-$HOME/.cache}/sdc/canon-simulate/<rev>/bin/canon` (never on `PATH`, every other
check keeps the released binary), and `protocol/simulate/<name>.txt` are the pinned reports, 71
findings on the eight protocols at `dfe4a3f`. A different report fails the check with a diff and
`simulate: <protocol>: findings changed (was N, now M)`; `--repin` rewrites the reports for a
deliberate change (`docs/protocol-notes.md`, "Simulate"). The earlier plan, a count ratchet in
`protocol/simulate-expected.json`, is replaced by the byte-for-byte comparison.

Until Canon releases `simulate-revise-step` (wave 8), `simulate-record-groups` (wave 7) and
`simulate-child-outcome-step` (wave 6), most findings are walk artifacts, not SDC defects, and
the pinned reports will be re-pinned when those waves change simulate's output.

## Acceptance

Done on the pinned commit: with `protocol/simulate/*.txt` pinned, `scripts/check-protocols.sh`
prints each protocol's reachable, terminable, waiting and finding counts and exits non-zero when a
protocol's report differs from the pinned one.

Remains for the Canon release (`upstream-blocker:canon-simulate-release`): the pin
`protocol/simulate/CANON_REV` and the install step are replaced by the released Canon version
(the `canon` the other checks use), and the reports are re-pinned from it. Later target: zero
findings, so the pinned reports are empty and the check fails on any finding; after Canon's waves
6-8 are released, a finding that is an SDC defect is fixed in the protocol, not pinned.

## Scope

- `scripts/check-protocols.sh`
- `protocol/simulate/` (`CANON_REV` and the eight pinned reports)
- `docs/protocol-notes.md` ("Simulate")

## Ambiguities

- Which of the 71 findings are SDC defects is decided per class when Canon's waves land
  (`plan/simulate-triage` classifies today's); the pinned reports hold until then.
