---
format: aep.planning-md/3
id: story:protocols-declare-revises-and-authors
kind: story
status: proposed
title: Protocols declare revises, author and decided_by
summary: every writer action declares revises and author; every judgement may_produce declares decided_by
tags:
- blocked-upstream
- canon
relations:
- decomposes: epic:kernel-and-runtime-uptake
scope:
- confidence: inferred
  path: docs/protocol-notes.md
- confidence: inferred
  path: protocol/compiled
- confidence: inferred
  path: protocol/sdc-direction.protocol.yaml
- confidence: inferred
  path: protocol/sdc-epic.protocol.yaml
- confidence: inferred
  path: protocol/sdc-intake.protocol.yaml
- confidence: inferred
  path: protocol/sdc-plan.protocol.yaml
- confidence: inferred
  path: protocol/sdc-release.protocol.yaml
- confidence: inferred
  path: protocol/sdc-spec.protocol.yaml
- confidence: inferred
  path: protocol/sdc-story.protocol.yaml
- confidence: inferred
  path: protocol/sdc.protocol.yaml
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T06:26:12Z", actor: "human:timo", revision: 12, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: protocols-declare-revises-and-authors

## Outcome

Every writer action in the eight protocols declares `revises` and `author`, and every `may_produce`
entry that is a judgement declares `decided_by`.

## Context

Canon session, 2026-11-21: `protocol/1` now accepts `revises`, `author` and `decided_by`, with no
evaluation meaning yet. They are what Canon's revise step (`simulate-revise-step`, wave 8) reads;
without them `canon simulate` cannot tell a failed check on an artifact the protocol revises (46 of
the 87 findings at `f364b6b`) from an SDC defect. On Canon branch `wave/2026-11-20` at `49ff2ce`,
unreleased.

## Acceptance

With a Canon release accepting these fields pinned, `canon validate` accepts all eight protocols
with `revises` and `author` on every writer action and `decided_by` on every judgement
`may_produce`, `scripts/check-protocols.sh` reports the same state counts as before, and
`docs/protocol-notes.md` holds a table of action to revises.

## Scope

- `protocol/sdc.protocol.yaml`, `protocol/sdc-intake.protocol.yaml`,
  `protocol/sdc-direction.protocol.yaml`, `protocol/sdc-epic.protocol.yaml`,
  `protocol/sdc-spec.protocol.yaml`, `protocol/sdc-plan.protocol.yaml`,
  `protocol/sdc-story.protocol.yaml`, `protocol/sdc-release.protocol.yaml`
- `protocol/compiled/` (regenerated IR)
- `docs/protocol-notes.md`

## Ambiguities

- Which `may_produce` entries are judgements (decided by a principal) and which are measurements is
  decided per entry; the table records the choice.
