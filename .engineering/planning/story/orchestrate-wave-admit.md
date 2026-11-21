---
format: aep.planning-md/3
id: story:orchestrate-wave-admit
kind: story
status: proposed
title: Admit or refuse a wave by program
tags:
- orchestration
relations:
- decomposes: epic:orchestrate-area-case
- depends_on: story:orchestrate-protocol-adopt
- depends_on: story:concurrency-limit
scope:
- confidence: inferred
  path: cases/09-orchestrate-wave-admit
- confidence: inferred
  path: driver/src/bin/sdc-wave-admit.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:36Z", actor: "human:timo", revision: 3}
---
# Story: orchestrate-wave-admit

## Outcome

Whether a wave may run at once is decided by rules, and a refusal names the failing rule
(`docs/orchestration.md` § 2.4).

## Context

Rules 1-4 of § 2.4: every story active, in one wave of `aep plan artifact waves`, no collision,
none unassessed; every `depends_on` delivered; story count within free run slots (driver ceiling,
`slot status`, model budget); every story's ESS gate passes. Rule 5 (WIP) is
`story:wip-limits-admission`.

## Acceptance

`cases/09-orchestrate-wave-admit` has four variants (collision, undelivered dependency, more
stories than free slots, all rules hold), and `admission` is `refused` naming rules 1, 2 and 3
respectively, and `admitted` for the fourth.

## Scope

A new program `sdc-wave-admit`, one fixture case with variants.

## Ambiguities

- Rule 4 for an adopted spec uses the ratchet (`story:ess-ratchet-gate`); until then the full
  gate. inferable: `docs/orchestration.md` § 7 decision 3.

## Tooling note

- New tool: the Rust binary `sdc-wave-admit` (`driver/src/bin/sdc-wave-admit.rs`), in place of `driver/examples/sdc/tools/orchestrate-wave-admit.sh` named before.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)
