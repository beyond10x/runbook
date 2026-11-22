---
format: aep.planning-md/3
id: story:ui-decision-request
kind: story
status: proposed
title: Drawer and Now band show the decision request
relations:
- decomposes: epic:decisions-a-person-can-take-cold
- depends_on: story:decision-request-from-question
scope:
- confidence: inferred
  path: ui/decisions.mjs
- confidence: inferred
  path: ui/test/decisions.spec.mjs
- confidence: inferred
  path: ui/test/now.spec.mjs
- confidence: inferred
  path: ui/test/scope-fixtures
- confidence: inferred
  path: ui/web/inbox.js
- confidence: inferred
  path: ui/web/now.js
- confidence: inferred
  path: ui/web/style.css
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:38Z", actor: "human:operator", revision: 3}
---
# Story: ui-decision-request

## Outcome

A person who opens only the Decisions drawer can decide: the drawer, and the Now band while a run
waits, show the same request the command line prints.

## Acceptance

In a Playwright run over the trial-2 scope fixture, the drawer's item for a scope-changing question
shows the context line, the idea excerpt, each option with consequence and cost, the recommendation
with grounds and the what-happens-next line, and the Now band's waiting state shows the question
with a link that opens that drawer item; a screen-only check, reading no other panel, finds all
five.

## Scope

The scope block of the drawer (`ui/web/inbox.js`, `scopeBlock`), the pending-item builder
(`ui/decisions.mjs`), the Now band's waiting state (`ui/web/now.js`, `waitOf`, `renderNow`), the
styles, and the scope fixtures (`ui/test/scope-fixtures/`, `ui/test/decisions.spec.mjs`,
`ui/test/now.spec.mjs`). The server builds the request with `sdc-decision-request`.

## Tooling note

- New tool: the Rust binary `sdc-decision-request` (`driver/src/bin/sdc-decision-request.rs`), in place of `tools/decision-request.sh` named before.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)
