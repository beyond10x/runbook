---
format: aep.planning-md/3
id: story:canon-0-10-0-pin
kind: story
status: active
title: Runbook evaluates with Canon 0.10.0
summary: 'Move Runbook''s Canon pin from 0.9.0 to 0.10.0: decisions, compiled protocols, site data and the docs that state the pin.'
tags:
- canon
- conductor-succession
- loom
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: cases
- confidence: cited
  path: docs/loom-bindings.md
- confidence: cited
  path: docs/protocol-notes.md
- confidence: cited
  path: protocol/simulate
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T22:49:15Z", actor: "human:operator", revision: 2, executor: "agent:rb1-canon-pin", correlation: "operator-standing-acceptance-2026-11-21"}
- {from: "proposed", to: "active", at: "2026-11-21T22:49:16Z", actor: "human:operator", revision: 3, executor: "agent:rb1-canon-pin", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: canon-0-10-0-pin

## Outcome

Runbook evaluates its protocols and expected decisions with Canon 0.10.0, the Canon that Loom's
next wave runs on, so both sides use the same Canon. Runbook has no Rust dependency on
`b10x-canon`: it calls the `canon` binary of the `CANON_DIR` checkout (default `../canon`), so the
pin is the version of that checkout, the commit in `protocol/simulate/CANON_REV`, the
`decision.json` files evaluated with it, and the statements in the docs.

## Context

Canon 0.9.0 was the pin; Canon tag `0.10.0` is commit `2c7a1ea6`, which `protocol/simulate/CANON_REV`
already holds. The step moves the evaluated artifacts and the statements to 0.10.0 and records
what changed byte for byte in the 158 expected decisions.

## Acceptance

- `scripts/check-protocols.sh` and `scripts/evaluate-cases.sh` exit 0 with a `canon` built from
  tag `0.10.0`; every `decision.json` is regenerated with it, and a difference in an outcome is
  listed as a finding before anything is committed.
- The three crates `driver`, `watch` and `goals` build, and the driver's own tests pass.
- `scripts/check-bindings.sh`, `aep plan artifact validate` and `sdc-protocol-site check` exit 0.
- The docs that say which Canon is pinned say 0.10.0; statements about what arrived in 0.9.0 are
  unchanged.

## As built (review fix B1, branch `wave-rb1/canon-pin-check`)

The pin is now checked, not only stated. `CANON_DIR` and the sibling-checkout default are gone.

- `driver/canon-pin.json` (`sdc.canon-pin/1`): version `0.10.0` and the full commit of tag `0.10.0`.
  `scripts/check-protocols.sh` fails unless `protocol/simulate/CANON_REV` is a prefix of that commit.
- `canon_resolve` in `scripts/evaluate-cases.sh`, sourced by the other five scripts (one copy; the
  file returns before any step when sourced): `CANON` when set, else the cached install of the
  pinned commit (`check-protocols.sh` makes it with `cargo install`, nothing builds in a Canon
  checkout); refused with both versions unless `canon --version` equals the pin; refused with the
  install command when `CANON` is unset and no cache exists.
- `evaluate-cases.sh`, `check-protocols.sh` and `limitations.sh` call the resolved binary
  (`compile`, `evaluate`, `check`, `validate`, `simulate`); `run-trial.sh`, `run-full-cycle.sh` and
  `run-finding-cycles.sh` use the same step and no longer default to `$HOME/beyond10x/canon`.
- `runbook doctor` compares the version of the canon it finds (`CANON`, the cached pinned install,
  `canon` on `PATH`) with the compiled-in pin and fails a different version.
