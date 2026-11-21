---
format: aep.planning-md/3
id: story:orchestrate-protocol-adopt
kind: story
status: proposed
title: Adopt sdc.orchestrate into the protocol family
tags:
- orchestration
relations:
- decomposes: epic:orchestrate-area-case
scope:
- confidence: inferred
  path: cases/05-orchestrate-idle
- confidence: inferred
  path: driver/examples/sdc/sdc-orchestrate.bindings.json
- confidence: inferred
  path: protocol/compiled/sdc-orchestrate.ir.json
- confidence: inferred
  path: protocol/probes/sdc-orchestrate.gate-probe.yaml
- confidence: inferred
  path: protocol/sdc-orchestrate.protocol.yaml
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:36Z", actor: "human:timo", revision: 4}
---
# Story: orchestrate-protocol-adopt

## Outcome

`sdc.orchestrate` is a checked member of the family, bound to stub programs, so later stories each
replace one stub.

## Context

Draft: `protocol/draft/orchestrate.protocol.yaml`, outside `protocol/*.protocol.yaml`, so
`scripts/check-protocols.sh` skips it; no bindings, no probe (`docs/orchestration.md` § 6).

## Acceptance

`scripts/check-protocols.sh` and `scripts/check-bindings.sh` exit 0 with `sdc.orchestrate` in
their output, `canon check` reports at most 65,536 states and 0 findings, and
`cases/05-orchestrate-idle` evaluates with only `intake.route` admissible.

## Scope

`protocol/sdc-orchestrate.protocol.yaml`, its compiled IR and gate probe, a new
`driver/examples/sdc/sdc-orchestrate.bindings.json` (every action bound to a stub), one fixture case.

## Out of Scope

Real programs for each action (their own stories).

## Canon children form

Canon accepted request 14 as Canon `story:child-outcome-subject` (Canon wave 8, after
`story:child-outcome-revision`, wave 7): `{children: {in: [passed], subject: wave, revision: current}}`
reads only the `case_outcome` records about `wave` at its current revision
(`docs/orchestration.md` § 2.6). Not needed here: the draft protocol works without it through
`wave_result`, and the form does not replace `wave_result` where that record also states that the
integration gate passed. So no upstream blocker. The runner's duty the form relies on (an `open`
`case_outcome` per opened child, `subject` = the `case_open` subject) `sdc-drive` meets today
(`driver/src/main.rs:491`, `:503-507`).
