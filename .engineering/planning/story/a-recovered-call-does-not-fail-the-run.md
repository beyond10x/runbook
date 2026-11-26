---
format: aep.planning-md/3
id: story:a-recovered-call-does-not-fail-the-run
kind: story
status: active
title: A recovered call does not fail the run
summary: A failed call the client recovers from is booked and noted; the run ends as its step ended
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-26T06:29:58Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-26T06:29:58Z", actor: "agent:runbook", revision: 3}
---
## Outcome

A model call that failed (a 4xx answer the client may recover from, a 5xx answer, a gateway timeout, a connection that
fails) no longer ends the run by itself. The call is booked and passed to the client as before; the run ends as its
step ended. The gateway's own refusals of the route, its spend ceiling and the run's caps still stop the run at once.

## Found

- Two demo delegations on a hosted route (2026-11-26): the gateway answered 400 (`invalid_request_error`) to two side
  calls of Claude Code. Claude Code went on, its next two calls were answered, it exited 0 and wrote `result.md`, yet
  each run ended `runtime-failure` "route-unavailable ... the gateway answered 400".
- A delegation on a local route (2026-11-26): one call ended at the gateway as a timeout, `502 upstream-failed` after
  about 122 s. Claude Code retried, later calls were answered, the step exited 0 with a correct `result.md`, and the
  run ended `runtime-failure` "route-unavailable ... upstream-failed: the gateway answered 502".

## Order

The operator, 2026-11-26 (relayed by the host operator): "all runs I observe fail"; make the fixes for the top
failure causes of the last 24 hours the head of the next wave, in one pull request. That order is this story's
acceptance (standing rule of 2026-11-21: a story made from the operator's requirement is accepted on creation). The
coordinator widened the rule the same day from 4xx answers to every failed call, on the second run above.

## Acceptance

- A free delegation whose gateway answers 400 to the first call and 200 after, whose agent exits 0 with `result.md`,
  ends `delivered` with its result text.
- A free delegation whose gateway answers 502 (`upstream-failed`) to the first call and 200 after, whose agent retries
  and exits 0 with `result.md`, ends `delivered` with its result text.
- A refused 4xx call's ledger line names the cause `call-refused`; a 5xx, a timeout and a connection that fails name
  `route-unavailable`; each line carries the gateway's code and, when the answer gave one, the gateway's error text
  (type and message, at most 300 bytes, no header, no token).
- The run's record holds the event `runbook.model-calls-failed`, naming the step and each failed call with its cause
  and code.
- A failed call in a step whose program exits non-zero ends the run with that call as its first cause.
- The gateway's refusals of the route (`model-unknown`, `wire-not-served`, `credential-*`), its `spend-ceiling` and the
  run's caps stop the run as before.

## Scope

- `runbook/src/relay.rs`: which failed calls stop the run, the line's cause and error text (cited).
- `runbook/src/engine/gate.rs`: the booking keeps failed calls apart from the calls that stop the run (cited).
- `runbook/src/engine/mod.rs`: the run's end follows the step; the note in the run's record (cited).
- `docs/model-routes.md` sections 4 and 7, `docs/engine.md`, `docs/daemon-api.md`, the delegation skill,
  `plugin/README.md`, `CHANGELOG.md` (cited). `ess/domains/daemon.yaml` lists no run event kinds: no change (checked).
