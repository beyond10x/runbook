---
format: aep.planning-md/3
id: epic:live-view
kind: epic
status: proposed
title: 'Live view: checks, plan items and spec changes rendered'
summary: Operator requests of 2026-11-21 for the run's live view; low priority.
tags:
- later
- operator-request
relations:
- decomposes: initiative:roadmap
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:16:34Z", actor: "human:timo", revision: 2, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Epic: live-view

## Outcome

The live view of a run (`ui/`, `ui/README.md`) shows what is running and what changed in terms a
reader understands at a glance: checks and gates as running, passed or failed; planning items and
the roadmap rendered, not raw; specification changes with their semantic classification.

## Why Now

Operator requests of 2026-11-21, filed so they are not forgotten; low priority ("it's not high
priority").

## Scope

`ui/server.mjs`, `ui/web/` (Now band, What changed, a Plan view), their Playwright specs. The data
comes from records the run already writes or from programs (`aep`, `ess`); never from an LLM.

## Out of Scope

The area board for orchestration (`epic:area-board`).

## Done When

The stories under it are implemented.
