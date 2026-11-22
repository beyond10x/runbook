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
