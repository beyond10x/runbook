---
format: aep.planning-md/3
id: story:structured-request-nodes
kind: story
status: proposed
title: 'Nodes run as one structured request: facts in the prompt, a response schema, no tools'
summary: 'spec.author, plan.decompose and the intake and direction nodes without a conversation: strong -0.30, mixed -0.14 USD per run (inferred)'
tags:
- eval
- improvement-loop
relations:
- decomposes: epic:improvement-loop
scope:
- confidence: cited
  path: driver/examples/sdc/tools/llm-adapter-claude-cli.sh
- confidence: cited
  path: driver/examples/sdc/tools/llm-adapter-metaharness.sh
- confidence: cited
  path: driver/examples/sdc/tools/llm-node.sh
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T17:45:30Z", actor: "human:operator", revision: 5, executor: "agent:sdc-improve", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: structured-request-nodes

## Outcome

A node can run as one structured request instead of a tool-using conversation:

- the program assembles the node's facts into the prompt;
- the reply is a JSON object of a response schema, and no tool is offered;
- the program writes the files;
- a refused check sends at most two follow-up requests, each with the refusal lines.

`spec.author`, `plan.decompose` and the seven intake and direction nodes use it.

## Context

The operator's target, 2026-11-21: 0.47 USD per run (`docs/improving.md`, Target), "dont collect
big conversations" (`experiments/baseline-0.1.0/inputs.md`).

- Every metaharness session's first request reads about 15,500 cached tokens of harness prefix.
  On Sonnet that is a third of an intake session.
- `spec.author` reads 35 to 37 KB of its own `in/` over 20 turns.
- Conversation growth is 1.46 USD of a strong run and 1.03 of a mixed run.

## Smallest change

- A mode `structured` per node in the bindings, read by `driver/examples/sdc/tools/llm-node.sh`.
- The node's fact set as fields of records, listed in `inputs.md`, not whole files.
- `spec.author`: the format as one minimal example (`system.yaml` and one domain file), not the
  28.8 KB example tree.
- The adapters already pass a response schema (`claude-cli`, the review and the decider).
- The apply step writes `files: [{path, content}]`.

## Hypothesis and metric

- Saving per run: intake, direction and `plan.decompose` strong 0.17, mixed 0.04 USD;
  `spec.author` strong 0.13, mixed 0.10 USD (inferred).
- Primary: `node.spec/spec.author.llm_cost_usd`, direction decrease.
- Guards: `run.delivered`, `run.clean`, the `spec.validate` refusals per run.

## Acceptance

An experiment record on the mixed setting, with the mode behind a run setting and n ≥ 3 per variant,
reaches a verdict by `sdc-experiment report`, and the verdict, held or not, is in the ledger.
