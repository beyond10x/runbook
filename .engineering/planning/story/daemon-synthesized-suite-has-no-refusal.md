---
format: aep.planning-md/3
id: story:daemon-synthesized-suite-has-no-refusal
kind: story
status: proposed
title: The synthesized suite of sdc.daemon has no refusal
summary: ess verify conform synthesize reports 0 refusals for the domain sdc.daemon, run in the slow lane, its output committed with a digest of the sources.
scope:
- confidence: cited
  path: ess
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T03:16:36Z", actor: "human:operator", revision: 3, correlation: "operator-requirement-2026-11-22-one-binary"}
---
# Story: the synthesized suite of sdc.daemon has no refusal

## Outcome

`ess verify conform synthesize` reports 0 refusals for the domain `sdc.daemon`, run in the `slow`
lane, and its output is committed with a digest of the sources it was made from. Today the
authored scenarios (`ess/scenarios/daemon-*.yaml`) cover the domain on the interpreted target and
the synthesized suite does not.

## Source

The synthesis of `ess` at commit `5250bcf` of `spec/daemon-api` (ESS 0.96.0, interpreted target) made 123
scenarios and refused 85 outcomes, against 78 scenarios and 2 refusals at the base. By code, as that
run reported them (the specification has changed since; the synthesis is to be rerun):

| Code | Count | Cause |
|---|---|---|
| `ESS-SYNTH-001` | 34 | no witness: a guard that is a row set (`RecordRunEvent` position and sequence order, `ResumeRun` still-open, `AnswerDecision` through its request), so the rows it counts are not only the scenario's own |
| `ESS-SYNTH-003` | 5 | no candidate row satisfies the guards before the branch (`RecordRunEvent` run-closed, recorded, not-ascending; the floor guard) |
| `ESS-SYNTH-004` | 1 | an entity state (`DecisionRequest` Answered refuses `AnswerDecision`) whose route runs through an outcome no input reaches |
| `ESS-SYNTH-005` | 30 | a scenario that skips its read of a parameterised view (`RunsInState`, `RunsOfSessionInState`, `RunsOfSession`, `EventsAfter`) |
| `ESS-SYNTH-011` | 1 | the `RunEvent` invariant has no scenario after `RecordRunEvent/recorded` |
| `ESS-SYNTH-014` | 13 | an outcome or transition of the run lifecycle with no scenario, behind a read the synthesis skipped |
| `ESS-SYNTH-025` | 1 | the same invariant, a second accounting |

The synthesis took 744 s wall time at that commit and 852 s at the base, both far over
the test budget: it runs in the `slow` lane, never in the test step.

## Acceptance

- `ess verify conform synthesize --path ess` reports 0 refusals for `sdc.daemon`.
- The synthesized suite is written by the `slow` lane when the sources changed, committed, and a
  fast check compares the digest of its sources and fails on a stale file.
- The refusals are removed by changing the specification toward forms the synthesis can witness
  (guards by `via` and `predicate` instead of row sets, views the synthesis can read), or by a
  recorded exception per refusal that names why the language cannot witness it.
- The authored scenarios keep passing on the interpreted target.

## Scope

`ess/` only.

## Part C3b: the suite run against the daemon (2026-11-22)

Facts, measured with ess 0.97.1 on the host.

Synthesis:

- Sources of `627cdaf`: `ess verify conform synthesize --path ess --scenarios ess
  --target ir --out suite.json` run with a named limit of 1800 s, ran 1036 s in the pool (wall 1048 s) and made 111 scenarios (81 of
  `sdc.decision`, 30 of `sdc.daemon`) and 79 refusals: `ESS-SYNTH-004` 56, `ESS-SYNTH-001` 15,
  `ESS-SYNTH-003` 2, `ESS-SYNTH-005` 2, `ESS-SYNTH-014` 2, `ESS-SYNTH-013` 1, `ESS-SYNTH-025` 1. The
  `ESS-SYNTH-004` ones are outcomes whose route runs through a runtime-side command no input reaches
  (`AnswerDecision`, `CancelRun/cancelled`, `EndRun/ended`, `FailRunStart/failed` and the like).
- Sources of `86c9acf8` (the merge of decisions 16 to 19: 2 scenarios, the command `EndOrphanedRun`,
  the event `RunOrphaned`, one field): the same command was killed at its named 1800 s (exit 124), after
  850 s in the queue. A second run with a named limit of 3600 s was cancelled before it started, on the
  word of the ess session: the time grows about N^3.2 to N^3.7 with the commands that move one entity,
  the outcomes per command, the row width and the actor attributes, no option bounds it and
  `--component` does not cut it; the fix is ess 0.97.2. Alone, the domain `sdc.decision` was not
  finished by a 110 s probe.
- So no synthesized suite of the current sources is committed. The open step, with its command, when
  ess 0.97.2 is published: `task ess-suite:record` in the background (it names `slot --timeout 30m`),
  commit `ess/generated/conformance/suite.json` and `DIGEST`, wire `check:ess-suite:synthesized` into
  `check:ess-generated`, run the crate test against it (`CONFORMANCE_SUITE` for a trial), and put its
  numbers into `GATE` in `runbook/tests/daemon/conformance.rs`.

What is committed instead: the suite of the authored scenarios (`ess/generated/conformance/authored.json`,
written by `ess verify conform author` in 1 s, compared byte for byte by `task check:ess-suite`) and the
runner `runbook/tests/daemon/conformance.rs`. Against the daemon at `86c9acf8`: 12 scenarios of the
daemon, 10 executed, 4 passed, 6 failed, 2 skipped (3 more scenarios are of `sdc.decision`). The crate
test step: 144 tests passed in 31 s; the conformance test 1.4 s. The suite of `627cdaf` kept in
scratch, run against the same daemon: 30 scenarios of the daemon, 22 executed, 18 passed, 4 failed,
8 skipped.

The six failures are two differences between the specification and the daemon, found by the runner and
named in `KNOWN_FAILURES`:

- The daemon writes its own run-log event `runbook.submitted` at sequence 1 of a run; the specification's
  `SubmitRun` writes no `RunEvent` row, so the first `RecordRunEvent` of a scenario (sequence 1) is
  refused as `not-ascending`: `daemon-submit-and-run-to-an-outcome`,
  `daemon-two-runs-in-a-session-keep-their-events-apart`, `daemon-events-after-a-sequence-number`.
- `CancelRun` and `EndOrphanedRun` withdraw the run's open decision requests in the same step; the
  specification leaves them Open until a separate `WithdrawDecision`, which the daemon then answers
  `not-open`: `daemon-cancel-a-run`, `daemon-a-run-whose-runtime-is-gone-ends-as-orphaned`, and, for the
  answer after a cancel (`withdrawn`, where the specification says `run-not-waiting`),
  `daemon-an-answer-is-judged-by-its-request-before-its-run`.

The four earlier differences are fixed in the merged daemon: a second answer is `DecisionAlreadyAnswered`
(scenario `daemon-answer-to-an-answered-request-is-refused` runs fully and is skipped only for an
unread instant); a run live at restart ends as `orphaned` through `EndOrphanedRun` (the scenario now
runs and fails only on the withdrawal above); a repeated submit answers with the first run
(`daemon-idempotent-submit-returns-the-first-run` passes); the intake limit stays bytes in the daemon.

Refusals. The domain's command table holds 40 refusals. The authored scenarios assert 21 of them.
Passed against the daemon in the committed suite (5): `SubmitRun` `context-outside-roots`, `no-runbook`
and `below-floor`; `RequestDecision` `run-not-waiting`; `ResumeRun` `still-open`. Answered correctly in
scenarios skipped only for an unread clock instant (6): `AnswerDecision` `already-answered` and
`not-of-run`; `CloseSession` `wrong-state`; `OpenSession` `session-exists`; `SubmitRun` `no-session` and
`session-closed`. Passed in the synthesized suite of `627cdaf` (scratch, 13): the `no-run` of `CancelRun`,
`EndRun`, `FailRunStart`, `RequestDecision`, `ResumeRun`, `StartRun`, `WaitOnDecision`; `CloseSession`
`no-session`; `OpenSession` `session-exists`; `StartDaemon` `already-started`; `SubmitRun` `no-session`
and `session-closed`; `WithdrawDecision` `no-request`. Together 18 of 40 passed against the daemon.

Left, the 22 of 40: the refusals no suite exercises (`wrong-state` of `StartRun`,
`FailRunStart`, `WaitOnDecision`, `ResumeRun`, `EndRun`; `no-daemon`, `intake-too-large` of `SubmitRun`;
`already-ended` of `CancelRun`, and the like) and the ones behind the two differences above. The
synthesis of ess 0.97.2 is to say which of them it witnesses.

What a green run cannot read: the grants (`CloseSession` and `SubmitRun` `grant/denied`: the daemon
authenticates no actor, so the scenarios are skipped) and the exact instants (`submitted_at`,
`closed_at` and the like: the daemon stamps its own clock and has no seam for a test; a clock hook in
`runbook/src` would let those claims be read, and one scenario under `ess/scenarios/` could not close
this, since the claim is about the daemon's clock, not about its rules).

Mutations that turned the committed suite red, none committed: a handler answering `no-runbook` for
`context-outside-roots` turned `daemon-context-outside-the-roots-is-refused` red; `runs()` dropping its
first row turned `daemon-decision-is-answered-and-the-run-resumes`,
`daemon-idempotent-submit-returns-the-first-run` and `daemon-submit-below-the-floor-is-refused` red;
`answer_decision` skipping its state change turned `daemon-decision-is-answered-and-the-run-resumes` and
`daemon-answer-to-an-answered-request-is-refused` red.
