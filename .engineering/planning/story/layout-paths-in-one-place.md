---
format: aep.planning-md/3
id: story:layout-paths-in-one-place
kind: story
status: proposed
title: The layout paths of a checkout are written in one place
summary: LAYOUT paths are repeated in assets.rs, check.rs, ui/server.mjs and the scripts
tags:
- review-rb2
scope:
- confidence: cited
  path: runbook/src/assets.rs
- confidence: cited
  path: runbook/src/check.rs
- confidence: cited
  path: scripts
- confidence: cited
  path: ui/server.mjs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T02:50:55Z", actor: "human:operator", revision: 3, executor: "agent:rb2-review-fixes-ui", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: layout-paths-in-one-place

## Outcome

The directories and files that make a checkout (the layout table) are written once, and every reader of a path of the checkout takes it from that table or from a file generated from it.

## Context

Found in the wave rb2 review of runbook/, protocol/, ui/, views/, docs/ (S3 list): runbook/src/assets.rs:70-85, runbook/src/assets.rs:160-174 (helper), runbook/src/assets.rs:183 (is_checkout), runbook/src/check.rs, ui/server.mjs. Filed by the review-fix unit of wave rb2; the fix is not part of that unit.

## Acceptance

- A test fails when a layout path named in `runbook/src/assets.rs` is written as a literal in `runbook/src/check.rs`, `ui/server.mjs` or a file under `scripts/` instead of being read from the one table.

## Scope

`runbook/src/assets.rs`, `runbook/src/check.rs`, `ui/server.mjs`, `scripts`.
