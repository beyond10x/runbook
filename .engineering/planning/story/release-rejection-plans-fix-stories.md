---
format: aep.planning-md/3
id: story:release-rejection-plans-fix-stories
kind: story
status: proposed
title: A rejected release plans fix stories and builds again, at most twice
summary: after a rejected release the epic plans fix stories from the reasons and builds a new release, at most twice, then ends with a truthful outcome
tags:
- trial-3
relations:
- decomposes: epic:conductor-succession
- informed_by: story:canon-slice-bound-uptake
scope:
- confidence: inferred
  path: driver/examples/sdc/sdc-epic.bindings.json
- confidence: inferred
  path: driver/examples/sdc/sdc-plan.bindings.json
- confidence: inferred
  path: driver/examples/sdc/sdc-release.bindings.json
- confidence: inferred
  path: protocol/compiled
- confidence: inferred
  path: protocol/sdc-epic.protocol.yaml
- confidence: inferred
  path: protocol/sdc-plan.protocol.yaml
- confidence: inferred
  path: protocol/sdc-release.protocol.yaml
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T06:32:49Z", actor: "human:timo", revision: 9, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: release-rejection-plans-fix-stories

## Outcome

After a rejected release the epic plans fix stories from the rejection's reasons and builds a new
release, at most twice; then the tree ends with a truthful outcome.

## Context

Trial 3, run `runs/trial-3-haiku-3`: stopped by `rule:autopilot`, exit 6, cause
`rejected_without_rework_path`, case `IDEA-1.app.release`, action `release.publish` (attempt 3
notes, line 4); `run-stopped.json`: "the protocol has no action that reworks a rejected revision
with the reasons". The decider's reasons named r3 and r4 not built and r5 half built.

Step 1 (the tree ends by itself: release withheld, epic and root end with a named outcome) is in
work on branch `fix/roadmap-read-precondition`. This story is step 2.

## Acceptance

On a fixture run whose first two releases are rejected with reasons, the epic records fix stories
derived from each rejection's reasons and a new release after each, and after the second rejection
the release, epic and root end with a named outcome instead of a stop with cause
`rejected_without_rework_path`.

## Scope

- `protocol/sdc-epic.protocol.yaml`, `protocol/sdc-plan.protocol.yaml`,
  `protocol/sdc-release.protocol.yaml`, `protocol/compiled/`
- `driver/examples/sdc/sdc-epic.bindings.json`, `driver/examples/sdc/sdc-plan.bindings.json`,
  `driver/examples/sdc/sdc-release.bindings.json`

## Ambiguities

- `sdc.plan` sits exactly at Canon's 65,536 state bound (`docs/protocol-notes.md:156-157`); a
  replanning path through it may not fit until a Canon release checks the bound per slice
  (`story:canon-slice-bound-uptake`).
- Whether fix stories are planned in `sdc.plan` or in `sdc.epic`.
