---
format: aep.planning-md/3
id: story:release-rejection-plans-fix-stories
kind: story
status: proposed
title: A failed acceptance run plans fix stories and delivers again, at most twice
summary: after the delivery stage failed its acceptance run the epic plans fix stories from the reasons and delivers again, at most twice, then ends with a truthful outcome
tags:
- trial-3
relations:
- decomposes: epic:conductor-succession
- informed_by: story:canon-slice-bound-uptake
scope:
- confidence: inferred
  path: driver/examples/sdc/sdc-delivery.bindings.json
- confidence: inferred
  path: driver/examples/sdc/sdc-epic.bindings.json
- confidence: inferred
  path: driver/examples/sdc/sdc-plan.bindings.json
- confidence: inferred
  path: protocol/compiled
- confidence: inferred
  path: protocol/sdc-delivery.protocol.yaml
- confidence: inferred
  path: protocol/sdc-epic.protocol.yaml
- confidence: inferred
  path: protocol/sdc-plan.protocol.yaml
revision: 16
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T06:32:49Z", actor: "human:timo", revision: 9, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: release-rejection-plans-fix-stories

## Outcome

After the delivery stage failed (the acceptance run of the specification's suite failed on the build of the merged changes), the epic plans fix stories from the failure's reasons and builds and accepts again, at most twice; then the tree ends with a truthful outcome.

## Context

Trial 3, run `runs/trial-3-haiku-3`: stopped by `rule:autopilot`, exit 6, cause
`rejected_without_rework_path`, case `IDEA-1.app.release`, action `release.publish` (attempt 3
notes, line 4); `run-stopped.json`: "the protocol has no action that reworks a rejected revision
with the reasons". The decider's reasons named r3 and r4 not built and r5 half built.

Step 1 (the tree ends by itself: release withheld, epic and root end with a named outcome) is in
work on branch `fix/roadmap-read-precondition`. This story is step 2.

Since ruling D150 (2026-11-21) the cycle ends `delivered` and opens no release: the end-of-cycle check that sends work back is `sdc.delivery`'s `acceptance.run`, which today ends `not_accepted` at once (epic `not_accepted`, root `not_delivered`; `docs/protocol-notes.md`, "The cycle ends delivered"). A refused publication is the standalone `sdc.release`'s `withheld` and is not this story. The loop needs a children form that reads only children of the current revision (Canon's `revision: current`), since a closed story cannot be reopened to good effect (limitation 4).

## Acceptance

On a fixture run whose first two acceptance runs fail with reasons, the epic records fix stories derived from each failure's reasons and a new delivery after each, and after the second failure the delivery, epic and root end with a named outcome (`not_accepted`, `not_delivered`).

## Scope

- `protocol/sdc-epic.protocol.yaml`, `protocol/sdc-plan.protocol.yaml`, `protocol/sdc-delivery.protocol.yaml`, `protocol/compiled/`
- `driver/examples/sdc/sdc-epic.bindings.json`, `driver/examples/sdc/sdc-plan.bindings.json`, `driver/examples/sdc/sdc-delivery.bindings.json`

## Ambiguities

- `sdc.plan` sits exactly at Canon's 65,536 state bound (`docs/protocol-notes.md:156-157`); a
  replanning path through it may not fit until a Canon release checks the bound per slice
  (`story:canon-slice-bound-uptake`).
- Whether fix stories are planned in `sdc.plan` or in `sdc.epic`.
