---
format: aep.planning-md/3
id: epic:decision-branches
kind: epic
status: draft
title: 'Decision branches: time travel, backtrack and fork a run at any decision'
summary: From any decision point in a run's ledger, see the state at that point, re-take the decision with another option as a new run, or run several options side by side and compare them.
relations:
- decomposes: initiative:roadmap
revision: 1
---
# Epic: decision-branches

## Origin

The operator's idea, 2026-11-26 about 07:55 CET, relayed by the host's operator session, filed as a
draft for later (not for the wave in flight). His words: "decission exploration backtracking,
timetravel, forking different vartiants based on disissions".

## Outcome

Every decision point of a run is a branch point. From any decision recorded in a run's ledger a
person can:

1. **Time travel**: see the run's state as it was at that decision: the case tree, the open
   question and its options, the files of the run's workspace, the model calls made so far and the
   money booked so far.
2. **Backtrack**: take that decision again with another option and continue the run from there,
   as a new run that names its parent run, the decision and the option taken.
3. **Fork**: start several variants from one decision side by side, one option each, and compare
   their outcomes (end state, products, calls, money, wall time) in one view.

A decision here is any point where the run took one option of several: a question to a person
(`decision_requested` / `runbook decide`), a decider's answer, and a step's choice of the next
action the tree recorded (`Evaluated`).

## What the ledger already gives

- An order of events per run: the daemon journal's `position` and `sequence`, and each case's
  events with `seq` and `prev` (a hash chain), so "the state at decision N" is a prefix of a chain.
- Decision requests with their options, answer, `answered_by` and times in the journal.
- Idempotency keys on submit, so a branch run can be submitted once per (parent, decision, option).
- The runbook pin of each run (`runbook-pin.json`), so a branch runs the same runbook version.
- Recorded model and decider calls per run (`.sdc/llm/<n>-<step>/` request, response, events;
  `replay/calls.jsonl`), and the recorded re-drive: `runbook run --recorded <run dir>` answers every
  model and decider call from the recording, with `--on-divergence stop|nearest|live`, `--set`
  conditions, `--take-recorded <actions>` and `--inject reject@<decision>#<n>` (`docs/loom-run.md`,
  "A recorded run"). That is a re-drive of a whole run with one decision changed, in process.
- The file-change journal of a watched run (`<run>/fs/changes.jsonl`, `docs/fs-changes.md`).

## What is missing

- The state at a decision as one view: the workspace's files at that event are not kept (the
  change journal exists only for `--watch` runs in process, and only as changes).
- A branch run in the daemon: a submit that names a parent run, a decision and an option, starts
  from the recorded prefix up to that decision and runs live after it. Today the re-drive is in
  process only, takes the whole recording, and changes a decision only as an injected rejection.
- An answer other than a rejection at a recorded decision (take option B at decision N).
- A run record field for the parent link (parent run, decision id, option), and a tree of runs
  built from it.
- A compare view of the variants of one decision: end states, products, calls, money, wall time.
- Money for a fork: each variant books its own calls against its own cap; a fork of N variants
  needs a cap for the whole fork, named before it starts.

## Out of scope for now

- Changing a decision inside a run that is still running (a branch is always a new run).
- Merging two variants back into one run.

## Related

- `epic:decisions-a-person-can-take-cold`: what a decision request shows a person. This epic
  uses those requests as branch points and does not change what they show.
