---
format: aep.planning-md/3
id: story:simulate-as-a-protocol-gate
kind: story
status: proposed
title: canon simulate as a protocol gate
summary: check-protocols.sh runs canon simulate on all eight protocols and fails on a rising finding count
tags:
- blocked-upstream
- canon
relations:
- decomposes: epic:kernel-and-runtime-uptake
scope:
- confidence: inferred
  path: protocol/simulate-expected.json
- confidence: inferred
  path: scripts/check-protocols.sh
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T06:26:12Z", actor: "human:timo", revision: 4, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: simulate-as-a-protocol-gate

## Outcome

`scripts/check-protocols.sh` runs `canon simulate --path <protocol.yaml>` on all eight protocols
and fails on a finding that is an SDC defect.

## Context

Canon session, 2026-11-21. `canon simulate` exits 0 on no finding, 1 on a finding or refusal, 2 on
an unreadable protocol. Output: the findings; then, on a protocol with a `case_open` action, one
line starting `limit:`; then `simulated: protocol ... N reachable states, T terminable (...),
[W waiting on supplied evidence or expiry,] F findings (S stuck or cycle states)`. It is on Canon
branch `wave/2026-11-20` at `49ff2ce`, unreleased.

On SDC's protocols at commit `f364b6b`: 87 findings, classified by Canon as 46 a failed check on an
artifact the protocol revises, 24 on an artifact another case revises, 7 a guard gap in `sdc.plan`,
2 two approval records, 4 lasting denials, 4 false findings of the walk.

Until Canon releases `simulate-revise-step` (wave 8), `simulate-record-groups` (wave 7) and
`simulate-child-outcome-step` (wave 6), most findings are walk artifacts, not SDC defects.

## Acceptance

With a Canon release carrying `canon simulate` pinned, `scripts/check-protocols.sh` prints each
protocol's `simulated:` summary line, compares each protocol's finding count with
`protocol/simulate-expected.json`, and exits non-zero when a count rises above the committed one;
after Canon's waves 6-8 are released it exits non-zero on any finding in a class that is an SDC
defect.

## Scope

- `scripts/check-protocols.sh`
- `protocol/simulate-expected.json` (new: expected finding counts per protocol)

## Ambiguities

- Which of the 87 findings are SDC defects (the 7 `sdc.plan` guard gaps are candidates) is decided
  per class when the waves land; the count ratchet holds until then.

## Findings of the walk at `70eda28` that no longer stand

Canon's `canon simulate` on Canon branch `impl/simulate-child-outcome-step` at `70eda28` printed 9
findings for the SDC tree at `f364b6b` (`sdc` revision 6: 78 reachable states, 43 terminable, 6
waiting, 4 findings, 14 states; `sdc.epic` revision 3: 122 states, 69 terminable, 13 waiting, 5
findings, 25 states). Each was read against the head of `main` (`sdc` revision 8, `sdc.epic`
revision 5) from the protocol text and the `children` rule of Canon's evaluator; the binary was not
built or run. Canon's second adversary pass may change a figure, and the current head may print a
finding the older tree did not.

Removed, 6 findings. A child case that ended with an outcome other than `passed` made
`children.passed` false, and the old tree had no outcome reading that. `ebe8694` (stopped) and
`25e0709` (refused, not_delivered, not_released) added them:
- sdc, `{intake_opened, roadmap_approval, intake.open opened a child}`: waits now; the child's outcome
  reaches `refused` (`protocol/sdc.protocol.yaml:155-166`).
- sdc, `{case_outcome, intake_opened, roadmap_approval result approved}`: `refused` is legitimate
  (`protocol/sdc.protocol.yaml:155-166`; case `cases/09-child-ended-otherwise/root`).
- sdc, `{case_outcome, intake_opened, roadmap_approval}`: `refused` is legitimate, same lines.
- sdc.epic, `{ess_gate, roadmap_approval result approved, spec_opened, spec.open opened a child}`:
  waits now; the child's outcome reaches `not_delivered`
  (`protocol/sdc-epic.protocol.yaml:209-219`).
- sdc.epic, `{case_outcome, ess_gate result passed, roadmap_approval result approved, spec_opened}`:
  `not_delivered` is legitimate, same lines (case `cases/09-child-ended-otherwise/epic`).
- sdc.epic, `{case_outcome, ess_gate, roadmap_approval result approved, spec_opened}`:
  `not_delivered` is legitimate, same lines.

Stand, 3 findings, filed as stories:
- sdc, intake and direction passed with a roadmap copy that is not approved:
  `story:sdc-root-roadmap-copy-not-approved`.
- sdc.epic, `{roadmap_approval}` and `{case_outcome result passed, ess_gate, roadmap_approval result
  approved, spec_opened}`: `story:sdc-epic-record-without-unlocking-result`.

Cannot tell without the binary: none by itself; every verdict above rests on reading, not on a run.
