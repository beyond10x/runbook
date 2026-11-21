---
format: aep.planning-md/3
id: story:llm-call-count-matches-record
kind: story
status: proposed
title: The driver's LLM call count matches the call record
tags:
- conductor-succession
- trial-2
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: inferred
  path: driver/examples/sdc/tools/llm-node.sh
- confidence: inferred
  path: driver/src/main.rs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:36:14Z", actor: "human:timo", revision: 3, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: llm-call-count-matches-record

## Outcome

The cap on LLM calls counts calls that reached the model, and the run's two figures agree.

## Context

Trial 2: the driver counted 30 of 45 calls where `llm-calls.jsonl` holds 27, because "not called"
starts (a node that returned without calling the model) count toward the cap
(`docs/trial-2.md:529-530`, R8).

## Acceptance

On a fixture run with one "not called" start and two real calls, `sdc-drive` reports 2 calls against
`--max-llm-calls`, equal to the lines in `llm-calls.jsonl`.

## Scope

`driver/src/main.rs` (the counter); `llm-node.sh` (existing bash, a fix: mark a start that made no
call).
