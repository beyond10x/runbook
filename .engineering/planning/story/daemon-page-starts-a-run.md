---
format: aep.planning-md/3
id: story:daemon-page-starts-a-run
kind: story
status: active
title: 'The daemon''s page starts a run: pick a runbook, enter the intake, start'
summary: 'Operate mode of the page: a runbooks view of the daemon and a start form that sends submit-run; the inspector stays the read-only watch mode.'
tags:
- backlog-2026-11-23
relations:
- informed_by: story:ui-embedded-in-the-binary
scope:
- confidence: cited
  path: docs/daemon-api.md
- confidence: cited
  path: ess
- confidence: cited
  path: runbook/src/api
- confidence: cited
  path: runbook/src/commands.rs
- confidence: cited
  path: runbook/src/mcp
- confidence: cited
  path: ui/README.md
- confidence: cited
  path: ui/daemon
- confidence: cited
  path: ui/test
- confidence: cited
  path: ui/web
revision: 13
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T01:25:45Z", actor: "human:operator", revision: 2, executor: "agent:runbook", correlation: "operator-2026-11-23-start-from-page"}
- {from: "proposed", to: "active", at: "2026-11-23T01:25:45Z", actor: "human:operator", revision: 3, executor: "agent:runbook", correlation: "operator-2026-11-23-start-from-page"}
---
## Outcome

The page of the daemon (`runbook serve --ui`) starts a run: the person picks a runbook from a list,
enters the intake and presses Start. The page says which of its two modes it is in: **watch** (the
read-only inspector, `ui/server.mjs`) or **operate** (the daemon's page: start a run, take a
decision).

## Source

- The operator, 2026-11-23, relayed by the consumer session: "shouldn't there be an option to start
  them manually in the runbook thing? ... right now this is the inspector, which is read only. So
  maybe you have two variants of the main view". He wants one page with two modes: watch
  (read-only) and operate (start a run, take decisions).
- Read at `main` `566a0c3f`: the daemon has the command (`POST /daemon/commands/submit-run`,
  `runbook/src/api/routes.rs:92`; input `SubmitRun`, `runbook/src/commands.rs:247-262`) and the
  page has no control that calls it (`ui/web/`, `ui/daemon/decide.js`). The daemon has no view that
  lists the runbooks it can start (`runbook/src/api/routes.rs:18-29`).

## Acceptance

- A view `runbooks` of the daemon lists the runbooks it can start (name and version), from
  `--runbook-home` under `--runtime loom` and from `--runbook-dir` under the stub runtime, behind
  the daemon's one token check. `docs/daemon-api.md` and the ESS specification of the daemon API
  carry it.
- In operate mode the page has a start form: a runbook from that view, the intake as text, the
  confinement (never below the daemon's floor; no level is preset: the person picks one and Start
  stays disabled until then). Start sends `submit-run` with the token from the page's memory and
  an idempotency key that belongs to the filled form: the key stays the same for a press repeated
  after an answer that did not arrive, and changes when the daemon answered or the content of the
  form changed, so one filled form starts at most one run. The new run appears in the run list and
  is selected. A refusal of the daemon is shown with its name and text. A token that cannot be sent
  is forgotten and asked for again; closing the form empties the token field.
- The page's header names the mode: "watch" when the inspector serves it, "operate" when the daemon
  serves it. Under the stub runtime the form says that the run is scripted and nothing is executed.
- The inspector stays read-only: served by `ui/server.mjs` the page has no start form, no token
  field and sends no command (a spec holds this next to `ui/test/read-only.spec.mjs`).
- The read cookie authorises no command. The token lives in page memory only. The content of an
  intake never becomes markup.
- Specs with a stub daemon runtime; no model call. No test over 60 s.

## Scope

`runbook/src/api/`, `runbook/src/commands.rs`, `runbook/src/mcp/`, `ess/`, `ui/web/`, `ui/daemon/`,
`ui/test/`, `docs/daemon-api.md`, `ui/README.md`.

## Not in this story

- The daemon's default runtime (it stays `stub` until the operator decides otherwise).
- Runs of `runbook run` in the daemon's list (`story:daemon-page-watches-run-directories`).
- What a step produced on the daemon's page (the draft `daemon-page-shows-step-products`).
