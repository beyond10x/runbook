---
format: aep.planning-md/3
id: story:one-instruction-renderer
kind: story
status: proposed
title: The instruction renderer is written once
summary: llm-node.sh, decider.sh and instructions.rs hold the same rules three times (review rb2 S3-6).
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: bindings/tools/decider.sh
- confidence: cited
  path: bindings/tools/llm-node.sh
- confidence: cited
  path: driver/src/instructions.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T02:42:20Z", actor: "human:operator", revision: 2, executor: "agent:rb2-driver-fixes", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: one-instruction-renderer

## Outcome

The instruction renderer is written once. A change of its rules is made in one place.

## Context

Source: review rb2, driver, S3-6: the same rules are written three times, in
`bindings/tools/llm-node.sh:397`, `bindings/tools/decider.sh:156` and `driver/src/instructions.rs:40`
(documented at `driver/src/instructions.rs:5`). Only the goldens hold the three equal.

## Acceptance

A rule of the renderer (the placeholder syntax, the refusal of a path, the single pass) is changed
in exactly one file, and a test fails for every tool whose output differs from it.
