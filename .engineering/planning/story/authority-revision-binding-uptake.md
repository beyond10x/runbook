---
format: aep.planning-md/3
id: story:authority-revision-binding-uptake
kind: story
status: proposed
title: Use Canon's revision-bound authority
tags:
- blocked-upstream
- canon
- conductor-succession
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: inferred
  path: protocol/compiled/sdc-direction.ir.json
- confidence: inferred
  path: protocol/sdc-direction.protocol.yaml
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:25:40Z", actor: "human:operator", revision: 3, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: authority-revision-binding-uptake

## Outcome

An approval is Canon authority bound to the revision it approved, not an evidence record standing in
for it.

## Context

Canon authority is not bound to a revision, so the SDC records approvals as revision-bound evidence
(`docs/protocol-notes.md:174-178`; `docs/driver.md` gap 1).

## Acceptance

With the Canon release carrying the binding, `roadmap.approve` in `sdc.direction` grants authority
for the roadmap revision it names, a revised roadmap makes `roadmap.approved` false without an
evidence record, and `scripts/check-protocols.sh` exits 0.

## Scope

`sdc.direction` first; the other approval kinds follow the same change.
