---
format: aep.planning-md/3
id: story:implement-one-request
kind: story
status: proposed
title: change.implement as one structured request per story with two follow-ups
summary: 'The story''s facts and files inlined, an edit schema, cargo build and tests.run between follow-ups: strong -0.70, mixed -0.53 USD per run (inferred)'
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
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T17:45:31Z", actor: "human:operator", revision: 4, executor: "agent:sdc-improve", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: implement-one-request

## Outcome

`change.implement` runs as one structured request per story, with a cap of two follow-ups:

- the program inlines the story's fact set and the current text of the files its scope names;
- the reply is a list of edits and new files;
- the program writes them and runs `cargo build` and `tests.run`;
- compiler and test lines go into a follow-up request.

## Context

The operator's target, 2026-11-21: 0.47 USD per run (`docs/improving.md`, Target).

- Implement is the most expensive node: 1.98 (strong) and 1.51 (mixed) USD per run.
- Conversation growth is about half of that: 1.07 and 0.81. The median session takes 23 to 25
  turns, with about 11 `Read`s, each re-read on every later turn.
- The node reads 13 to 18 KB of its own `in/` and 17 to 48 KB of `impl/`
  (`experiments/baseline-0.1.0/inputs.md`).
- One request per file or per requirement would write the shared facts (about 4k tokens) once
  per request. Per story is the unit.

## Smallest change

- The `structured` mode of `story:structured-request-nodes`, with the edit schema
  `{edits: [{path, old, new}], files: [{path, content}]}`.
- The fact set: the story, its requirements and acceptance lines, its compiled spec commands only,
  build constraints, findings, and the last rejection addressed to it.
- The current text of the files its scope names, or of the files the previous story changed.
- A follow-up after a failed build or test, at most two.

## Hypothesis and metric

- Saving per run: strong 0.70, mixed 0.53 USD (inferred).
- Primary: `node.story/change.implement.llm_cost_usd`, direction decrease.
- Guards: `run.delivered`, `run.clean`, `edge.story: change.review:rejected ->
  change.implement.count`, the first-pass yield of `story/change.implement`.
- The risk is a node that cannot look things up and builds against what it was given.

## Acceptance

An experiment record on the mixed setting, with the mode behind a run setting and n ≥ 3 per variant,
reaches a verdict by `sdc-experiment report`, and the verdict, held or not, is in the ledger.
