---
format: aep.planning-md/3
id: story:parity-product-skips-are-recorded
kind: story
status: proposed
title: Each rule of the parity product tree that leaves something out is recorded in the summary
summary: the time stamp rewrite, the lock-name skip, the directory skips and the symlink skip can each hide a difference
tags:
- review-rb2
scope:
- confidence: cited
  path: docs/parity.md
- confidence: cited
  path: runbook/src/parity.rs
- confidence: cited
  path: runbook/tests/cli.rs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T02:50:56Z", actor: "human:operator", revision: 3, executor: "agent:rb2-review-fixes-ui", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: parity-product-skips-are-recorded

## Outcome

What the product tree leaves out or rewrites (time stamps, aep lock files, the `target`, `node_modules` and `.git` directories, symlinks) is listed in the summary, so `compare` shows a difference that one of these rules would hide.

## Context

Found in the wave rb2 review of runbook/, protocol/, ui/, views/, docs/ (S3 list): runbook/src/parity.rs:429-447 (without_times), 449-472 (lock name, target, node_modules and .git directories, symlinks). Filed by the review-fix unit of wave rb2; the fix is not part of that unit.

## Acceptance

- A test in `runbook/tests/cli.rs` makes two runs that differ only in a symlink and in a time stamp in a product file, and `runbook parity compare` exits 0 while the summaries list the symlink and the rewritten file under the names of the rules that left them out.

## Scope

`runbook/src/parity.rs`, `runbook/tests/cli.rs`, `docs/parity.md`.
