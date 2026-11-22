---
format: aep.planning-md/3
id: story:check-name-scan-by-path
kind: story
status: proposed
title: runbook check matches the names of instruction files by path, not by basename token
summary: the name scan of runbook check treats a basename token as the file it names
tags:
- review-rb2
scope:
- confidence: cited
  path: runbook/src/check.rs
- confidence: cited
  path: runbook/tests/cli.rs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T02:50:53Z", actor: "human:operator", revision: 3, executor: "agent:rb2-review-fixes-ui", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: check-name-scan-by-path

## Outcome

The scan for the names of instruction files in `runbook check` reads a name as the path it is written as, so a token that equals the basename of another file does not count as a reference to it.

## Context

Found in the wave rb2 review of runbook/, protocol/, ui/, views/, docs/ (S3 list): runbook/src/check.rs:339-346. Filed by the review-fix unit of wave rb2; the fix is not part of that unit.

## Acceptance

- A test in `runbook/tests/cli.rs` puts a token that equals the basename of an existing instruction file, written under a different directory, into a binding and `runbook check` reports the name as not found (exit 1); the same token written as the real path passes.

## Scope

`runbook/src/check.rs`, `runbook/tests/cli.rs`.
