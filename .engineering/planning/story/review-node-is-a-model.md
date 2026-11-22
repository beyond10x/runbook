---
format: aep.planning-md/3
id: story:review-node-is-a-model
kind: story
status: proposed
title: change.review is an LLM node of class REVIEW
summary: the review is a schema-constrained model call by a principal other than the author; a rejection goes back through the retry path, rounds bounded
tags:
- trial-3
relations:
- decomposes: epic:conductor-succession
- depends_on: story:llm-node-structured-output
- informed_by: story:independent-reviews
scope:
- confidence: inferred
  path: driver/examples/sdc/sdc-story.bindings.json
- confidence: inferred
  path: driver/examples/sdc/tools/llm-node.sh
- confidence: inferred
  path: driver/examples/sdc/tools/node-classes.json
- confidence: inferred
  path: driver/examples/sdc/tools/sdc-tool.sh
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T06:32:49Z", actor: "human:operator", revision: 6, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: review-node-is-a-model

## Outcome

`change.review` is an LLM node of class REVIEW (`SDC_LLM_MODEL_REVIEW`), run by a principal other
than the change's author, given the story, its acceptance, the requirements it cites, the diff and
the test output. Its reply is schema-constrained: a decision, and findings each with file and line
and each tied to a requirement or an acceptance line. A rejecting review sends the change back to
the implementer through the existing retry path, and the number of review rounds is bounded.

## Context

Today `change.review` is the program stub `review-change) test -s impl/README.md`
(`driver/examples/sdc/tools/sdc-tool.sh:279`, producer `reviewer-stub`,
`driver/examples/sdc/sdc-story.bindings.json:79-90`), so every review reads approved, and class
REVIEW calls no model (`docs/driver.md:353-354`). The retry path exists: `change.implement` retries
on `code_review: rejected`, `max_runs` 3 (`sdc-story.bindings.json:50-58`).

Trial 3's mixed setting cannot claim "Sonnet for reviews" before this lands
(`story:clean-unattended-run-compare`). `story:independent-reviews` holds the review record's
validation (`sdc-review-check`); this story supplies the model call. The reply follows the
structured-output rule (`story:llm-node-structured-output`).

## Acceptance

A stub cycle with a scripted REVIEW reply that rejects once then approves records two `code_review`
records, the first rejected with findings citing file, line and a requirement or acceptance line,
the second approved, under a principal other than the implementer; and a real call on a recorded
change returns a reply that validates against the review schema.

## Scope

- `driver/examples/sdc/tools/sdc-tool.sh` (`review-change`)
- `driver/examples/sdc/tools/llm-node.sh`
- `driver/examples/sdc/sdc-story.bindings.json`
- `driver/examples/sdc/tools/node-classes.json`

## Ambiguities

- Whether the bound on review rounds is the existing `max_runs` of `change.implement` or a separate
  count on `change.review`.
- `spec.review` is the same stub kind; out of scope here.
