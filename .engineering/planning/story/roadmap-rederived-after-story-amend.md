---
format: aep.planning-md/3
id: story:roadmap-rederived-after-story-amend
kind: story
status: proposed
title: Roadmap re-derived after a later stage amends a requirement
summary: A plan or story stage amendment leaves no approved roadmap that contradicts the amended requirements (runs/trial-3-mixed-3 decider 009-011)
tags:
- conductor-succession
- late-findings
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: inferred
  path: driver/src/bin/sdc-findings.rs
- confidence: inferred
  path: scripts/run-finding-cycles.sh
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T10:42:55Z", actor: "human:operator", revision: 2, executor: "agent:sdc-superseded", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: roadmap-rederived-after-story-amend

## Outcome

A plan or story stage amendment that changes a requirement the approved roadmap states leaves no
roadmap that contradicts the amended requirements: the roadmap is re-derived (or set aside and
rewritten) as the vision is after a context-stage amendment, or the contradiction is reported
where a decider and a person read it.

## Context

`runs/trial-3-mixed-3` (unattended, A5): the build's contradiction f5 (cites r3 and q5) was decided
by the agent as q8 "Offline step only opens the cached list; offline edits are queued in the sync
story" (`store/IDEA-1/work/questions.yaml`, q8 `answered_by: non-interactive`). The roadmap kept
"caching makes offline opening and editing possible" (`work/aep/initiative/roadmap.md:31`). The
decider then read the two against each other: `decider/009-merge.approve` escalated (best reject)
on "the inputs disagree on which step owns offline editing"; `decider/010-merge.approve` approved
with "aep/initiative/roadmap.md:31 still says caching makes offline editing possible, which
conflicts with q8; the decided requirement governs, so the roadmap needs an update";
`decider/011-merge.approve` escalated (best reject) citing the roadmap's done-means
(`roadmap.md:25`) against q8.

Cause: `sdc-findings` `mark_derived` (`driver/src/bin/sdc-findings.rs`, `DERIVED`) re-derives only
the amending stage's own tree (`story` marks `impl/`); only a context-stage amendment sets the
vision aside. Nothing marks `aep/initiative/roadmap.md` after a later amendment, and
`sdc.direction`, which writes and approves it, has ended by then (`docs/trial-2.md`, "Limits": "An
approved vision is not re-derived when a later stage amends the requirements").

## Acceptance

A stub cycle where a story-stage finding amends a requirement the roadmap states ends with either a
roadmap revision that agrees with the amended requirement, or a recorded reason the run stops on;
no decider record of the cycle reads a roadmap line against a decision taken after the roadmap was
approved.

## Scope

`driver/src/bin/sdc-findings.rs` (`mark_derived`), the direction and epic protocols if a
re-derivation needs a path back to `roadmap.write`, `scripts/run-finding-cycles.sh`.

## Ambiguities

- Whether the roadmap is re-derived (a protocol path back into an ended `sdc.direction`, which the
  epic protocol has no room for today, `docs/trial-2.md` "Limits") or the decider input carries the
  decisions taken after the roadmap's approval so the decider reads them as governing.
