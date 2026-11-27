---
format: aep.planning-md/3
id: story:a-case-of-a-run-ended-by-a-cap-is-not-shown-stuck
kind: story
status: active
title: A case of a run ended by a cap is not shown as stuck
summary: The cases view lists the open case of a run ended by its spend or call cap as Stuck, because the one-step BudgetSpent line is its last stop.
relations:
- depends_on: story:a-budget-stop-in-a-running-run-is-named
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T06:41:05Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-27T06:41:05Z", actor: "agent:runbook", revision: 3}
---
# A case of a run ended by a cap is not shown as stuck

## Origin

Found on 2026-11-26 while naming the one-step `Stopped` line (`story:a-budget-stop-in-a-running-run-is-named`).
The operator's session approved filing it as a red-first defect for the next wave.

## Seen

Inferred from the code, not yet observed in a run. The `cases` view (`runbook/src/commands.rs`, around line 1983)
shows a case whose last `Stopped` line has `stop: BudgetSpent` as `Stuck` once the run has ended. In a daemon run the
engine gives every Loom `run_tree` call one step (`runbook/src/engine/mod.rs:509`), so a case's last `Stopped` line is
nearly always the one-step `BudgetSpent`/`MaxSteps` line. A run that the daemon ends by its spend cap or its model-call
cap then shows its open case as `Stuck`, although the case did not get stuck: the run was ended by a cap.

`docs/daemon-api.md` (around line 533) and `docs/engine.md` (around line 308) state the current mapping, so the fix
changes what the docs promise.

## Acceptance

1. A red-first daemon case: a run on a fake route that the daemon ends by its spend cap (and one by its model-call cap)
   lists its open case in the `cases` view with the run's cap end named (for example `ended by the spend cap`), never
   `Stuck`.
2. A case whose tree was left with no admissible move still lists as `Stuck`.
3. A case that waits on a person still lists as waiting.
4. `docs/daemon-api.md` and `docs/engine.md` say how a case of a run ended by a cap is shown.

## Limit

Acceptance 2 holds for a tree left without a move under no cap: that case lists as `Stuck`. A tree left without a
move by the step that reaches a cap lists the cap word (`SpendCapReached`, `CallCapReached`, `WallCapReached`), like a
tree with a move left: the engine checks the caps before it asks Loom for the next move, so no `Stuck` line exists, and
telling the two apart needs Loom to evaluate the tree without taking a step, a call runbook does not have. The case
`a_tree_left_without_a_move_shows_stuck_with_no_cap_near_and_the_cap_word_when_its_last_step_reached_the_cap` in
`runbook/tests/daemon/caps.rs` holds this; `story:a-capped-runs-stuck-tree-shows-stuck` is the follow-up.
