---
format: aep.planning-md/3
id: epic:conductor-succession
kind: epic
status: proposed
title: 'Conductor succession: close the 12 gaps'
summary: SDC succeeds Conductor (operator decision, 2026-11-21); the 12 gaps of docs/conductor-comparison.md section 7 are closed first.
tags:
- conductor-succession
relations:
- decomposes: initiative:roadmap
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:25:39Z", actor: "human:timo", revision: 3, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Epic: conductor-succession

## Decision

The operator, 2026-11-21, in the sdc session: "So we're replacing conductor then. It's a standing
decision here." SDC succeeds Conductor; Conductor stays frozen; the 12 gaps of
`docs/conductor-comparison.md` § 7 are closed first. Record:
`approval-record:operator-decision-conductor-succession`.

## Outcome

SDC can do the work Conductor does today: every gap of `docs/conductor-comparison.md` § 7 is closed
by an implemented artifact or a released upstream capability, and an operating record on a real
repository shows it.

## Gaps, state on 2026-11-21, and what closes each

| # | gap | state today | closed by |
|---|---|---|---|
| 1 | version control and planning store | store populated; first commit prepared, not made | the operator's first commit (no story) |
| 2 | real verification | conformance runs against the interpreter; reviews are `test -f` stubs (`docs/driver.md` gaps 16, 17); scripted false done = 1 (`docs/eval-design.md:616`) | `story:conformance-against-implementation`, `story:independent-reviews`, `story:verifier-independence` |
| 3 | real git | merge and publish are `cp` (`driver/examples/sdc/tools/sdc-tool.sh:255`, `:260`) | `story:per-story-worktree`, `story:story-merge-step`, `story:story-branch`, `story:change-merge-git`, `story:release-tag` |
| 4 | forge evidence | none in protocols or bindings | `story:forge-pr-evidence`, `story:forge-ci-evidence`, `story:forge-release-evidence` |
| 5 | parallelism with WIP limits | declared, not executed (`docs/orchestration.md` § 1.1) | `epic:concurrent-child-cases`, `epic:flow-limits` |
| 6 | resume of a cut step | observation records exist (`docs/driver.md` "Observation records"); no resume (gap 15) | `story:resume-cut-step` |
| 7 | confinement | bound commands run unconfined (`docs/layers.md` § 4 row 1) | `story:confined-bound-commands`, blocked by `upstream-blocker:loom-confinement` |
| 8 | run on Loom | SDC runs its own runtime copy | `story:bindings-to-loom-bindings-1`, `story:sdc-on-loom-tree-run` (blocked by `upstream-blocker:loom-tree-run`), `story:loom-concurrent-children` |
| 9 | brownfield entry | rated 2 of 5 | `epic:brownfield-entry` |
| 10 | standing operation | drafted protocol only | `story:orchestrate-standing-horizon`, `story:standing-wakeup` |
| 11 | autonomy routing, authority bound to a revision | revision-bound records stand in for authority (`docs/protocol-notes.md:174-178`) | `story:requester-decider`, `story:requester-decider-loom` (blocked by `upstream-blocker:loom-decider-per-class`), `story:authority-revision-binding-uptake` (blocked by `upstream-blocker:canon-authority-revision-binding`) |
| 12 | operating record | one run, one toy story | `story:operating-record-report`, `story:pilot-real-repository` |

From the 2026-11-21 trial: `story:decider-finding-decision`. The used-up writer verdict needs no
story: `sdc-drive` already ends with `writer_spent` (`docs/driver.md` "Run caps";
`driver/src/main.rs:715-719`).

## Out of Scope

Conductor's own self-maintenance, multi-instance daemon and API, account rotation
(`docs/conductor-comparison.md` § 7, "Not listed on purpose"). No change to Conductor.

## Done When

Every artifact in the table is implemented or its upstream blocker cleared and taken up, and
`story:pilot-real-repository` has an operating record.

## Gap 12 additions

Also under gap 12, from the operator's request of 2026-11-21 (waiting time and model choice):
`story:model-per-node-variable` and `story:clean-unattended-run-compare` (trial 3). The store has no
eval epic, so they sit here.
