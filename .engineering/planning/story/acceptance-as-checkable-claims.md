---
format: aep.planning-md/3
id: story:acceptance-as-checkable-claims
kind: story
status: proposed
title: Acceptance as checkable claims, satisfied by evidence
tags:
- later
- operator-request
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: inferred
  path: driver/src/bin/sdc-acceptance.rs
- confidence: inferred
  path: protocol/compiled/sdc-story.ir.json
- confidence: inferred
  path: protocol/sdc-plan.protocol.yaml
- confidence: inferred
  path: protocol/sdc-story.protocol.yaml
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:16:35Z", actor: "human:operator", revision: 3, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: acceptance-as-checkable-claims

## Outcome

A story's acceptance is something a program runs and evaluates; no LLM judges whether a story is
accepted.

## Context

The operator, 2026-11-21: "the actual acceptance criteria up until this project or an integration
with Canon, those were usually just text-based, whereas now they can basically become more
programmatic, so it's a machinery which can programmatically run and evaluate it, and technically
the thing creating the plan and scoping out the details could, instead of writing a text-based
acceptance criteria, already sort of submit this expression-like thing where we just have to
execute stuff. We don't need an LLM as a judge to basically prove whether the thing is accepted or
not."

What exists: `plan.decompose` writes each story with an `## Acceptance` section in text
(`protocol/sdc-plan.protocol.yaml:115`); nothing evaluates it. A story passes on `gate.passed`,
`change.built`, `tests.pass`, `change.reviewed` and `change.merged`
(`protocol/sdc-story.protocol.yaml:165-175`); `tests.pass` reads `conformance_run`
(`:63-68`), produced by `ess verify conform run` (`docs/protocol-notes.md:82`). ESS conformance is
the verification evidence (`docs/phases.md:36`). A release is accepted on `release_conformance`
`passed` (`protocol/sdc-release.protocol.yaml:55-60`).

## Acceptance

On a fixture story whose plan carries acceptance as claims (a command with expected exit, a file
fact, an ESS scenario id, a Canon claim over recorded evidence) plus one text line, `sdc-acceptance`
records one evidence record per claim, the story's acceptance obligation holds only when every
claim's record is `passed`, and the text line is listed as `unchecked` without blocking or
satisfying anything.

## Scope

`sdc.plan` (the decomposer emits claims), `sdc.story` (an acceptance claim and obligation over the
records), a new Rust binary `sdc-acceptance`.

## Ambiguities

- The AEP planning format has no acceptance-claim field: `upstream-blocker:aep-acceptance-claims`
  (the `aep` repository).
- Canon claim forms: today's claims over evidence kinds and results (`protocol/sdc-story.protocol.yaml:50-85`)
  seem enough for one aggregate `acceptance` kind; a claim per acceptance line would need a form
  over a variable set of records (like the `children` form): a question for the Canon session if the
  aggregate proves too coarse.
- State budget: one more evidence kind in `sdc.story` (12,288 states) costs at least ×4.

