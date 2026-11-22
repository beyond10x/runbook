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

## Part C4: the specification moves to what the daemon does (2026-11-22)

The design owner decided that the specification follows the daemon in the two differences part C3b
found. Branch `impl/daemon-spec-matches-daemon` from `57101503`. ESS 0.97.1; the synthesized suite
was not run (known defect, fixed in 0.97.2).

Counts, before and after the change. Against the daemon (the crate's conformance test): 12 scenarios,
10 executed, 4 passed, 6 failed, 2 skipped; after: 10 executed, 10 passed, 0 failed, 2 skipped,
`KNOWN_FAILURES` empty, `GATE` raised to 10 passed. On the interpreted target
(`ess verify conform run --target interpreted`): 15 of 15 before and 15 of 15 after. The two skipped
scenarios (`daemon-answer-to-an-answered-request-is-refused`, `daemon-closed-session-takes-no-run`)
claim an exact instant of the daemon's own clock and stay skipped: no small truthful change reads them.

Difference 1, the daemon's own first event. `SubmitRun` states that the accepted outcome makes the
daemon write `runbook.submitted` as sequence 1 (position: the next of the daemon), so the runtime's
first record is sequence 2; a replayed submit (`already-submitted`) and every refused submit write no
event and create no run. Every event the daemon writes on its own is declared in the summary of the
command that writes it, and listed in `EventKind`: `runbook.submitted` (`SubmitRun`),
`runbook.cancel-requested` (`CancelRun`), `runbook.start-failed` (`FailRunStart`),
`runbook.run-orphaned` (`EndOrphanedRun`), `runbook.answer-undeliverable` (`AnswerDecision`) and
`runbook.runtime-failed` (the daemon's own `RecordRunEvent` just before `EndRun` with the outcome
`runtime-failure`). Three scenarios record the runtime's events from sequence 2 and from the positions
after the submits; their `at` assertions page after the submit event's position, so the interpreted
target (which keeps no row for the daemon's own events) and the daemon read the same first row.

Difference 2, withdrawal. `CancelRun/cancelled` and `EndOrphanedRun/orphaned` carry an `affects`
entry (`entity: sdc.daemon.DecisionRequest`, `where: run_id == input.run_id`, `moves:
sdc.daemon.DecisionRequest.withdraw`; ess/22 and later express it, only rows in `Open` move): the
requests are Withdrawn in the same step. `DaemonProcess` no longer holds `WithdrawDecision`.
`WithdrawDecision` stays for the runtime's own withdrawal of an Open request of a run that goes on
(an Open request exists only while its run is WaitingOnDecision); after a cancel or an orphan end it
answers `not-open`. An answer to such a request is `DecisionWithdrawn`. `AnswerDecision/run-not-waiting`
is now unreachable through the daemon's commands (the crate's own test makes it by hand from a journal)
and the summary says so; no authored scenario reaches it any more.

What the language could not express, and what stands in its place:

- A command writes one kind of row: `SubmitRun` cannot also create a `RunEvent`, and `CancelRun`,
  `FailRunStart`, `EndOrphanedRun` and `AnswerDecision` cannot either (an `affects` entry moves or
  updates rows, it does not create them). The daemon's own events are stated in the summaries and
  read through the event views only.
- Because of it the specification does not refuse a runtime record at sequence 1: the interpreted
  target accepts it, the daemon refuses it as not ascending. No scenario asks that step.
- The `affects` entry publishes no event per request: the answer of a cancel carries `RunCancelled`
  only. See the finding below.

Finding against the recorded decisions (the handler is unchanged). Decision 15 says a cancel issues
`WithdrawDecision` per open request and "the event is `DecisionRequestWithdrawn`". The daemon does
withdraw each request, but the answer of `cancel-run` publishes `RunCancelled` only: the events of the
inner withdrawals are dropped (`cancel_run`, `end_orphaned_run` in `runbook/src/commands.rs`). The
specification now says what the daemon does; if `DecisionRequestWithdrawn` is wanted per request, the
handler has to publish them and the owner has to say where (decision 15 and this brief disagree on
"one command per request").

Runner. `runbook/tests/daemon/conformance.rs` now reads a view expectation `at` (first or last row in
the order named) and writes a whole-number view parameter without `.0` (the suite spells `1.0`); both
had been hidden behind the six failures. Two assertions of an exact `ended_at` (cancel, orphaned) were
dropped from `daemon-cancel-a-run` and `daemon-a-run-whose-runtime-is-gone-ends-as-orphaned`: with them
those two scenarios would be skipped, not passed, and `ended_at: input.at` is no longer read by an
authored scenario for those two commands.

Red proof (not committed): `cancel_run` without its withdrawal loop turned `daemon-cancel-a-run`
(`"sdc.daemon.WithdrawDecision" answered withdrawn and not not-open`) and
`daemon-an-answer-is-judged-by-its-request-before-its-run` (`answered run-not-waiting and not
withdrawn`) red: 8 passed, 2 failed.
