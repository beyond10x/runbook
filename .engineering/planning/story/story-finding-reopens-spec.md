---
format: aep.planning-md/3
id: story:story-finding-reopens-spec
kind: story
status: proposed
title: A story-stage finding that changes the spec reopens the spec case
tags:
- conductor-succession
- trial-2
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: inferred
  path: cases/21-story-finding-reopens-spec
- confidence: inferred
  path: protocol/compiled/sdc-epic.ir.json
- confidence: inferred
  path: protocol/compiled/sdc-story.ir.json
- confidence: inferred
  path: protocol/sdc-epic.protocol.yaml
- confidence: inferred
  path: protocol/sdc-story.protocol.yaml
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:36:14Z", actor: "human:operator", revision: 3, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: story-finding-reopens-spec

## Outcome

A decision taken in the story stage that changes the specification reaches the specification, so
the conformance suite tests what was decided.

## Context

Trial 2: f9 (a session cookie) amended r1 and r2, but no action in `sdc.story` writes `spec`, so the
suite stayed the one derived before the decision (`docs/trial-2.md:489-494`, R7). The run stopped
on f16 (a read command), whose option 2 changes the spec and cannot be applied from the story case
(`docs/trial-2.md:516-527`, R8).

## Acceptance

In `cases/21-story-finding-reopens-spec`, a story-stage finding decided with a spec-changing
option makes `spec.author` admissible in the spec case, and the new spec revision, once through the
ESS gate and the spec review, makes the story's implementation and conformance evidence
inapplicable, so `change.implement` runs against it; a decision that changes no specification
leaves the spec case closed.

## Scope

`sdc.story` and `sdc.epic` protocols and their IR, one fixture case.

## Risks

- A child's outcome cannot be invalidated (`docs/protocol-notes.md`, limitation 3); reopening may
  need a new spec child per revision, like a revised subject today.
