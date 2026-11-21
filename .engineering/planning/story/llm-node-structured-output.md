---
format: aep.planning-md/3
id: story:llm-node-structured-output
kind: story
status: proposed
title: LLM nodes with data output are called with a response schema
tags:
- conductor-succession
- operator-request
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: inferred
  path: docs/driver.md
- confidence: inferred
  path: driver/examples/sdc/schemas
- confidence: inferred
  path: driver/examples/sdc/tools/decider.sh
- confidence: inferred
  path: driver/examples/sdc/tools/llm-node.sh
- confidence: inferred
  path: driver/src/bin/sdc-reply-check.rs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:58:39Z", actor: "human:timo", revision: 3, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: llm-node-structured-output

## Outcome

Every LLM node whose output is a data document is called with a response schema, so the reply is
the document in the required shape or the call fails; the node's program check stays as the second
line. Free-form artifacts (source code, prose documents, the multi-file ESS specification) are
excluded and listed.

## Context

The operator, 2026-11-21: "When doing LLM calls with the LLM node, make sure we're using sort of
structured output for reasonable cases. Yeah, so everything needs to be as deterministic and
reliable as possible."

- Trial 3, attempt 1: both runs stopped at `context.approve` after 5 calls, because the decider's
  replies cited ids that do not exist (`context.idea_excerpt`, `context.idea`, `context.platform`,
  `context.kind`, `context.offline`; `runs/trial-3-haiku/console.txt:3-10`, a run record not in
  git). The decider part is being fixed on branch `fix/decider-citations`; this story does not
  duplicate it, it gives the decider's reply the same schema mechanism.
- Trial 2, R2: `questions.answer`'s output was refused by its apply check three times
  (`changed a scope-changing question: not applied`), which spent the writer and left the interview
  open (`docs/trial-2.md:368-378`). No other writer failure in trial 2 was a shape failure; the
  others were tool or lifecycle defects (R3, R6).

Nodes in `driver/examples/sdc/tools/llm-node.sh` and their outputs:

| node | line | output | schema or excluded |
|---|---|---|---|
| `idea.dissect` | :155 | `dissection.yaml` | schema |
| `questions.write` | :167 | `questions.yaml` | schema |
| `questions.answer` | :180 | `questions.yaml` (edited in place) | schema |
| `requirements.normalise` | :188 | `requirements.yaml` | schema |
| `context.default` | :201 | `context.yaml` | schema |
| `vision.write` | :211 | `vision.json` + `vision.md` | schema for the JSON; the Markdown body is prose, excluded |
| `roadmap.write` | :217 | `roadmap.json` + `roadmap.md` | schema for the JSON; the Markdown body is prose, excluded |
| `spec.author` | :227 | `spec/` tree | excluded: a multi-file ESS specification, validated by `ess specify validate` |
| `plan.decompose` | :244 | `stories.json` | schema |
| `change.implement` | :253 | `impl/` + optional `findings.yaml` (:277-283) | code excluded; schema for `findings.yaml` |
| decider (`decider.sh`) | | the decision reply | schema (with `fix/decider-citations`) |

## Acceptance

For each node marked "schema" above, a schema file exists under `driver/examples/sdc/schemas/`, is
passed on the call and is the one the node's check validates the reply against; a recorded-reply
cycle shows a shape-invalid reply retried with the validator's error at most twice and then failing
with a named cause; every LLM call record states whether the reply was schema-constrained; a stub
trial run still closes; and `docs/driver.md` lists every node as "schema" or "excluded, and why".

## Scope

`driver/examples/sdc/tools/llm-node.sh` and `driver/examples/sdc/tools/decider.sh` (existing bash:
fixes), new schema files under `driver/examples/sdc/schemas/`, a Rust validator binary
(`driver/src/bin/sdc-reply-check.rs`) if no existing program validates JSON against a schema,
`docs/driver.md`.

## Ambiguities

- Whether the metaharness adapter can pass a response schema, and which models accept one: to be
  verified from the adapter's and the provider's documentation, not assumed.
- Whether schemas are generated from the ESS specification where a contract exists (`ess generate
  schema`), for example the decision reply of `ess/domains/decision.yaml`.
- Nodes that write YAML today: ask for JSON under the schema and convert to YAML by program, or
  keep YAML and validate after parsing.
