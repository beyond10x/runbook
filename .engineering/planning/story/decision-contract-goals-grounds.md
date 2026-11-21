---
format: aep.planning-md/3
id: story:decision-contract-goals-grounds
kind: story
status: proposed
title: Goals grounds and free answers in the decision contract
summary: A ground can cite one goals entry, the grounding check sees it move, and a question can be decided with a free answer.
tags:
- requester
relations:
- decomposes: epic:requester-side-and-goals-context
- depends_on: story:goals-format-check
scope:
- confidence: inferred
  path: decision/examples/goals-grounds
- confidence: inferred
  path: ess/domains/decision.yaml
- confidence: inferred
  path: tools/decision-grounds.sh
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:35Z", actor: "human:timo", revision: 4}
---
# Story: decision-contract-goals-grounds

## Outcome

A recommendation or answer can rest on one goals entry, and the grounding check knows when that
entry moved; a question request can be decided with a free answer, as both of the operator's trial-2
answers were (`runs/trial-2-train/store/IDEA-1/children/IDEA-1.intake/work/questions.yaml:11`, `:33`).

## Acceptance

`tools/decision-grounds.sh` exits 0 on a fixture proposal citing entry `product/p1` of `goals` at
the current revision and entry digest, and exits 1 naming the cause on the same proposal after the
entry's statement changed, after the entry was removed, and at a revision that is not current;
`ess specify validate --path ess` passes with `GroundSource` value `goals`,
`ArtifactRevisionCitation.entry` and an optional `answer` on `Decide` for question requests.

## Scope

`ess/domains/decision.yaml` and `tools/decision-grounds.sh`, fixtures under `decision/examples/`.
The contract file is being edited by another session: coordinate before the change.

## Tooling note

- Changes the existing bash tool `tools/decision-grounds.sh`: allowed, a fix to prototype glue; no new bash tool.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)
