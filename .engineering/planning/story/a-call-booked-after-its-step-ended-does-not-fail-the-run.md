---
format: aep.planning-md/3
id: story:a-call-booked-after-its-step-ended-does-not-fail-the-run
kind: story
status: active
title: A call booked after its step ended does not fail the run, and an abandoned call ends at the gateway
summary: A late failed call is named under its own step and never fails a run whose steps went on; a caller that hangs up ends its call upstream and is booked as abandoned
relations:
- informed_by: story:a-recovered-call-does-not-fail-the-run
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T11:10:21Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-27T11:10:21Z", actor: "agent:runbook", revision: 3}
---
## Outcome

A model call that the relay books after its step ended never fails the run by itself: a step that went on keeps the
run on its protocol, and the run's record names the late call under its own step. A caller that hangs up on its call
(its own timeout killed it) also ends the call at the gateway: the relay closes its request upstream, so the managed
route stops generating for nobody, and books the call as abandoned with what is known.

## Context

A consumer's refresh rounds on a managed route, reviewed on 2026-11-27 and relayed by the host's operator session,
which ordered three stories filed and built in rank order; this is the first. It is a regression of story
`a-recovered-call-does-not-fail-the-run`: that story's rule ("a step whose program exits 0 with its products goes on",
`docs/engine.md`, "A gateway") holds for a failed call booked while its step runs, not for one booked after its step
ended. The second half answers the rule of the same section that a caller who hangs up does not stop a call that was
sent.

## Evidence

- A report run: its digest step exited 0 with its digest written. Then four model-call lines were recorded at once
  (two of an earlier step whose client had timed out at 480 s, one broken and one answered of the digest step), then
  `runbook.model-calls-failed` ("the step went on and its program did not fail"), and 15 ms later
  `runbook.runtime-failed` and the end `runtime-failure`.
- Two investigate runs of the same round: a call in flight at the end of its step was booked failed and the run ended
  `runtime-failure`, while a third run with the same kind of failed call ended with its protocol's outcome.
- 17 of the round's 20 gateway calls finished on the managed route after their client's 270 s or 480 s timeout
  (gateway durations up to 665 s): each kept a sequence of the model busy for a caller that was gone and slowed the
  next run.
- In the review's count this change prevents 4 of 15 hard failures (3 by the order of the records, 1 inferred).

## Acceptance

- A run whose step leaves a call in flight and goes on (its program exits 3, an exit its bindings take as a result),
  and whose next step, in a child case, exits 0 with its product while the gateway answers the earlier call 502
  (`upstream-failed`), ends with its protocol's outcome. Its record holds one `runbook.model-calls-failed` that names
  the call under its own step, and no `runbook.runtime-failed`.
- A failed call booked while its own step runs settles as before: the cases of `a-recovered-call-does-not-fail-the-run`
  and `failed-calls-of-a-bound-program-belong-to-its-step` stay green.
- A caller that closes its connection to the relay before the answer has ended (before the gateway's head, and while
  the answer streams): the relay closes its request to the gateway within 1 s (a stand-in gateway records the close)
  and books the call once with the cause `call-abandoned` and the code `caller-hung-up`, the tokens the answer reported
  so far, and, when its cost is not known, the call's reservation with `cost_known: false` (the rule of an answer that
  broke off). An abandoned call is no failed call of its step.
- The tests make no model call and use no network beyond loopback.

## Scope

- `runbook/src/engine/gate.rs`: the booking keeps a failed call whose step is no longer in flight apart from the
  step's own, and names it when the run's calls are recorded (cited).
- `runbook/src/relay.rs`: the exchange and the answer's stream watch the caller; the abandoned call's line and its
  charge (cited).
- `docs/engine.md` ("A gateway"), `docs/model-routes.md` section 4, `docs/daemon-api.md` (the relay), `CHANGELOG.md`
  (inferred).
