---
format: aep.planning-md/3
id: story:model-read-reasons-as-instruction-files
kind: story
status: proposed
title: The refusal reasons and says sentences a model reads are instruction files
summary: check_reply reasons, unstick says and table-check reasons move out of the Rust sources (review rb2 S2-1).
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: driver/src/bin/sdc-conform.rs
- confidence: cited
  path: driver/src/bin/sdc-reply/implement.rs
- confidence: cited
  path: driver/src/bin/sdc-unstick.rs
- confidence: cited
  path: instructions
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T02:42:19Z", actor: "human:operator", revision: 2, executor: "agent:rb2-driver-fixes", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: model-read-reasons-as-instruction-files

## Outcome

The sentences a model reads back from the driver's Rust binaries are instruction files, like the
prompts: the refusal reasons of `check_reply`, the `says` sentences of the unstick moves and the
reasons of the conformance table check.

## Context

Source: review rb2, driver, S2-1 (second part): `driver/src/bin/sdc-reply/implement.rs:214-311`
(quoted back in the retry prompt), `driver/src/bin/sdc-unstick.rs:117`, `:128`, `:134`,
`driver/src/bin/sdc-conform.rs:438`, `:446`, `:455`. The request-mode headings and notes already
moved in this wave; these did not.

## Acceptance

With these sentences moved, `the_three_rust_binaries_hold_no_instruction_text` finds no word of
them in the three sources, and the goldens of the retry, unstick and conformance prompts are
unchanged.
