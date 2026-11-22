---
format: aep.planning-md/3
id: story:parity-rename-uses-the-rules-table
kind: story
status: proposed
title: The parity rename step reads the RULES table instead of repeating it
summary: rename lists the generated members a second time beside the RULES table
tags:
- review-rb2
scope:
- confidence: cited
  path: runbook/src/parity.rs
- confidence: cited
  path: runbook/tests/cli.rs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T02:50:56Z", actor: "human:operator", revision: 3, executor: "agent:rb2-review-fixes-ui", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: parity-rename-uses-the-rules-table

## Outcome

The members compared by pattern are named once, in the RULES table, and the rename step takes them from it.

## Context

Found in the wave rb2 review of runbook/, protocol/, ui/, views/, docs/ (S3 list): runbook/src/parity.rs:616-655 (rename) and the RULES table. Filed by the review-fix unit of wave rb2; the fix is not part of that unit.

## Acceptance

- A test in `runbook/tests/cli.rs` adds a pattern member to a copy of the RULES table and the compare step renames it without any change to the rename step.

## Scope

`runbook/src/parity.rs`, `runbook/tests/cli.rs`.
