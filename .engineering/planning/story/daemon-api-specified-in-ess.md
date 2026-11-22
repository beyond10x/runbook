---
format: aep.planning-md/3
id: story:daemon-api-specified-in-ess
kind: story
status: active
title: The Runbook daemon API is specified in ESS before any handler exists
summary: 'Domain sdc.daemon: runs, sessions, decision requests, run events, their commands, views and scenarios; OpenAPI and JSON Schema generated from it.'
relations:
- decomposes: epic:runbook-one-binary-and-daemon
scope:
- confidence: cited
  path: ess
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T02:01:12Z", actor: "human:operator", revision: 2, correlation: "operator-requirement-2026-11-22-one-binary"}
- {from: "proposed", to: "active", at: "2026-11-22T02:01:12Z", actor: "human:operator", revision: 3, correlation: "operator-requirement-2026-11-22-one-binary"}
---
# Story: daemon-api-specified-in-ess

## Outcome

The API of the Runbook daemon (`runbookd`, the same binary as `runbook serve`) is specified in ESS
before any handler exists: the domain `sdc.daemon` in `ess/domains/daemon.yaml` holds runs,
sessions, decision requests and run events, the commands that move them, the views that read them,
and ten scenarios in the house form. The OpenAPI document and the JSON Schemas are generated from
it. This is step B of the order in the design sheet (decision 5 of the operator's requirement of
2026-11-22: "All of this should be ESS specified, of course.").

## Context

The daemon keeps many runs at once, in named sessions, and a client follows a run's events over
SSE and resumes with `Last-Event-ID`. The runtime behind the daemon is a trait (a stub in the
tests, Loom later), so the specification names the runtime's reports as commands of an actor
`Runtime`: a run's lifecycle needs a cause for every transition, and the runtime is that cause.

## Acceptance

Met, at the second round:

- `ess specify validate --path ess` exits 0 and prints `sdc v1 — 4 file(s), 13 scenario(s), valid`
  with no warning: the system holds `sdc.decision` and `sdc.daemon`.
- The entities `Run`, `Session`, `DecisionRequest`, `RunEvent` and `Daemon` exist; the commands
  `SubmitRun`, `CancelRun`, `AnswerDecision`, `OpenSession` and `CloseSession` exist with their
  refusals named; the runtime commands include `WithdrawDecision`.
- The ten authored scenarios of `ess/scenarios/daemon-*.yaml` compile with `ess verify conform
  author` and pass on the `interpreted` target, with the three of `sdc.decision`.
- `ess generate --kind openapi` and `--kind schema` of `ess` write the committed files under
  `ess/generated/` and `ess generate ... --check` exits 0 for both; `task check:ess-generated` runs
  both and is part of `task check`.
- `docs/daemon-api.md` states the API for a caller.
- `driver/tests/decision_request.rs` stays green.
- The change touches `ess/`, `Taskfile.yml`, `docs/` and the planning store: no handler is written.

Not met, filed apart: the synthesized suite is not clean (`story:daemon-synthesized-suite-has-no-refusal`).

## Scope

`ess/` only (system, domain, scenarios, one component, generated output) and this story.

## Open questions for the owner (UNMAPPED in the domain)

Closed by the owner's twelve decisions of 2026-11-22 and applied to the domain (no `UNMAPPED:` marker
is left in `ess/domains/daemon.yaml`):

- Client label, minted run id with an idempotency key, host references (inline intake, context under
  a context root, runbook by name), open event kinds and run outcomes, the confinement levels
  `local` and `linux`, run settings as a text map, the decision request as a mirror of the runtime's,
  and cancel withdrawing the Open requests (`Withdrawn`, `WithdrawDecision`, `DecisionWithdrawn`).
- Three event streams paged by `position` (`EventsAfter`, `EventsOfSessionAfter`,
  `EventsOfDaemonAfter`).

Still open, outside the language:

- SSE itself: the views are the page a stream serves; the `text/event-stream` binding is written and
  tested in the story `daemon-skeleton-on-a-runtime-trait`, as are the host checks of a submit (runbook
  known, context under a root).
- The synthesized suite has refusals: the story `daemon-synthesized-suite-has-no-refusal`.
- Two defects of the interpreted target of ESS 0.96.0 for the ESS session: the ordered comparison
  `floor > input.level` over an ordered enum accepts every level (the floor guard spells the levels
  out), and a predicate guard through one `via` input is not evaluated when a later branch names a
  different `via` input (the guards of `SubmitRun` and `AnswerDecision` are built to avoid it).

## Acceptance note

- Accepted on creation from the operator requirement of 2026-11-22 (correlation `operator-requirement-2026-11-22-one-binary`), by the standing rule of the repository `AGENTS.md`.

## Decisions 16 to 19

Part C3a of the daemon (branch `impl/daemon-spec-16-19`, from `627cdaf`). Decisions 16 to 19 of the
owner are in `ess/domains/daemon.yaml`, in the handlers of `runbook/src/` and in `docs/daemon-api.md`.

- Decision 16, guard order of `AnswerDecision`. The branches are now declared in the order they
  answer: `no-request`, `not-of-run`, `withdrawn`, `already-answered`, `no-run`, `run-not-waiting`,
  `answered`. `already-answered` is still the `wrong_state` branch, declared above the related-row
  guards. Finding: on the interpreted target the specification already answered by the request's state
  first (from `ess/27` a held-state refusal precedes a `when_related` branch whatever the order in the
  file), so the scenario of the new behaviour passed before the change. The defect was in the C1 handler,
  which followed the declaration order and judged the run first; the handler now judges the request
  first. The declaration order was misleading and is corrected; scenario
  `daemon-an-answer-is-judged-by-its-request-before-its-run`. `RunNotWaiting` is left for an Open
  request whose run does not wait (a run cancelled while its request is not yet withdrawn).
- Decision 17, runs that were live when the daemon stopped. The runtime trait has
  `recover(run_id) -> Recovery { Resumed, Gone }`, asked at start for each run held as `Running` or
  `WaitingOnDecision`; the stub answers `Gone`. The new command `EndOrphanedRun` (actor `DaemonProcess`,
  not `Client`, so the generator gives it no route) moves the run by the new transition `orphan`
  (`Running`, `WaitingOnDecision` to `Ended`) with `outcome: orphaned`, and publishes `RunOrphaned`. The
  daemon then issues `WithdrawDecision` for each Open request. `Ended` carries `outcome`, open text that
  already holds the daemon's own `runtime-failure`, so no member was added; the outcome summary now names
  the two causes. The run's stream gets one event of kind `runbook.run-orphaned` (C1's `runbook.orphaned`
  is gone). A run that has ended is not asked about again, so a second start adds nothing. A `Resumed`
  run is left as it is; how it reports again is the engine story. Scenario
  `daemon-a-run-whose-runtime-is-gone-ends-as-orphaned`.
- Decision 19, a repeated submit. The language lets an error carry a value from a related row
  (`{related: {entity, where, field}}`), but `already-submitted` is not a refusal in this specification:
  it is a 202 outcome, so a retry after a lost answer gets the answer it lost. The event it publishes,
  `SubmitReplayed`, now carries `run_id`, the first run, read from the run that holds the key, and the
  body shows it (`published[0].payload.run_id`). The `Location` header of C1 stays. Scenario
  `daemon-idempotent-submit-returns-the-first-run` asserts it.
- Decision 18, no code. `docs/daemon-api.md` says 1,048,576 bytes of UTF-8 and that the character count of
  the specification is the weaker bound; the `IntakeTooLarge` summary says so too.
