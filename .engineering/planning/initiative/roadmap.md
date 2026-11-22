---
format: aep.planning-md/3
id: initiative:roadmap
kind: initiative
status: proposed
title: 'Roadmap: real stories unattended, then retire Conductor and takt'
summary: Milestones M0-M6 toward the proposed goal; stands in for a roadmap kind until AEP has one.
tags:
- roadmap
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:33:30Z", actor: "human:operator", revision: 2, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Initiative: roadmap

AEP has no roadmap kind yet (`upstream-blocker:aep-roadmap-kind`); the kind is being built in the
aep repository today. Until it lands, this `initiative` stands in for the roadmap. Full text:
`docs/roadmap.md`.

## Goal (proposed: the operator has not confirmed the wording)

The system delivers real stories on a real repository unattended, with evidence a program checked,
cheaper and with fewer operator touches than Conductor did: 10 stories on `decider` at autonomy A3
or higher, at most 11 USD per merged item and at most 0.245 operator touches per item, running on
Loom with every command confined; then Conductor and takt are retired.

## Epics in order

| milestone | epics (and stories pulled from other epics) |
|---|---|
| M0 Baseline in git | none (first commit, trial 2 write-up, Gates enrolment) |
| M1 Unattended clean run | `epic:requester-side-and-goals-context`, `epic:decisions-a-person-can-take-cold`; from `epic:conductor-succession`: decider-finding-decision, model-per-node-variable, clean-unattended-run-compare |
| M2 Real verification and real git | `epic:conductor-succession` (verification, git and forge stories); per-story-worktree and story-merge-step from `epic:concurrent-child-cases` |
| M3 Operating record | `epic:brownfield-entry` (pilot subset); `epic:conductor-succession` (operating-record-report, pilot-real-repository); flow-kpis from `epic:flow-limits` |
| M4 On Loom, confined | `epic:kernel-and-runtime-uptake`; `epic:conductor-succession` (Loom and confinement stories); requester-decider-loom |
| M5 Many items | `epic:orchestrate-area-case`, `epic:concurrent-child-cases`, `epic:flow-limits`, `epic:area-board`, rest of `epic:brownfield-entry` |
| M6 Succession complete | `epic:conductor-succession` (takt-freeze-notice, project-rename) |

## Done When

The goal's numbers hold on Loom with confinement (M4 exit), a concurrent wave runs under WIP limits
(M5 exit), and Conductor and takt carry freeze notices (M6 exit).
