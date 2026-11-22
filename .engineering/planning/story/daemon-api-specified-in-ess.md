---
format: aep.planning-md/3
id: story:daemon-api-specified-in-ess
kind: story
status: active
title: The Runbook daemon API is specified in ESS before any handler exists
summary: 'Domain sdc.daemon: runs, sessions, decision requests, run events, their commands, views and scenarios; OpenAPI and JSON Schema generated from it.'
scope:
- confidence: cited
  path: ess
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T02:01:12Z", actor: "human:operator", revision: 2, correlation: "operator-requirement-2026-11-22-one-binary"}
- {from: "proposed", to: "active", at: "2026-11-22T02:01:12Z", actor: "human:operator", revision: 3, correlation: "operator-requirement-2026-11-22-one-binary"}
---
# Story: daemon-api-specified-in-ess

## Outcome

The API of the Runbook daemon (`runbookd`, the same binary as `runbook serve`) is specified in ESS
before any handler exists: the domain `sdc.daemon` in `ess/domains/daemon.yaml` holds runs,
sessions, decision requests and run events, the commands that move them, the views that read them,
and nine scenarios in the house form. The OpenAPI document and the JSON Schemas are generated from
it. This is step B of the order in the design sheet (decision 5 of the operator's requirement of
2026-11-22: "All of this should be ESS specified, of course.").

## Context

The daemon keeps many runs at once, in named sessions, and a client follows a run's events over
SSE and resumes with `Last-Event-ID`. The runtime behind the daemon is a trait (a stub in the
tests, Loom later), so the specification names the runtime's reports as commands of an actor
`Runtime`: a run's lifecycle needs a cause for every transition, and the runtime is that cause.

## Acceptance

- `ess specify validate --path ess` exits 0 and prints `sdc v1 — 4 file(s), 12 scenario(s), valid`
  with no warning: the system holds `sdc.decision` and `sdc.daemon`.
- The entities `Run`, `Session`, `DecisionRequest` and `RunEvent` exist with the states named in
  the requirement; the commands `SubmitRun`, `CancelRun`, `AnswerDecision`, `OpenSession` and
  `CloseSession` exist with their refusals named (`BelowConfinementFloor`, `RunNotWaiting`,
  `DecisionAlreadyAnswered`, `RunAlreadyEnded`, `SessionClosedToRuns`).
- The nine authored scenarios of `ess/scenarios/daemon-*.yaml` compile with
  `ess verify conform author` and pass on the `interpreted` target.
- `ess generate --kind openapi` and `--kind schema` of `ess` write the committed files under
  `ess/generated/` and `ess generate ... --check` exits 0 for both.
- The existing `driver/tests/decision_request.rs` stays green.
- The change touches `ess/`, the planning store and nothing that runs: no handler is written.
- Known, not accepted as fine: `ess verify conform synthesize` of `ess` refuses 85 outcomes where
  the base refused 2 (row-set guards, parameterised run views, the run lifecycle chain). It takes
  no longer than the base. A follow-up story removes the refusals; the authored scenarios cover
  the behaviour meanwhile.

## Scope

`ess/` only (system, domain, scenarios, one component, generated output) and this story.

## Open questions for the owner (UNMAPPED in the domain)

- The names of the confinement levels: `unconfined`, `workspace`, `strict` are placeholders to be
  aligned with the runtime; only their order is decided.
- The typed members of a run's settings; the form of an intake or context reference; the runtime's
  record kinds; whether a run outcome is a closed set; what identifies a client.
- Whether the daemon's decision request is the `sdc.decision` request itself or a mirror of it.
- Per-session and whole-daemon event streams (decision 4 of the design sheet): the events are
  ordered per run only.
- What happens to the open decision requests of a cancelled run (they stay Open).

## Acceptance note

- Accepted on creation from the operator requirement of 2026-11-22 (correlation `operator-requirement-2026-11-22-one-binary`), by the standing rule of the repository `AGENTS.md`.
