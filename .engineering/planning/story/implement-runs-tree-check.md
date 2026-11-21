---
format: aep.planning-md/3
id: story:implement-runs-tree-check
kind: story
status: proposed
title: The implementing node may run the tree's own check
summary: change.implement and change.fix run task check or cargo test through the build pool, confined, output to the model
tags:
- eval
- improvement-loop
relations:
- decomposes: epic:improvement-loop
- depends_on: story:improvement-baseline
- informed_by: story:model-per-node-variable
scope:
- confidence: inferred
  path: driver/examples/sdc/sdc-bugfix.bindings.json
- confidence: inferred
  path: driver/examples/sdc/sdc-story.bindings.json
- confidence: inferred
  path: driver/examples/sdc/tools/llm-node.sh
- confidence: inferred
  path: experiments
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T12:38:20Z", actor: "human:operator", revision: 6, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: implement-runs-tree-check

## Outcome

`change.implement` (and `change.fix` in `sdc.bugfix`) may run the tree's own check, `task check` or
`cargo test` through the build pool, inside its work directory, and nothing else: one declared
operation, confined, whose output is returned to the model.

## Context

Diagnosis of 2026-11-21: `runs/trial-3-haiku-8` (run record, not in git) stopped `writer_spent` in
`IDEA-1.list.access`. The writer was shown the failing assertion in `in/feedback.md` twice
(`src/http.rs:366`, left 200, right 400), diagnosed it correctly and still could not fix it: the
fault was in its own test's escaping (the JSON escape was decoded in the Write tool call), and in
its own words it had no shell and never ran the tests. Today's run metrics show `change.implement`
as the node with the highest looping factor on Haiku (2.75, n 6) and first-pass yield 0.167 on
Haiku against 1.0 in the strong setting.

## Hypothesis and metric

- Hypothesis: with the tree's check available to it, the first-pass yield of
  `story/change.implement` rises and its looping factor falls on Haiku and mixed, at a higher cost
  per call.
- Primary metric: `node.story/change.implement.first_pass_yield`, direction increase.
- Guards: `run.cost_usd` of a clean pass, the clean-pass count.

## Scope questions (to record, not to solve here)

- Which adapter operation gives a command run today: the metaharness adapter's operations are
  `dir.list`, `file.read`, `file.write` and `search`.
- What confinement it needs: Substrate's executable allow-list (merged on 2026-11-21 as story 1 of
  that repository) lists programs by path. A missing capability is requested from that
  repository's session (`AGENTS.md`, Layers).

## Acceptance

An experiment record with this hypothesis, at least 3 runs per variant in the Haiku setting,
reaches a verdict by `sdc-experiment report`, and the change is merged only when it held.
