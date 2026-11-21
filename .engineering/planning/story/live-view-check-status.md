---
format: aep.planning-md/3
id: story:live-view-check-status
kind: story
status: proposed
title: Checks and gates show running, passed or failed in the live view
tags:
- later
- operator-request
relations:
- decomposes: epic:live-view
scope:
- confidence: inferred
  path: ui/server.mjs
- confidence: inferred
  path: ui/test/now.spec.mjs
- confidence: inferred
  path: ui/web/model.js
- confidence: inferred
  path: ui/web/now.js
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:16:34Z", actor: "human:timo", revision: 3, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: live-view-check-status

## Outcome

A reader of the live view sees which check or gate is running now, and then whether it passed or
failed and why, without reading logs.

## Context

The operator, 2026-11-21: "whenever we're running gates or special commands like tests, like cargo
test or something like that, or gates in general, and we know that's a gate, we would want to
visually display that, hey, it's running now, okay, now green check mark, or no, it failed, this
is the error. Just make this visually more intuitive to understand what is currently actually
running."

What exists: an action whose declared `effect` is not `write` is an observation; its binding names
`produces` and a `results` table from exit code to result (`docs/driver.md:87`;
`driver/examples/sdc/sdc-story.bindings.json`, `tests.run`: `"0":"passed","5":"finding","*":"failed"`).
Wall time per event: `observed.jsonl`; LLM call state: `status.json` (`docs/driver.md`, "Observation
records").

## Acceptance

In a recorded run, a step of an observing action shows in the Now band as running with its command
and elapsed time while `action_started` has no `action_finished`, and afterwards as passed (check
mark) or failed with the first error lines of its log and a link to the full output, taken from the
evidence record's result and the step's log.

## Scope

`ui/web/now.js`, `ui/web/model.js`, `ui/server.mjs` (serve the step log), a Playwright spec.

## Ambiguities

- Where a check's own log lives per step (stdout/stderr capture): inferable from the driver's step
  record; if none is kept, the driver must keep the last lines.
