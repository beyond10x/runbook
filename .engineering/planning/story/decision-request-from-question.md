---
format: aep.planning-md/3
id: story:decision-request-from-question
kind: story
status: proposed
title: Render a scope-changing question as a decision request
relations:
- decomposes: epic:decisions-a-person-can-take-cold
- depends_on: story:interview-question-context
scope:
- confidence: inferred
  path: driver/src/bin/sdc-decision-request.rs
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:35Z", actor: "human:operator", revision: 3}
---
# Story: decision-request-from-question

## Outcome

A scope-changing question reaches a person as a decision request in the contract's shape, built by
a program from `questions.yaml` and `dissection.yaml`, never written by an LLM.

## Acceptance

`sdc-decision-request <case dir> <question id>` prints one JSON request for the trial-2 fixture
whose `question` is the question text, whose context line reads "<run>, <idea>, <stage>", whose
`options` each carry `does`, `cost` and `consequence` filled from the question's per-option
consequences, whose recommendation names the default with its grounds citing the dissection item
(`path:line` into `work/dissection.yaml`), and which ends with a what-happens-next line naming the
action the run takes after the answer; the output validates against the `sdc.decision` request
shape, and running it twice gives the same bytes.

## Scope

A new `sdc-decision-request` (beside `tools/decision-from-record.sh`, which builds the
approval and close requests and is not changed). Reuses the grounding of `tools/decision-grounds.sh`.

## Tooling note

- New tool: the Rust binary `sdc-decision-request` (`driver/src/bin/sdc-decision-request.rs`), in place of `tools/decision-request.sh` named before.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)

