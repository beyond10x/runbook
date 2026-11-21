---
format: aep.planning-md/3
id: story:ui-concurrent-writers
kind: story
status: proposed
title: Store watcher for concurrent writers
tags:
- orchestration
relations:
- decomposes: epic:area-board
- depends_on: story:per-case-lock
- depends_on: story:area-board-shell
scope:
- confidence: inferred
  path: ui/server.mjs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:38Z", actor: "human:timo", revision: 3}
---
# Story: ui-concurrent-writers

## Outcome

The live view stays correct while several children are written at once, instead of assuming one
`sdc-drive` lock per tree (`docs/orchestration.md` § 5, "Server").

## Acceptance

With two writers appending events to two sibling cases at once, the server's event stream
delivers every event of both cases, each exactly once, in per-case order.

## Scope

`ui/server.mjs`. `ui/web/now.js` needs no change (*inferred*, § 5).
