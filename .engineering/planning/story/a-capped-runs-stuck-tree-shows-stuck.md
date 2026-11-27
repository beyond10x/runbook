---
format: aep.planning-md/3
id: story:a-capped-runs-stuck-tree-shows-stuck
kind: story
status: draft
title: A tree left without a move by the step that reaches a cap shows Stuck
summary: A capped run's case left without a move by its last step lists the cap word; telling it from a tree with a move left needs Loom to evaluate without a step.
relations:
- depends_on: story:a-case-of-a-run-ended-by-a-cap-is-not-shown-stuck
revision: 1
---
# A tree left without a move by the step that reaches a cap shows Stuck

## Origin

Found on 2026-11-27 by the adversary of `story:a-case-of-a-run-ended-by-a-cap-is-not-shown-stuck`. That story shows an
open case of a run the daemon ended at a cap with the cap word (`SpendCapReached`, `CallCapReached`, `WallCapReached`)
and names this case as its limit.

## The case

A daemon run whose step uses the last of its spend cap or its call cap and, with the same step, leaves the tree with no
admissible move (in the test, a check that books its call and then fails, so the publish can never become admissible).
With no cap near, the same run ends `failed: step ...` and its case lists as `Stuck`. With the cap, the engine checks the
caps at the top of its step loop (`runbook/src/engine/mod.rs`, `gate.admit()`) before it asks Loom for the next move, so
no `Stuck` line is written; the case's last stop is the step's `BudgetSpent` line and the view `cases` lists the cap word.

## Why the recorded lines are not enough

- A `BudgetSpent` line always has `next: []`, and Loom records no evaluation after the last step.
- Asking Loom with a raised cancel flag (a `Stopped` `Cancelled` line with its `next`) was tried and is unsound: an empty
  `next` does not prove that no move is left.
  - A case whose last step met its outcome gets `next: []`, because Loom records the case's end only on its next call.
  - A case whose next move is a person's review gets `next: []`, because Loom's `next_action` leaves out person actions.
  - The `Cancelled` line becomes a run event that the run page words "Cancelled", for a run nobody cancelled.

## What it needs from Loom

A call that evaluates the tree without taking a step, starting a program or writing a line that reads as a stop, and
answers what the tree's next call would end as: `Terminal`, `AwaitingPerson` (or `AwaitingAuthority`), `Stuck`, or a move
left. The request goes to Loom's session (`docs/canon-requests.md` or a story marked blocked upstream), not into a patch
here.

## Acceptance

1. The case `a_tree_left_without_a_move_shows_stuck_with_no_cap_near_and_the_cap_word_when_its_last_step_reached_the_cap`
   in `runbook/tests/daemon/caps.rs` is renamed to say so and asserts `Stuck` for the capped runs too.
2. The cases of a case that met its outcome on its last step, of a case whose next move is a person's, and of a run with
   no event that reads as a cancel stay green.
