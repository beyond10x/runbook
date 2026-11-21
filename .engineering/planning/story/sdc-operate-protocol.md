---
format: aep.planning-md/3
id: story:sdc-operate-protocol
kind: story
status: proposed
title: 'sdc.operate: open a release from a delivered tree and wake an operational agent'
summary: a later protocol that opens a standalone sdc.release for a delivered tree and wakes the project's operational agent; not built (D150)
tags:
- orchestration
relations:
- decomposes: epic:orchestrate-area-case
scope:
- confidence: inferred
  path: driver/examples/sdc/sdc-operate.bindings.json
- confidence: inferred
  path: protocol/sdc-operate.protocol.yaml
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T10:49:03Z", actor: "human:timo", revision: 2, executor: "agent:sdc-delivered", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: sdc-operate-protocol

## Outcome

A protocol `sdc.operate` that takes a delivered tree, opens a standalone `sdc.release` for it and
wakes an operational agent for the project, so that publishing and operating run as a protocol of
their own after the development cycle and never inside it.

## Context

Ruling D150 (the operator, 2026-11-21): the development cycle ends `delivered` (stories merged,
built, acceptance passed against the spec); "operate is sort of an endless thing, and it's
basically a protocol on its own. It would usually just dispatch or wake up an operational agent
for this project… we should not deal with that now." Since D150 `sdc.release` runs alone from a
delivered tree (`scripts/run-trial.sh <name> --release <delivered run>`, `delivery.read` copies the
acceptance record; `docs/protocol-notes.md`, "The cycle ends delivered") and nothing opens it.

## Acceptance

`scripts/check-protocols.sh` checks `protocol/sdc-operate.protocol.yaml` within the 65,536-state
bound; a stub run from a delivered tree opens one `sdc.release` case through `sdc.operate` and
records the wake-up of the operational agent as evidence, and the delivered tree's records are
unchanged.

## Scope

A new protocol under `protocol/`, its bindings in `driver/examples/sdc/`, fixture cases under
`cases/`. Not part of any eval run.

## Ambiguities

- What the operational agent is (a Loom-run agent, a conductor instance) and what wakes it is not
  decided; the operate loop is endless, so the protocol's outcomes (handed over, withheld) need a
  decision of their own.
