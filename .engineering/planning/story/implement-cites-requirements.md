---
format: aep.planning-md/3
id: story:implement-cites-requirements
kind: story
status: proposed
title: The implementing node cites, per requirement, the line that builds it, checked by program
summary: 'A requirement nobody built is sent back by a program before tests.run, not by the review: the largest loss of the 0.1.0 baseline (10 review rejections, 5.25 USD, 1,873 s)'
tags:
- eval
- improvement-loop
relations:
- decomposes: epic:improvement-loop
scope:
- confidence: cited
  path: driver/examples/sdc/sdc-story.bindings.json
- confidence: cited
  path: driver/examples/sdc/tools/llm-node.sh
- confidence: cited
  path: driver/src/bin
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T16:04:35Z", actor: "human:operator", revision: 2, executor: "agent:sdc-improve", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: implement-cites-requirements

## Outcome

A requirement of a story that nobody built is sent back by a program before `tests.run`, not by
the model review after it: the implementing node's reply states, per requirement id the story
covers, the file and line that builds it, and a check refuses the reply when an id has no
citation or a citation names no line of the change.

## Context

The largest loss of the 0.1.0 baseline that no other story of `epic:improvement-loop` addresses:
`change.review` sent the build back 10 times in 9 runs, a loop of 5.25 USD and 1,873 s
(strong 5, 3.271 USD, 781 s, 19 % of the strong setting's cost; mixed 2, 1.037 USD, 266 s;
Haiku 3, 0.944 USD, 826 s; `experiments/baseline-0.1.0/losses.md`). Classes of the 10 rejections:
an offline requirement not built in the story that covers it 4 (strong-2 twice, strong-3,
mixed-3), sync logic wrong 2, the story's scope and the build disagree 2, the Postgres default q9
1, the build diverges from the spec 1. In three of the four first-class cases the build's own
README said the requirement was not built (for example strong-3: "README.md:22-23 admits this"),
so a per-requirement statement would have shown the gap without a model.

## Smallest change

- The implement prompt and reply schema (`driver/examples/sdc/tools/llm-node.sh`, the
  `change.implement` reply) gain `requirements: [{id, at: "<file>:<line>[-<line>]"}]`, one entry
  per id in the story's covers list, or `{id, not_built: "<reason>"}`.
- A program check after the writer and before `tests.run` (a Rust subcommand beside
  `sdc-findings`; binding in `driver/examples/sdc/sdc-story.bindings.json`): every covered id
  appears; every `at` names a file of the change and a line that exists; a `not_built` entry sends
  the writer back with the id. Its refusal text names the ids, so it reaches the writer through
  R13.
- A protocol change only if the check needs its own result in `sdc.story`; otherwise it is a
  binding and driver change.

## Hypothesis and metric

- Hypothesis: with the requirement citations checked, the count of
  `story: change.review:rejected -> change.implement` falls.
- Primary metric: `edge.story: change.review:rejected -> change.implement.count`, direction
  decrease; the register: strong 5 in 3 runs.
- Maximum gain: the whole loop, 5.25 USD and 1,873 s over the 9 runs; the first class alone, 4 of
  the 10 rejections, about 2.10 USD and 750 s if shared by count (inferred).
- Guards: `run.delivered` (clean passes of strong and mixed stay 3 of 3), `run.cost_usd`, the
  first-pass yield of `story/change.implement`, and the count of `merge.approve` rejections (a
  requirement now caught earlier must not be caught later instead).

## Acceptance

An experiment record on the strong setting, the change behind a run setting, n ≥ 3 per variant,
reaches a verdict by `sdc-experiment report`, and the verdict, held or not, is in the ledger.
