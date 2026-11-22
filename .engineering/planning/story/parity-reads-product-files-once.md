---
format: aep.planning-md/3
id: story:parity-reads-product-files-once
kind: story
status: proposed
title: The parity reader holds each product file in memory once
summary: walk_product makes four copies of each product file before it hashes it
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
# Story: parity-reads-product-files-once

## Outcome

Summarizing a run reads a product file into memory once and hashes it from that buffer, so a large product file does not cost four times its size.

## Context

Found in the wave rb2 review of runbook/, protocol/, ui/, views/, docs/ (S3 list): runbook/src/parity.rs:429-447 (without_times) and the walk of the product tree (449-472). Filed by the review-fix unit of wave rb2; the fix is not part of that unit.

## Acceptance

- A test in `runbook/tests/cli.rs` summarizes a run whose product holds a file of 64 MiB and its peak memory (measured by the test) stays under three times that size, with the same digest as before.

## Scope

`runbook/src/parity.rs`, `runbook/tests/cli.rs`.
