---
format: aep.planning-md/3
id: story:sdc-root-roadmap-copy-not-approved
kind: story
status: proposed
title: 'sdc: the root case is stuck after intake and direction passed when its roadmap approval copy is not approved'
summary: 'canon simulate finding: no outcome and no admissible action once the root holds a roadmap copy without the result approved'
tags:
- canon
- from-simulate
relations:
- decomposes: epic:kernel-and-runtime-uptake
- informed_by: story:simulate-as-a-protocol-gate
- depends_on: story:protocols-declare-revises-and-authors
scope:
- confidence: inferred
  path: cases/12-root-roadmap-copy-not-approved
- confidence: inferred
  path: docs/protocol-notes.md
- confidence: inferred
  path: protocol/compiled/sdc.ir.json
- confidence: inferred
  path: protocol/sdc.protocol.yaml
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T09:20:28Z", actor: "human:operator", revision: 3, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: sdc-root-roadmap-copy-not-approved

## Outcome

`canon simulate --path protocol/sdc.protocol.yaml` no longer prints the stuck state in which the
intake and direction cases ended `passed` while the root case holds a roadmap approval copy whose
result is not `approved`. In that state the root has no outcome and no admissible action except the
explicit `abandoned` decision. After the change the protocol text gives such a root a way on that a
real case can take, or the walk finds the state open.

## Context

Canon's unreleased `canon simulate` (branch `impl/simulate-child-outcome-step` at `70eda28` in the
sibling Canon checkout) printed 4 findings (14 stuck or cycle states) for `sdc`. Canon simulated an
older state of the SDC tree, commit `f364b6b` (`sdc` revision 6, as read for "request 16"), not the
current head (`sdc` revision 8). Canon's second adversary pass on that unit was still running and
may change a figure. Printed summary:

```
simulated: protocol `sdc` revision 6: 78 reachable states, 43 terminable (outcome `abandoned` 39, outcome `operating` 8), 6 waiting on supplied evidence, a child outcome or expiry, 4 findings (14 stuck or cycle states)
```

Three of the four findings are removed by `ebe8694` and `25e0709` (the root outcomes `refused` and
`not_delivered`, `protocol/sdc.protocol.yaml:155-177`); the list is in `story:simulate-as-a-protocol-gate`.
The fourth stands on the current head. Printed text of it:

```
stuck-state: state {evidence `case_outcome` result `passed`, evidence `direction_opened` about `idea`, evidence `intake_opened` about `idea`, evidence `roadmap_approval` about `roadmap`}: no outcome is legitimate and no action is admissible or approval-required; path: action `intake.open` produces evidence `intake_opened` about `idea`, then action `roadmap.read` produces evidence `roadmap_approval` about `roadmap`, then a child of action `intake.open` records evidence `case_outcome` result `passed`, then action `direction.open` produces evidence `direction_opened` about `idea`, then a child of action `direction.open` records evidence `case_outcome` result `passed`; decision `explicitly_abandoned` taken makes outcome `abandoned` legitimate; 3 other stuck states contain it
```

Why it stands on `main`:

- `roadmap.read` (`protocol/sdc.protocol.yaml:81-89`) is admissible while `roadmap.copied` is
  unknown (`:85-87`). The printed state holds a record without the result `approved`, so `roadmap.approved`
  (`:47-53`) stays unknown, and the action text says so: "a second copy with another result would
  leave roadmap.approved unknown for good" (`:83`).
- With `intake.opened` and `direction.opened` true and `children.passed` true (`:60-64`), the
  actions that could move on are blocked: `epics.open` needs `roadmap.approved` (`:115-141`, line
  `:123`).
- No outcome is legitimate: `operating` needs `roadmap.approved` (`:143-150`), `not_delivered`
  needs `children.passed` false (`:167-177`), `refused` needs `direction.opened` unknown
  (`:155-166`), `abandoned` needs the decision (`:151-154`).

In a real case the direction case ends `passed` only on an approved roadmap
(`protocol/sdc-direction.protocol.yaml:302-310`), and a revised roadmap sets the old copy aside and
makes `roadmap.read` admissible again (`protocol/sdc.protocol.yaml:83`). The walk revises no
artifact, so it reaches the state `docs/protocol-notes.md:241-242` classes as "a failed check only
another case revises (a2): no change". This story asks for a protocol text under which the walk finds
nothing, without removing that revision path.

`docs/protocol-notes.md:241-242` classes this finding as a failed check only another case revises, which a real case leaves by revising the artifact and the walk does not model; Canon's revise step for a declared `revises` is `story:protocols-declare-revises-and-authors`.

Candidate, *inferred* and not run: an outcome of the root legitimate when `roadmap.copied` is true and
`roadmap.approved` is unknown, or a `roadmap.read` precondition that holds the copy back until
`direction.opened` and `children.passed` (it is admissible from the empty case today, `:85-87`).
Check each with `canon evaluate` on a case before choosing; the evidence claim's value for a record
of another result is stated only in the action text (`:83`).

## Acceptance

- Once Canon releases `canon simulate`, `canon simulate --path protocol/sdc.protocol.yaml` does not
  print the stuck state `{case_outcome result passed, direction_opened, intake_opened,
  roadmap_approval}` or any state that contains it.
- `scripts/check-protocols.sh` passes, and `canon check` of `sdc` names its state count: 4608
  states on revision 8 (`docs/check-output.txt:10`); a change that adds an evidence kind states the
  new count, which stays under the 65,536 bound.
- Each new case under `cases/` is green in `scripts/evaluate-cases.sh`.
- Alternatively to the first item: or the finding no longer prints once the protocol declares `revises` for the artifact and Canon's simulate walks the revise step.

## Scope

- `protocol/sdc.protocol.yaml` (and its compiled IR `protocol/compiled/sdc.ir.json`)
- fixture cases: `cases/12-root-roadmap-copy-not-approved/` (root at intake and direction passed,
  copy not approved; red on revision 8, green after)
- `docs/protocol-notes.md` ("What `canon simulate` still reports and why")

## Ambiguities

- Whether the root ends on this state without a person or waits for a revised roadmap; a protocol
  outcome ends the case in the driver (*inferred*, not checked), so the choice is the operator's.

## Current figures (Canon a1a937ec on dfe4a3f)

Canon commit `a1a937ec` (branch `wave/2026-11-20`, unreleased) on SDC `main` at `dfe4a3f`, `sdc`
revision 8 (`docs/simulate-findings.md`, stuck state 1):

```
simulated: protocol `sdc` revision 8: 78 reachable states, 54 terminable (outcome `abandoned` 39, outcome `not_delivered` 16, outcome `operating` 8, outcome `refused` 6), 10 waiting on supplied evidence, a child outcome or expiry, 1 finding (1 stuck or cycle state)
```

The finding above is the only one left, and it now prints without "3 other stuck states contain
it". Verdict: `left by revision`.

- A copy that is not `approved` is set aside by the direction case's next roadmap revision. All
  cases read the same `aep/initiative/roadmap.md` digest.
- The printed state needs the direction case to pass at the same roadmap revision it rejected.
  Only a vision revision re-opens that decision (`protocol/sdc-direction.protocol.yaml:319-324`).
  With the roadmap rejected, no direction action can revise the vision (`:218-231`, `:289-299`),
  so only an outside edit of the vision reaches it.
- No trial-3 run has a roadmap rejection.
