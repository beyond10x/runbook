---
format: aep.planning-md/3
id: story:sdc-plan-gate-not-passed-has-no-outcome
kind: story
status: proposed
title: 'sdc.plan: a gate record that is not passed leaves the plan case with no action and no outcome'
summary: 'canon simulate stuck-state, verdict real: a failed ESS gate in sdc.plan has no way on but the epic''s abandoned decision'
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
  path: protocol/compiled/sdc-plan.ir.json
- confidence: inferred
  path: protocol/sdc-plan.protocol.yaml
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T10:50:31Z", actor: "human:operator", revision: 2, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: sdc-plan-gate-not-passed-has-no-outcome

## Outcome

`canon simulate --path protocol/sdc-plan.protocol.yaml` no longer prints the stuck state in which
the roadmap is approved and the ESS gate record about the current specification is not `passed`.
In that state no action of `sdc.plan` is admissible, `passed` is its only outcome, and the parent
epic waits on the child until a person takes the epic's `abandoned` decision. After the change
the plan case ends without a person, and the epic ends `not_delivered`.

## Context

Canon's unreleased `canon simulate` (Canon commit `a1a937ec`, branch `wave/2026-11-20`) was run on
SDC `main` at `dfe4a3f`. It printed 17 findings for `sdc.plan`, of which this one is `real`
(`docs/simulate-findings.md`, stuck state 5). Printed summary and finding:

```
simulated: protocol `sdc.plan` revision 7: 85 reachable states, 1 terminable (outcome `passed` 1), 17 findings (17 stuck or cycle states)
```

```
stuck-state: state {evidence `ess_gate` about `spec`, evidence `roadmap_approval` about `roadmap` result `approved`}: no outcome is legitimate and no action is admissible or approval-required; path: action `roadmap.read` produces evidence `roadmap_approval` about `roadmap` result `approved`, then action `spec.gate` produces evidence `ess_gate` about `spec`
```

Why it is real:

- `spec.gate` runs once per specification revision (`protocol/sdc-plan.protocol.yaml:147-157`).
  Every other action needs `gate.passed` (`:158-221`), and `amend.upstream` needs
  `plan.covers_spec` `false` (`:222-233`). `passed` is the only outcome (`:235-246`). Nothing in
  the case revises the specification: the spec case has ended.
- A real run reaches the state when the gate program fails on a specification its spec case
  passed. The gate runs the same four commands as `sdc.spec`'s checks
  (`driver/examples/sdc/tools/sdc-tool.sh:143-149` against `:150-155`), and every non-zero exit
  except 126, 127 and 75 records `failed` (`driver/examples/sdc/sdc-plan.bindings.json:25`,
  `driver/src/main.rs:549-554`). So a crash, a timeout, or a newer `ess` between the spec case and
  the plan case gets there.
- No trial-3 run has hit it: all 13 `sdc.plan` gate records in `runs/trial-3-*` are `passed`.

The same cause prints in `sdc.story` and `sdc.release` (stuck states 8 and 6, `left only by a
decision`: `dropped`, `withheld`). In `sdc.epic` it is in `story:sdc-epic-record-without-unlocking-result`.

```
stuck-state: state {evidence `ess_gate` about `spec`}: no outcome is legitimate and no action is admissible or approval-required; path: action `spec.gate` produces evidence `ess_gate` about `spec`; decision `explicitly_dropped` taken makes outcome `dropped` legitimate; 2 other stuck states contain it
stuck-state: state {evidence `ess_gate` about `spec`}: no outcome is legitimate and no action is admissible or approval-required; path: action `spec.gate` produces evidence `ess_gate` about `spec`; decision `explicitly_withheld` taken makes outcome `withheld` legitimate; 2 other stuck states contain it
```

Candidate, probed with `canon simulate` on a scratch copy of the protocol, not with `canon check`:
an outcome of `sdc.plan` that requires `gate.passed` `false`. The stuck state no longer printed,
16 findings remained, and 2 states were terminable. The epic's `children` form lists only
`passed` (`protocol/sdc-epic.protocol.yaml:58-62`), so the plan's new outcome makes the epic
`not_delivered` (`:209-219`). The same outcome in `sdc.story` and `sdc.release` would end those
cases on a predicate instead of a decision.

## Acceptance

- Once Canon releases `canon simulate`, `canon simulate --path protocol/sdc-plan.protocol.yaml`
  does not print the stuck state `{ess_gate about spec, roadmap_approval result approved}` or any
  state that contains it.
- `scripts/check-protocols.sh` passes, and `canon check` of `sdc.plan` names its state count: 65536
  states on revision 7 (`docs/check-output.txt:8`). An outcome that reads existing claims is
  expected to keep that count (*inferred*); a different count is stated and stays within the bound.
- A case under `cases/` (plan with the gate `failed`: the new outcome legitimate; epic with that
  child outcome: `not_delivered` legitimate) is green in `scripts/evaluate-cases.sh`.

## Scope

- `protocol/sdc-plan.protocol.yaml` and `protocol/compiled/sdc-plan.ir.json`; optionally
  `protocol/sdc-story.protocol.yaml` and `protocol/sdc-release.protocol.yaml` for the same outcome
- a new fixture case under `cases/`
- `docs/protocol-notes.md` ("Every child outcome has a way on in the parent"),
  `docs/simulate-findings.md`

## Ambiguities

- Whether a failed gate in a later stage should end the case at all, or whether the gate program
  should instead record no result on its own failure (a binding change: it removes the real path,
  but the walk still prints the state). The operator chooses.
