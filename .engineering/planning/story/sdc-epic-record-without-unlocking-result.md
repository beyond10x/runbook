---
format: aep.planning-md/3
id: story:sdc-epic-record-without-unlocking-result
kind: story
status: proposed
title: 'sdc.epic: the epic case is stuck on a roadmap copy that is not approved or a spec child passed with the ESS gate not passed'
summary: 'canon simulate findings: no outcome and no admissible action after a roadmap copy or a gate record without the unlocking result'
tags:
- canon
- from-simulate
relations:
- decomposes: epic:kernel-and-runtime-uptake
- informed_by: story:simulate-as-a-protocol-gate
- depends_on: story:protocols-declare-revises-and-authors
scope:
- confidence: inferred
  path: cases/13-epic-roadmap-copy-not-approved
- confidence: inferred
  path: cases/14-epic-gate-not-passed
- confidence: inferred
  path: docs/protocol-notes.md
- confidence: inferred
  path: protocol/compiled/sdc-epic.ir.json
- confidence: inferred
  path: protocol/sdc-epic.protocol.yaml
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T09:20:29Z", actor: "human:timo", revision: 3, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: sdc-epic-record-without-unlocking-result

## Outcome

`canon simulate --path protocol/sdc-epic.protocol.yaml` no longer prints two stuck states of the
epic case: the roadmap approval copy whose result is not `approved` (nothing opens, no outcome), and
the specification case ended `passed` while the ESS gate record about the current specification is
not `passed` (the plan cannot open, no outcome). After the change the protocol text gives the epic a
way on in each state that a real case can take, or the walk finds the state open.

## Context

Canon's unreleased `canon simulate` (branch `impl/simulate-child-outcome-step` at `70eda28` in the
sibling Canon checkout) printed 5 findings (25 stuck or cycle states) for `sdc.epic`. Canon
simulated an older state of the SDC tree, commit `f364b6b` (`sdc.epic` revision 3, as read for
"request 16"), not the current head (`sdc.epic` revision 5). Canon's second adversary pass on that
unit was still running and may change a figure. Printed summary:

```
simulated: protocol `sdc.epic` revision 3: 122 reachable states, 69 terminable (outcome `abandoned` 61, outcome `passed` 16), 13 waiting on supplied evidence, a child outcome or expiry, 5 findings (25 stuck or cycle states)
```

Three of the five findings are removed by `ebe8694` and `25e0709` (the outcomes `not_delivered` and
`not_released`, `protocol/sdc-epic.protocol.yaml:209-230`); the list is in
`story:simulate-as-a-protocol-gate`. Two stand on the current head, with one cause: one record of a
kind guarded to be written once per revision has a result other than the one that unlocks the case,
and no outcome reads it. Printed text of both:

```
stuck-state: state {evidence `roadmap_approval` about `roadmap`}: no outcome is legitimate and no action is admissible or approval-required; path: action `roadmap.read` produces evidence `roadmap_approval` about `roadmap`; decision `explicitly_abandoned` taken makes outcome `abandoned` legitimate
```

```
stuck-state: state {evidence `case_outcome` result `passed`, evidence `ess_gate` about `spec`, evidence `roadmap_approval` about `roadmap` result `approved`, evidence `spec_opened` about `epic`}: no outcome is legitimate and no action is admissible or approval-required; path: action `roadmap.read` produces evidence `roadmap_approval` about `roadmap` result `approved`, then action `spec.gate` produces evidence `ess_gate` about `spec`, then action `spec.open` produces evidence `spec_opened` about `epic`, then a child of action `spec.open` records evidence `case_outcome` result `passed`; decision `explicitly_abandoned` taken makes outcome `abandoned` legitimate
```

Why they stand on `main`:

- Roadmap copy. `roadmap.read` (`protocol/sdc-epic.protocol.yaml:103-111`) is admissible while
  `roadmap.copied` is unknown (`:107-109`) and its record may carry any result. Without
  `roadmap.approved` (`:32-38`) `spec.open` (`:112-124`, line `:117`) and `spec.gate` (`:125-135`,
  line `:131`) are blocked, and the action text says a copy of another result leaves
  `roadmap.approved` "unknown for good" (`:105`). No outcome reads it: `passed` needs
  `roadmap.approved` (`:196-204`), `not_delivered` and `not_released` need `children.passed` false
  (`:209-230`, no child exists), `abandoned` needs the decision (`:205-208`).
- Gate record. `spec.gate` runs only while `gate.checked` is unknown (`:132-133`, text at `:127`),
  so a record whose result is not `passed` stays for the revision. `plan.open` needs `gate.passed`
  (`:136-151`, line `:142`). With the specification child `passed` (`children.passed` true), no
  action opens and no outcome holds: `passed` needs `release.opened` (`:203`), `not_delivered` needs
  `children.passed` false (`:216-217`).

In a real case a revised roadmap or a revised specification sets the old record aside, and the
specification case ends `passed` only when its own gate passes
(`protocol/sdc-spec.protocol.yaml:91-98` `spec.complete`, read by `spec.done` at `:153-160`, which outcome `passed` requires at `:326-331`). The walk revises no artifact, so it reaches both
states; `docs/protocol-notes.md:241-242` classes them "a failed check only another case revises
(a2): no change". This story asks for a protocol text under which the walk finds nothing, without
removing that revision path.

`docs/protocol-notes.md:241-242` classes this finding as a failed check only another case revises, which a real case leaves by revising the artifact and the walk does not model; Canon's revise step for a declared `revises` is `story:protocols-declare-revises-and-authors`.

Candidate, *inferred* and not run: one outcome legitimate when the record is present and the claim
is unknown (`roadmap.copied` true with `roadmap.approved` unknown; `gate.checked` true with
`gate.passed` unknown). Check each with `canon evaluate` on a case before choosing; the evidence
claim's value for a record of another result is stated only in the action texts (`:105`, `:127`).

## Acceptance

- Once Canon releases `canon simulate`, `canon simulate --path protocol/sdc-epic.protocol.yaml` does
  not print the state `{roadmap_approval}` or the state `{case_outcome result passed, ess_gate,
  roadmap_approval result approved, spec_opened}`, nor any state that contains either.
- `scripts/check-protocols.sh` passes, and `canon check` of `sdc.epic` names its state count: 36864
  states on revision 5 (`docs/check-output.txt:4`); a change that adds an evidence kind states the
  new count, which stays under the 65,536 bound.
- Each new case under `cases/` is green in `scripts/evaluate-cases.sh`.
- Alternatively to the first item: or the finding no longer prints once the protocol declares `revises` for the artifact and Canon's simulate walks the revise step.

## Scope

- `protocol/sdc-epic.protocol.yaml` (and its compiled IR `protocol/compiled/sdc-epic.ir.json`)
- fixture cases: `cases/13-epic-roadmap-copy-not-approved/` (copy with another result: the new way on
  legitimate) and `cases/14-epic-gate-not-passed/` (spec child passed, gate record `failed`); red on
  revision 5, green after
- `docs/protocol-notes.md` ("What `canon simulate` still reports and why")

## Ambiguities

- Whether the epic ends on these states without a person or waits for a revised roadmap or
  specification; a protocol outcome ends the case in the driver (*inferred*, not checked), so the
  choice is the operator's. If an outcome is chosen it also ends the root `not_delivered`
  (`protocol/sdc.protocol.yaml:167-177`).

## Current figures (Canon a1a937ec on dfe4a3f)

Canon commit `a1a937ec` (branch `wave/2026-11-20`, unreleased) on SDC `main` at `dfe4a3f`,
`sdc.epic` revision 5 (`docs/simulate-findings.md`, stuck states 2 and 3):

```
simulated: protocol `sdc.epic` revision 5: 122 reachable states, 91 terminable (outcome `abandoned` 61, outcome `not_delivered` 18, outcome `not_released` 26, outcome `passed` 16), 15 waiting on supplied evidence, a child outcome or expiry, 2 findings (2 stuck or cycle states)
```

Both findings above still print with the same text. The verdicts change from "a2, left by a
revision another case writes":

- Roadmap copy `{roadmap_approval}`: `left by revision`, and a real run does not reach it. The epic
  opens only on the root's `roadmap.approved` (`protocol/sdc.protocol.yaml:123`), and the copy
  takes the latest record at the current revision (`driver/src/main.rs:474-492`). All 16 epic
  copies in `runs/trial-3-*` are `approved`.
- Gate record with the spec child `passed`: `left only by a decision` (`abandoned`). Once the spec
  child has ended, nothing revises the specification.
  - A real run reaches it when the gate program fails on the revision its spec case passed. It
    runs the same four commands (`driver/examples/sdc/tools/sdc-tool.sh:143-149` against
    `:150-155`), and every non-zero exit except 126, 127 and 75 records `failed`
    (`driver/examples/sdc/sdc-epic.bindings.json:22`). So a crash, a timeout, or a newer `ess`.
  - The 16 `failed` epic gate records in `runs/trial-3-*` are all at spec revision `none`, set
    aside by the spec child's first revision, which is not this state.
  - The same cause in `sdc.plan` is `story:sdc-plan-gate-not-passed-has-no-outcome`, whose
    candidate is an outcome on `gate.passed` `false`.
