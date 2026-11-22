---
format: aep.planning-md/3
id: story:requirements-trace-to-behaviour
kind: story
status: proposed
title: Requirements trace to spec declarations that carry behaviour
summary: a program traces every requirement to behaviour-carrying spec declarations; a requirement citing only an empty domain fails the spec stage
tags:
- trial-3
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: inferred
  path: driver/examples/sdc/sdc-spec.bindings.json
- confidence: inferred
  path: driver/examples/sdc/tools/sdc-tool.sh
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T06:32:49Z", actor: "human:operator", revision: 4, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: requirements-trace-to-behaviour

## Outcome

Every requirement traces, by program, to specification declarations that carry behaviour; a
requirement that cites only an empty domain fails the spec stage.

## Context

Trial 3, run `runs/trial-3-haiku-3` (all Haiku, autopilot): the decider rejected the release because
sign-in (r3) and the offline copy (r4) were not built and sync (r5) was half built
(`run-stopped.json` `reason`). No earlier check noticed: requirements were r1..r6, the spec has no
command for r3 or r4, and `plan.cover` still read covered (attempt 3 notes, line 10), because
`plan-cover-shape` checks that every compiled command is named by one story
(`driver/examples/sdc/tools/sdc-tool.sh:256-262`), not that every requirement has a command.

In implementation on branch `fix/requirement-trace`; the design is that branch's, not repeated
here.

## Acceptance

On a fixture whose spec gives a requirement only an empty domain (no command, view or invariant),
the spec stage's trace check exits non-zero naming that requirement and the spec stage does not
pass; on trial 3's spec it names r3 and r4.

## Scope

- `driver/examples/sdc/tools/sdc-tool.sh`
- `driver/examples/sdc/sdc-spec.bindings.json`

## Ambiguities

- Which declarations count as carrying behaviour (command, view, invariant, scenario) is the
  branch's decision.
