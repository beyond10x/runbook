---
format: aep.planning-md/3
id: story:canon-slice-bound-uptake
kind: story
status: proposed
title: Restore what was dropped to fit Canon's state bound
tags:
- blocked-upstream
- canon
relations:
- decomposes: epic:kernel-and-runtime-uptake
scope:
- confidence: inferred
  path: docs/limitations/output.txt
- confidence: inferred
  path: protocol/compiled/sdc-intake.ir.json
- confidence: inferred
  path: protocol/draft/orchestrate.protocol.yaml
- confidence: inferred
  path: protocol/sdc-intake.protocol.yaml
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:35Z", actor: "human:timo", revision: 3}
---
# Story: canon-slice-bound-uptake

## Outcome

Protocols that were cut to fit Canon's whole-product bound of 65,536 states get back what was cut
only for that reason, once a Canon release checks the bound per slice.

## Context

Canon wave 2 is closed on its branch, not released (last release 0.9.0; canon commit `c76fb90`,
gate at `ac46b8c`: 968 tests). Canon `story:check-subject-slices`: `canon check` holds the bound
per slice (per outcome, action and property over the dimensions it reads); `canon generate` keeps
the whole-product bound. Of the SDC's four bound limitation protocols three now check, and
`story-with-four-gate-kinds` is refused for one slice above the bound. Same wave: `--import`
(`story:protocol-imports`) and the evaluation-cost fix.

Dropped or reshaped only to fit the bound:

1. `sdc.intake` dropped the invalidation `idea.revised` (`docs/trial-2.md:119-121`, "Scope-changing
   answers": 49,152 states, was 65,536).
2. Draft `sdc.orchestrate` dropped the result field of `triage` and `wave_approval`
   (`docs/orchestration.md` § 2.6 and § 6: refused at 196,608, now 49,152).
3. Intake's own abandon decision moved to the root (`docs/limitations/intake-with-abandon.protocol.yaml`,
   131,072 states; `docs/protocol-notes.md:139-148`).
4. Invalidated claims kept as leaves; no derived `change.verified`
   (`docs/limitations/story-with-derived-verified-claim.protocol.yaml`, 196,608;
   `docs/protocol-notes.md:152-157`).
5. Outside `sdc.spec` the gate is one composite `ess_gate` instead of four kinds
   (`docs/protocol-notes.md:111-113`; `story-with-four-gate-kinds.protocol.yaml`, 786,432; still
   refused per slice, so it stays).
6. `sdc.spec` dropped `spec_self_conformance` partly for the bound (`docs/protocol-notes.md:104-105`;
   the other reason, it checks the model against itself, still holds).

## Acceptance

With the first Canon release after 0.9.0 pinned, `scripts/check-protocols.sh` exits 0 with
`idea.revised` back in `sdc.intake` and result fields back on `triage` and `wave_approval`, and
`scripts/limitations.sh` records three bound limitation protocols checking and
`story-with-four-gate-kinds` refused.

## Scope

`protocol/sdc-intake.protocol.yaml` and its IR, `protocol/draft/orchestrate.protocol.yaml` (or its
adopted path), `docs/limitations/output.txt`.

## Ambiguities

- Items 3, 4 and 6: restore or keep the current shape: requires-stakeholder-input (the operator); the
  current shapes also have design reasons (`docs/protocol-notes.md:146-148`).
