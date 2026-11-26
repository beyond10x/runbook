---
format: aep.planning-md/3
id: story:a-recovered-call-does-not-fail-the-run
kind: story
status: active
title: A recovered call does not fail the run
summary: A 4xx the client recovers from is booked and noted; the run ends as its step ended
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-26T06:29:58Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-26T06:29:58Z", actor: "agent:runbook", revision: 3}
---
## Outcome

A model call the gateway refuses with a 4xx answer the client may recover from no longer fails a run whose step went on
and delivered its product. The call is booked and passed to the client as before; the run ends as its step ended.

## Found

Two demo delegations on a hosted route (2026-11-26): the gateway answered 400 (`invalid_request_error`) to two side
calls of Claude Code. Claude Code went on, its next two calls were answered, it exited 0 and wrote `result.md`, yet
each run ended `runtime-failure` "route-unavailable ... the gateway answered 400".

## Order

The operator, 2026-11-26 (relayed by the host operator): "all runs I observe fail"; make the fixes for the top
failure causes of the last 24 hours the head of the next wave, in one pull request. That order is this story's
acceptance (standing rule of 2026-11-21: a story made from the operator's requirement is accepted on creation).

## Acceptance

- A free delegation whose gateway answers 400 to the first call and 200 after, whose agent exits 0 with `result.md`,
  ends `delivered` with its result text.
- That refused call's ledger line names the cause `call-refused`, the gateway's code and the gateway's error text
  (type and message, at most 300 bytes, no header, no token).
- The run's record holds the event `runbook.model-calls-refused`, naming the step and each refused call.
- The same refusal in a step whose program exits non-zero ends the run with the refused call as its first cause.
- A refusal of the route by the gateway (`model-unknown`, `wire-not-served`, `credential-*`, `spend-ceiling`), a 5xx
  answer, a gateway that cannot be reached and the run's caps stop the run as before.

## Scope

- `runbook/src/relay.rs`: which refusal a client may recover from, the line's cause and error text (cited).
- `runbook/src/engine/gate.rs`: the booking keeps such calls apart from the calls that fail a step (cited).
- `runbook/src/engine/mod.rs`: the run's end follows the step; the note in the run's record (cited).
- `docs/model-routes.md` sections 4 and 7, `CHANGELOG.md`, `ess/domains/daemon.yaml` if it lists run event kinds
  (inferred).
