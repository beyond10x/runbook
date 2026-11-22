---
format: aep.planning-md/3
id: story:parity-load-checks-the-whole-summary
kind: story
status: proposed
title: runbook parity compare refuses a summary that is not whole
summary: load checks only the format member, so a summary with a missing member compares as empty
tags:
- review-rb2
scope:
- confidence: cited
  path: runbook/src/parity.rs
- confidence: cited
  path: runbook/tests/cli.rs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T02:50:57Z", actor: "human:operator", revision: 3, executor: "agent:rb2-review-fixes-ui", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: parity-load-checks-the-whole-summary

## Outcome

A summary file that lacks a member of `runbook.parity-summary/1` or holds one of the wrong type is refused with the member named, instead of comparing as an empty list.

## Context

Found in the wave rb2 review of runbook/, protocol/, ui/, views/, docs/ (S3 list): runbook/src/parity.rs:729-735. Filed by the review-fix unit of wave rb2; the fix is not part of that unit.

## Acceptance

- A test in `runbook/tests/cli.rs` removes the `cases` member from a summary and `runbook parity compare` exits 2 and names `cases`; a summary with `cases` of the wrong type exits 2 the same way.

## Scope

`runbook/src/parity.rs`, `runbook/tests/cli.rs`.
