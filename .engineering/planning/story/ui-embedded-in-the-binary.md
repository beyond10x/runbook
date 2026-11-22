---
format: aep.planning-md/3
id: story:ui-embedded-in-the-binary
kind: story
status: proposed
title: UI embedded in the binary and served beside the API
summary: runbook serve --ui serves the embedded ui/web files from the daemon API and SSE; the Node server is removed.
tags:
- one-binary
relations:
- decomposes: epic:runbook-one-binary-and-daemon
- depends_on: story:daemon-skeleton-on-a-runtime-trait
- depends_on: story:engine-on-loom-library-entry
- informed_by: epic:live-view
scope:
- confidence: inferred
  path: runbook/build.rs
- confidence: inferred
  path: runbook/src/ui.rs
- confidence: cited
  path: ui/README.md
- confidence: cited
  path: ui/decisions.mjs
- confidence: cited
  path: ui/package.json
- confidence: cited
  path: ui/server.mjs
- confidence: cited
  path: ui/test
- confidence: cited
  path: ui/udiff.mjs
- confidence: cited
  path: ui/vclock.mjs
- confidence: cited
  path: ui/web
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:47:34Z", actor: "human:operator", revision: 2, executor: "agent:plan-onebin", correlation: "operator-requirement-2026-11-22-one-binary"}
---
# Story: ui-embedded-in-the-binary

## Outcome

`runbook serve --ui` serves the web UI from files embedded in the binary, beside the API, reading
the daemon's API and SSE stream; several runs show at once. The Node server is removed at the end.

## Source

The operator, 2026-11-22: "we want the web UI to be embedded into the binary or into the server. So
when we do runbook serve, or runbookd the daemon, then we should just also specify --ui ... and
then it would ... serve it besides the API itself. And multiple such runs should be possible to run
there."

## Context

The UI is `ui/web/` (static files) with `ui/server.mjs`, `ui/decisions.mjs`, `ui/vclock.mjs` and
`ui/udiff.mjs` as its Node server and Playwright specs under `ui/test/`. Each of the Node server's
routes the UI calls is replaced by an API route or an SSE event of the daemon; where one has no
equal, the missing command goes into the ESS specification first
(`story:daemon-api-specified-in-ess`). The existing UI stories name `ui/server.mjs`; their scope
moves with this one (the orchestrator decides; this story edits none of them).

## Acceptance

- The files of `ui/web/` are compiled into the binary at build time: `runbook serve --ui` started from
  an empty directory with no `ui/` answers `GET /` with 200 and the content of `ui/web/index.html`
  byte for byte, and `GET /app.js` likewise; without `--ui`, `GET /` answers 404.
- The UI reads runs from `GET /daemon/views/runs` and events from the SSE routes of the daemon; with two stub
  runs in two sessions the page lists both and the feed of each shows its own events (a Playwright
  spec, in the existing `ui/test/` set, two workers).
- A decision request of a stub run is answered from the UI's drawer through
  `POST` of the ESS-specified command, and the run continues (Playwright spec).
- The UI route is guarded by the daemon's token rule: on a non-loopback listener a request without
  the token gets 401.
- At the end of the story `git ls-files ui/server.mjs ui/decisions.mjs` prints nothing,
  `grep -rn 'server.mjs\|decisions.mjs' --include='*' . --exclude-dir=node_modules --exclude-dir=.git
  --exclude-dir=.engineering --exclude-dir=docs` prints nothing, and `ui/README.md` and `ui/package.json`
  say how the UI is started now.
- The Playwright step ends within 120 s with no test over 60 s.

## Scope

`ui/web/` (reads the API; static files stay one copy), `ui/server.mjs`, `ui/decisions.mjs`,
`ui/vclock.mjs`, `ui/udiff.mjs` (removed or ported to the daemon where still used), `ui/test/`,
`ui/package.json`, `ui/README.md`, `runbook/src/ui.rs` (new), `runbook/Cargo.toml` (an embedding
crate or `include_dir`), `runbook/build.rs` if needed, `AGENTS.md` (directory map line for `ui/`).
