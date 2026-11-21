---
format: aep.planning-md/3
id: story:requester-kpis
kind: story
status: proposed
title: Requester-side KPIs G1 to G6
summary: Questions answered from goals, agreement with the persona, answers with no ground, questions a goal already answered.
tags:
- requester
relations:
- decomposes: epic:requester-side-and-goals-context
- depends_on: story:interview-derived-from-goals
- depends_on: story:eval-requester-persona
scope:
- confidence: inferred
  path: docs/kpis.md
- confidence: inferred
  path: driver/src/bin/sdc-finding-kpis.rs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:38Z", actor: "human:timo", revision: 3}
---
# Story: requester-kpis

## Outcome

The requester side is measured: G1 answered from goals without a person, G2 agreement with the
persona, G3 answers with no ground, G4 asked though a goal answered it, G5 answers set aside by a
goals change, G6 constraint findings (`docs/requester.md` § 6).

## Acceptance

`sdc-finding-kpis <run dir> --json` prints G1 to G6 for a recorded stub run of the trial-2 fixture
with the example goals at A3 and the values equal the hand count written in the fixture (G1 3 of 4,
G3 1 of 4, G4 0), and `docs/kpis.md` holds a § 6 "Requester side" with the six rows.

## Scope

`driver/src/bin/sdc-finding-kpis.rs` and `docs/kpis.md`. G2 reads the eval report of
`story:eval-requester-persona`. Waits for the running trial to end.
