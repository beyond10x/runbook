---
format: aep.planning-md/3
id: story:spend-cap-with-nothing-left-starts-nothing
kind: story
status: active
title: A spend cap with nothing left starts nothing
summary: 'Two findings of security pass 4 of run-without-model-node-needs-no-caps in code on main: the stop before a tree run depends on an option the model tool names, and a decision printed by the step that passed the cap is still answered.'
relations:
- informed_by: story:run-without-model-node-needs-no-caps
- depends_on: story:caps-are-optional
scope:
- confidence: cited
  path: runbook/src/loom_run/budget.rs
- confidence: cited
  path: runbook/src/loom_run/run_loop.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T09:19:06Z", actor: "human:operator", revision: 4, executor: "agent:runbook"}
- {from: "proposed", to: "active", at: "2026-11-23T09:19:06Z", actor: "human:operator", revision: 5, executor: "agent:runbook"}
---
## Outcome

A spend cap that has nothing left starts nothing: no `tree run` and no `tree approve` follows, whatever the
bindings of the run name.

## Source

Security pass 4 of `story:run-without-model-node-needs-no-caps` (2026-11-23), two findings in code that is
on `main` and that the unit did not change. The standing rule of the runner is that the spend cap fails
closed.

- The stop before a `tree run` reads the amount the runner wants to reserve for the step. For a run with a
  model node, that amount comes from an option of the declared model tool; where the tool does not name it,
  the stop does not apply and the run ends by its call cap instead. The guard must hold in the place that
  reserves: a cap with nothing left stops before a `tree run`, whatever the bindings name.
- After a step booked past the cap, a decision that the same step printed is still answered when the decider
  costs nothing, and the answer runs the program a person action binds. The guard must hold: once the cap is
  passed, the runner makes no further call to Loom that can start a program.

Every `sdc-node` binding under `bindings/` names the option, so no committed runbook is affected by the first
point. The reviewer's two measurements are committed with that unit as ignored tests that fail until this
story is built.

## Scope

- `runbook/src/loom_run/run_loop.rs` (`costed`, the reservation, `take_answer`), `runbook/src/loom_run/budget.rs`.
- `runbook/tests/loom_run/`: the two measurements become cases of the default step.
- `docs/loom-run.md`, `docs/cli.md`: the limit sentence of the unit is removed when the guard holds.

## Acceptance

- With a cap that has nothing left, a run with a model node makes no `tree run`, for a model tool that
  names its budget option and for one that does not; the run ends with the outcome of a reached cap.
- After the cap is passed, no `tree approve` is made; the run ends with the outcome of a reached cap and the
  open decision stays in the records as open.
- A run with a model node and a cap that has room behaves as before: the committed spend cases pass
  unchanged.
- A security review pass covers the change before it merges.

## Not in this story

- The amount one step may record (what one step records is booked whole; a bound per step is a change of
  Loom's program contract).
- The open question whether caps of 0 may start a run without a model node under `local`.
