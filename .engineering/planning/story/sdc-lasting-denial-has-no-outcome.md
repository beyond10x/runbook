---
format: aep.planning-md/3
id: story:sdc-lasting-denial-has-no-outcome
kind: story
status: proposed
title: 'sdc.direction and sdc.intake: a lasting capability denial has no outcome in the case'
summary: 'canon simulate cycles, verdict real: owner.approve or scope.answer denied leaves the child case unable to end'
tags:
- canon
- from-simulate
relations:
- decomposes: epic:kernel-and-runtime-uptake
- informed_by: story:simulate-as-a-protocol-gate
scope:
- confidence: inferred
  path: docs/protocol-notes.md
- confidence: inferred
  path: docs/simulate-findings.md
- confidence: inferred
  path: driver/src/main.rs
- confidence: inferred
  path: protocol/compiled/sdc-direction.ir.json
- confidence: inferred
  path: protocol/compiled/sdc-intake.ir.json
- confidence: inferred
  path: protocol/sdc-direction.protocol.yaml
- confidence: inferred
  path: protocol/sdc-intake.protocol.yaml
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T10:50:31Z", actor: "human:timo", revision: 2, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: sdc-lasting-denial-has-no-outcome

## Outcome

`canon simulate` no longer prints the cycles in which a capability denial lasts for the case and
the case has no outcome that the denial reaches:

- `sdc.direction` with `owner.approve` denied;
- `sdc.intake` with `scope.answer` denied while the interview is not settled.

In both, the parent waits on a child that cannot end. After the change each case ends without a
person, and its parent goes on through the outcomes it already reads.

## Context

Canon's unreleased `canon simulate` (Canon commit `a1a937ec`, branch `wave/2026-11-20`) was run on
SDC `main` at `dfe4a3f`. These four of its 71 findings are `real` (`docs/simulate-findings.md`,
groups D2 and I5). Printed text:

```
cycle: state {evidence `context_defaults` about `context` result `valid`, evidence `roadmap_validation` about `roadmap` result `valid`, capability `owner.approve` denied}: no outcome is legitimate, no action is approval-required, and every step of action `context.change` returns this state; path: action `context.check` produces evidence `context_defaults` about `context` result `valid`, then action `roadmap.validate` produces evidence `roadmap_validation` about `roadmap` result `valid`, then capability `owner.approve` denied
cycle: state {evidence `context_defaults` about `context` result `valid`, evidence `roadmap_validation` about `roadmap`, capability `owner.approve` denied}: no outcome is legitimate, no action is approval-required, and every step of action `context.change` returns this state; path: action `context.check` produces evidence `context_defaults` about `context` result `valid`, then action `roadmap.validate` produces evidence `roadmap_validation` about `roadmap`, then capability `owner.approve` denied
cycle: state {evidence `dissection_shape` about `dissection` result `valid`, evidence `idea_shape` about `idea` result `valid`, evidence `questions_settled` about `questions`, evidence `questions_shape` about `questions` result `valid`, evidence `requirements_shape` about `requirements` result `valid`, capability `scope.answer` denied}: no outcome is legitimate, no action is approval-required, and every step of action `questions.answer` returns this state; path: action `dissection.check` produces evidence `dissection_shape` about `dissection` result `valid`, then action `idea.check` produces evidence `idea_shape` about `idea` result `valid`, then action `questions.check` produces evidence `questions_settled` about `questions`, then action `questions.check` produces evidence `questions_shape` about `questions` result `valid`, then action `requirements.check` produces evidence `requirements_shape` about `requirements` result `valid`, then capability `scope.answer` denied; 2 other cycle states contain it
cycle: state {evidence `dissection_shape` about `dissection` result `valid`, evidence `idea_shape` about `idea` result `valid`, evidence `questions_settled` about `questions`, evidence `questions_shape` about `questions` result `valid`, evidence `requirements_shape` about `requirements`, capability `scope.answer` denied}: no outcome is legitimate, no action is approval-required, and every step of action `questions.answer` returns this state; path: action `dissection.check` produces evidence `dissection_shape` about `dissection` result `valid`, then action `idea.check` produces evidence `idea_shape` about `idea` result `valid`, then action `questions.check` produces evidence `questions_settled` about `questions`, then action `questions.check` produces evidence `questions_shape` about `questions` result `valid`, then action `requirements.check` produces evidence `requirements_shape` about `requirements`, then capability `scope.answer` denied; 2 other cycle states contain it
```

Summaries: `sdc.direction` revision 7, 41 reachable states, 1 terminable, 10 findings;
`sdc.intake` revision 9, 1215 reachable states, 408 terminable, 17 findings.

Why they are real:

- The driver refuses a capability decided twice, so a denial stands for the case
  (`docs/protocol-notes.md:243-245`).
- Direction: every approval needs `owner.approve`
  (`protocol/sdc-direction.protocol.yaml:204-217`, `:232-246`, `:272-288`). `context.change`
  (`:179-192`) is admissible but has no binding, so the driver never takes it
  (`driver/src/main.rs:648-650`), and the run stops `stuck` (`:1210-1211`). `passed` is the only
  outcome (`:301-310`). `docs/protocol-notes.md:246-250` leaves the way on to the root's
  `abandoned` decision.
- Intake: `questions.answer` leaves every scope-changing question open
  (`protocol/sdc-intake.protocol.yaml:170-181`, text at `:172`). Only `scope.answer` (`:182-199`)
  answers those questions, and it needs the denied capability, so `questions.settled` stays false.
  The loop is bounded only by the driver: `questions.answer` max_runs 3
  (`driver/examples/sdc/sdc-intake.bindings.json:100`).
- No trial-3 run stopped on either state.

Candidate, probed with `canon simulate` on a scratch copy of `sdc.direction`, not with
`canon check`. This is the pattern available without a kernel change (Canon ruling D152): a
supplied kind (`authority_withheld`, about `context`), and an outcome that requires it together
with `context.valid` and `context.decided` unknown. The two direction findings no longer printed;
the other 8 did. An outcome that required the supplied record alone made every state open (0
findings), which would hide every other finding, so the requirement must name the stuck state's
claims.

## Acceptance

- Once Canon releases `canon simulate`, `canon simulate` of `protocol/sdc-direction.protocol.yaml`
  and `protocol/sdc-intake.protocol.yaml` does not print the four states above or any state that
  contains them, and prints no fewer of the other findings than `docs/simulate-findings.md` lists
  for them.
- `scripts/check-protocols.sh` passes, and `canon check` names the state counts: 49152 for
  `sdc.direction` revision 7 and 49152 for `sdc.intake` revision 9 (`docs/check-output.txt:2`,
  `:6`). A new evidence kind multiplies them. The new counts are stated, and each slice stays
  within the bound of the pinned Canon release.
- A case under `cases/` per protocol (a denial with the supplied record: the new outcome
  legitimate) is green in `scripts/evaluate-cases.sh`, and the root reads the child's new
  outcome as `refused` or `not_delivered`.

## Scope

- `protocol/sdc-direction.protocol.yaml`, `protocol/sdc-intake.protocol.yaml` and their compiled IR
- the runtime that writes the supplied record on a denial (`driver/`)
- new fixture cases under `cases/`
- `docs/protocol-notes.md` ("A lasting denial"), `docs/simulate-findings.md`

## Ambiguities

- *Inferred*: the new kind does not fit the 65,536 bound of Canon 0.9.0, so this waits on the
  per-slice bound that the `a1a937ec` `check` help text describes.
- Who writes the supplied record: the driver when a capability is denied, or the decider.
