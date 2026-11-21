---
format: aep.planning-md/3
id: story:ui-view-data-per-runbook
kind: story
status: active
title: The run inspector reads its runbook's phases, file kinds and tool owners from a view file
relations:
- decomposes: epic:live-view
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: ui/README.md
- confidence: cited
  path: ui/decisions.mjs
- confidence: cited
  path: ui/server.mjs
- confidence: cited
  path: ui/test
- confidence: cited
  path: ui/web
- confidence: cited
  path: views
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T22:50:54Z", actor: "human:operator", revision: 2, executor: "agent:wave-rb1-ui-view-data", correlation: "operator-standing-acceptance-2026-11-21"}
- {from: "proposed", to: "active", at: "2026-11-21T22:50:54Z", actor: "human:operator", revision: 3, executor: "agent:wave-rb1-ui-view-data", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: ui-view-data-per-runbook

## Outcome

The run inspector works for any runbook, not only SDC. What the UI knows about one runbook (its
phases, what a file in a case means, which program a tool needs) is data the runbook ships:
`views/sdc.view.json` (format `runbook.view/1`, keys `phases`, `file_kinds`, `tool_owners`). The
server takes `--view <file>` (default `views/sdc.view.json` beside the repository root, when it
exists) and serves it at `/api/view`; the web code and the preflight read it. With no view file the
UI still works: one flat list of the protocols found in the store (no phase grouping), the file
name as the label of every file, no tool owner. No SDC protocol id, file name or tool name stays
in `ui/web/lenses.js`, `ui/web/meaning.js` or `ui/decisions.mjs`.

Answers the operator's question of 2026-11-21: "this whole UI we have built around SDC, now called
Runbook, will it work for different protocols as well?"

## Acceptance

- With `views/sdc.view.json` the UI's existing tests pass with the same assertions (same rendering
  for the SDC runbook).
- A test over a small second runbook (a store whose protocol ids are not SDC's) and no view file:
  the page renders its protocols, actions and files, with no JavaScript error and no SDC phase
  label.
- The same store with a two-phase view file: the phases of the file are shown.
- `grep -n "sdc-" ui/web/lenses.js ui/web/meaning.js ui/decisions.mjs` is reported with a reason for
  every remaining hit.
- `ui/README.md` describes the view file, `CHANGELOG.md` has a line under Unreleased, and
  `aep plan artifact validate` passes.

## Scope

`views/sdc.view.json`, `ui/server.mjs`, `ui/decisions.mjs`, `ui/web/`, `ui/test/`, `ui/README.md`,
`CHANGELOG.md`. Not `website/`, `driver/`, `bindings/`, `scripts/`.
