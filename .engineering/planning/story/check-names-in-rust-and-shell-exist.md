---
format: aep.planning-md/3
id: story:check-names-in-rust-and-shell-exist
kind: story
status: proposed
title: runbook check verifies that the instruction file names written in Rust and shell exist
summary: names of instruction files in driver/src, bindings/tools and scripts are not checked for existence
tags:
- review-rb2
scope:
- confidence: cited
  path: runbook/src/check.rs
- confidence: cited
  path: runbook/tests/cli.rs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T02:50:54Z", actor: "human:operator", revision: 3, executor: "agent:rb2-review-fixes-ui", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: check-names-in-rust-and-shell-exist

## Outcome

Every `instructions/<name>.md` named in a Rust or shell source of the checkout is checked for existence by `runbook check`, as the names in the bindings are.

## Context

Found in the wave rb2 review of runbook/, protocol/, ui/, views/, docs/ (S3 list): runbook/src/check.rs:339-346 (review, answer 2: a scan of instructions/*.md names in driver/src, bindings/tools, bindings/*.yaml and scripts found none missing by hand). Filed by the review-fix unit of wave rb2; the fix is not part of that unit.

## Acceptance

- A test in `runbook/tests/cli.rs` writes the name of a missing instruction file into a Rust source and a shell script of a scratch checkout, and `runbook check` exits 1 and names both files and the missing name.

## Scope

`runbook/src/check.rs`, `runbook/tests/cli.rs`.
