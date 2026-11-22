---
format: aep.planning-md/3
id: story:check-name-scan-skips-comments-and-orphans
kind: story
status: proposed
title: runbook check does not count a name in a comment or an orphan chain as a reference
summary: the name scan counts comments and orphan chains as references, so an unused instruction file passes
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
# Story: check-name-scan-skips-comments-and-orphans

## Outcome

An instruction file that is named only in a comment, or only by another file that nothing reaches, is reported as unreferenced by `runbook check`.

## Context

Found in the wave rb2 review of runbook/, protocol/, ui/, views/, docs/ (S3 list): runbook/src/check.rs:383-384. Filed by the review-fix unit of wave rb2; the fix is not part of that unit.

## Acceptance

- A test in `runbook/tests/cli.rs` names an instruction file only in a comment of a binding and in an orphan chain, and `runbook check` exits 1 and names the file as unreferenced; naming it from a reachable binding makes it exit 0.

## Scope

`runbook/src/check.rs`, `runbook/tests/cli.rs`.
