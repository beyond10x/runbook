---
format: aep.planning-md/3
id: story:a-budget-stop-in-a-running-run-is-named
kind: story
status: active
title: A step's budget stop in a running run is named as the end of a step
summary: Every step of a daemon run ends with a Loom Stopped BudgetSpent line; a run event of runbook's own names it as a step's end, not the run's.
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-26T19:00:14Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-26T19:00:14Z", actor: "agent:runbook", revision: 3}
---
## Seen

2026-11-26, a live run of the runbook `investigate` on a daemon of release 0.12.0 (`main` has the same engine): the run's state stayed `Running` and its page said "running a step", while its run events showed, after every finished action, a Loom line `Stopped` with `"stop":"BudgetSpent","next":[]`. Newest first: `runbook.action-started` (`findings.judge`), `Evaluated`, `Stopped` (`BudgetSpent`), `ActionFinished` (`report.write`), `runbook.action-started`, `Evaluated`, `Stopped`, `ActionFinished`, and so on. A reader of the events sees the run stop after every step while the daemon says it runs.

## What it means

No defect of the run. The engine drives the tree one step at a time: every `run_tree` call gets `max_steps` 1 (`runbook/src/engine/mod.rs:509`, `calls::run_settings`; `docs/engine.md`, "Caps"). Loom, before the second step of the call, finds its step budget spent, appends `Stopped` with `stop: BudgetSpent` and `budget: MaxSteps` to the log of the deepest open case and returns `TreeStop::BudgetSpent` (Loom `ed644a4d`, `crates/loom-tree/src/run.rs:245-304`). The engine takes that stop as one step done and goes on to the next step after holding the caps again (`runbook/src/engine/mod.rs:552-553`). The engine reports every run-log line unchanged as a run event (`runbook/src/engine/tail.rs`; the test `every_run_log_line_reaches_the_record_unchanged`), so the pacing stop reaches every reader as `Stopped`.

The defect is what runbook publishes: the events give a reader no way to tell this stop, the end of one `run_tree` call of one step, from a stop that ends the run or makes it wait. The record of a run-log line must stay byte for byte as Loom wrote it (its `prev` digest chain, `docs/daemon-api.md`, "Event streams"), so the name is a run event of runbook's own beside it.

## Acceptance

- After each `Stopped` line with `stop: BudgetSpent` that the engine takes as the end of one step, the run's events hold a run event `runbook.step-ended` with the record `{case, seq}`: the case whose log holds that line and the line's `seq`; it comes before the next run-log line of the run.
- A `Stopped` line that is not the end of a step (a wait on a person or on authority, `Stuck`) has no `runbook.step-ended`.
- Every run-log line still reaches the run's record unchanged.
- `docs/daemon-api.md` ("Event streams") and `docs/engine.md` ("Caps") name the event and tell a reader that a run ends by its state and outcome, never by a Loom `Stopped` line.
- The case: a run of the gate lane's engine runbook through two reviews to `delivered`, red before the change.
