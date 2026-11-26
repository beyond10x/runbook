---
format: aep.planning-md/3
id: story:failed-calls-of-a-bound-program-belong-to-its-step
kind: story
status: active
title: Failed calls of a bound program belong to its step
summary: A call the relay books for a bound program carries the step's action, so the failing step names its own failed call
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-26T10:07:46Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-26T10:07:46Z", actor: "agent:runbook", revision: 3}
---
## Outcome

A model call the relay books for a bound program carries the step's action, the name the step's failure and
`runbook.action-started` use. A step whose program exits non-zero ends the run with its own first failed call as the
run's cause, and the run's record lists the calls of a step under its action, never a case id.

## Found

- The consumer's acceptance on `992e3c30` (2026-11-26, 4 red checks): a run of a runbook whose one step `strict.call`
  runs a bound program that calls the model through the relay ended `failed: step strict.call exit 1`, and its event
  `runbook.model-calls-failed` said "2 model call(s) of the step <the run's id> failed at the gateway; the step went on
  and its program did not fail".
- Cause: the step in flight the relay books a call under is the action `engine::announce` finds for the started
  program, else the program's case. The action is found from the argv of the bindings files of the run's copy, and
  the daemon's home held a second runbook binding the same program with the same arguments (`lib.call`). Two actions
  matched, none was named, and the case id took its place.

## Order

The operator's delegation of 2026-11-26 (relayed by the host operator): the next wave takes the top red cause of the
consumer's run table. Accepted on creation (standing rule of 2026-11-21).

## Acceptance

- A runbook whose step runs a bound program that another runbook of the home binds with the same argv, whose one call
  the stand-in gateway answers 502 and whose program then exits 1, books the call under the step's action and fails
  the run with that call as its first cause (`route-unavailable: route default (chosen by run) failed the call:
  upstream-failed ...`), with no `runbook.model-calls-failed` event.
- The same runbook whose program exits 0 ends `delivered`, and its `runbook.model-calls-failed` event names the step's
  action and lists the call under it.
- `runbook.action-started` of that step names the action.

## Scope

- `runbook/src/engine/announce.rs`: the action of a started program is taken from the bindings of the protocol of
  its case (the `CaseOpened` line of the case's run log in the store) when more than one bindings file binds its argv
  (cited).
- `runbook/src/engine/mod.rs`, `runbook/src/loom_run/run_loop.rs`, `runbook/src/loom_run/in_flight.rs`: the wrapper is
  given the run's store (cited).
- `docs/engine.md` or `docs/loom-run-directory.md` where the action of a started program is described, `CHANGELOG.md`
  (inferred).
