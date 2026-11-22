---
format: aep.planning-md/3
id: story:import-approvals
kind: story
status: proposed
title: Import existing approvals and delivery evidence
tags:
- orchestration
relations:
- decomposes: epic:brownfield-entry
scope:
- confidence: inferred
  path: cases/17-import-approvals
- confidence: inferred
  path: driver/src/bin/sdc-import-approvals.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:36Z", actor: "human:operator", revision: 3}
---
# Story: import-approvals

## Outcome

Approvals and deliveries already recorded in an adopted planning store count as copied records,
so nobody re-approves or re-runs finished work (`docs/orchestration.md` § 4.2, § 4.4 gap 5).

## Context

Store transitions carry `actor: human:…`; delivery evidence lives in
`.engineering/evidence/story/<slug>/` (decider: one record for `story:decider-extract`, § 4.3).

## Acceptance

On `cases/17-import-approvals`, the import writes one copied approval record per human transition
to `active` or `implemented`, and the implemented story is absent from the portfolio's
undelivered set.

## Scope

A new program `sdc-import-approvals`, one fixture case.

## Tooling note

- New tool: the Rust binary `sdc-import-approvals` (`driver/src/bin/sdc-import-approvals.rs`), in place of `driver/examples/sdc/tools/import-approvals.sh` named before.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)
