---
format: aep.planning-md/3
id: story:simulate-as-a-protocol-gate
kind: story
status: proposed
title: canon simulate as a protocol gate
summary: check-protocols.sh runs canon simulate on all eight protocols and fails on a rising finding count
tags:
- blocked-upstream
- canon
relations:
- decomposes: epic:kernel-and-runtime-uptake
scope:
- confidence: inferred
  path: protocol/simulate-expected.json
- confidence: inferred
  path: scripts/check-protocols.sh
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T06:26:12Z", actor: "human:timo", revision: 4, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: simulate-as-a-protocol-gate

## Outcome

`scripts/check-protocols.sh` runs `canon simulate --path <protocol.yaml>` on all eight protocols
and fails on a finding that is an SDC defect.

## Context

Canon session, 2026-11-21. `canon simulate` exits 0 on no finding, 1 on a finding or refusal, 2 on
an unreadable protocol. Output: the findings; then, on a protocol with a `case_open` action, one
line starting `limit:`; then `simulated: protocol ... N reachable states, T terminable (...),
[W waiting on supplied evidence or expiry,] F findings (S stuck or cycle states)`. It is on Canon
branch `wave/2026-11-20` at `49ff2ce`, unreleased.

On SDC's protocols at commit `f364b6b`: 87 findings, classified by Canon as 46 a failed check on an
artifact the protocol revises, 24 on an artifact another case revises, 7 a guard gap in `sdc.plan`,
2 two approval records, 4 lasting denials, 4 false findings of the walk.

Until Canon releases `simulate-revise-step` (wave 8), `simulate-record-groups` (wave 7) and
`simulate-child-outcome-step` (wave 6), most findings are walk artifacts, not SDC defects.

## Acceptance

With a Canon release carrying `canon simulate` pinned, `scripts/check-protocols.sh` prints each
protocol's `simulated:` summary line, compares each protocol's finding count with
`protocol/simulate-expected.json`, and exits non-zero when a count rises above the committed one;
after Canon's waves 6-8 are released it exits non-zero on any finding in a class that is an SDC
defect.

## Scope

- `scripts/check-protocols.sh`
- `protocol/simulate-expected.json` (new: expected finding counts per protocol)

## Ambiguities

- Which of the 87 findings are SDC defects (the 7 `sdc.plan` guard gaps are candidates) is decided
  per class when the waves land; the count ratchet holds until then.
