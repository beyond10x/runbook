---
format: aep.planning-md/3
id: story:review-minimal-facts
kind: story
status: proposed
title: The review gets the smallest fact set, not the whole change tree
summary: 'change.review on the changed files, the covered requirements and commands and the test summary: a third of its input, strong -0.46 and mixed -0.36 USD per run (inferred)'
tags:
- eval
- improvement-loop
relations:
- decomposes: epic:improvement-loop
scope:
- confidence: cited
  path: driver/examples/sdc/tools/llm-node.sh
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T17:45:30Z", actor: "human:operator", revision: 3, executor: "agent:sdc-improve", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: review-minimal-facts

## Outcome

`change.review` gets the smallest fact set the program can assemble instead of the whole change
tree and the diff text. Its decisions stay the same, and its input falls to about a third.

## Context

The operator's target, 2026-11-21: `run.cost_usd` 0.47 USD (`docs/improving.md`, Target), and
"reduce context to minimal fact sets / inputs".

The review is a `claude-cli` session with `Glob`, `Read` and `Grep`, 10 to 17 turns. Its inputs
are the whole tree `in/impl/` (24 KB at the first story of `trial-0.1.0-strong-1`, 84 KB at its
fourth) plus `in/change.diff` (24 to 26 KB). For a new file the diff is the whole file again.
Its tools returned 31 to 110 KB per call. 0.69 of a strong run's 0.80 review USD is input
(`experiments/baseline-0.1.0/inputs.md`). The fourth review's prompt passes 100K tokens, where
Haiku costs five times as much.

The change: the facts go into the prompt and the read tools are dropped.

## Smallest change

`driver/examples/sdc/tools/llm-node.sh` builds the review's `in/` from:

- `scope.json`;
- the entries of `commands.json` the story covers;
- the changed files of `impl/`, with line numbers;
- the changed line ranges;
- the test summary and the names of the story's tests.

The prompt, the response schema and the check of `at` and `violates` stay.

## Hypothesis and metric

- Saving per run: strong 0.46, mixed 0.36 USD (inferred, input to a third).
- Test: record `review-minimal-facts`, three recorded replays of the strong runs of `release-0.1.2`
  with only the reviews live, on the source tree.
- Pass: every review returns the recorded decision, a rejection names the same requirement ids,
  and the review's cache-write tokens per call fall by at least half.

## Acceptance

The record `review-minimal-facts` reaches its result by `sdc-experiment report`, and the result,
pass or not, is in the ledger.
