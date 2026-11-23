---
format: aep.planning-md/3
id: story:runner-records-a-step-in-flight
kind: story
status: active
title: 'A live run shows the step in flight: the runner records an action''s start and itself'
summary: The runner writes a record when an action starts and a record of its own process; the inspector shows running, between steps or not live from them, never unknown.
relations:
- informed_by: story:runner-records-action-times-and-output
scope:
- confidence: cited
  path: runbook/src/loom_run/run_dir.rs
- confidence: cited
  path: runbook/src/loom_run/run_files.rs
- confidence: cited
  path: runbook/src/loom_run/run_loop.rs
- confidence: cited
  path: ui/server.mjs
- confidence: cited
  path: ui/web/now.js
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T11:55:19Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
- {from: "proposed", to: "active", at: "2026-11-23T11:55:19Z", actor: "human:operator", revision: 3, executor: "agent:runbook"}
---
## What the operator saw

2026-11-23, watching a live bug-fix run of release 0.8.0 in the inspector: "I see there is now a case, but
for some reason I'm not seeing any activity. In general, I think we're lacking some visibility into the
system, so I'm always having a hard time understanding what is currently actually happening or processing."
And: "right now i only see: No action in flight. Whether anything chooses the next step is not recorded."

Measured on that run by the session that started it:

- A step had been running for 3 minutes. The run's files name no action that started: the events file has
  `run_started`, `costed_call` and `step_made`, and `step_made` is written after a step ends.
- The inspector's live answer was `state: unknown`, "the runner writes no record of a live process", with
  no records.
- A check step waited 10 to 17 minutes for a build slot before it ran 3 minutes, so most of the run's wall
  time showed nothing.
- Between two steps the tree status showed `stop: BudgetSpent` for the case, which reads as a stopped run
  while the runner was alive and about to start the next step.

## Outcome

For a run of `runbook run`, the inspector says what is happening now:

1. The runner writes a record when an action starts: the action, the case, the try, the time. It writes a
   record of itself when the run starts (process id and its start time, so a reused id is not taken for the
   runner) and removes or closes it when the run ends.
2. The inspector's live answer for such a run is `running` with the action in flight and since when, or
   `between steps` with the runner alive, or `not live` when the runner's process is gone, each from those
   records. It never says `unknown` for a run whose runner wrote them.
3. A case between two steps of a live runner is not shown as stopped.
4. Where the program of the step in flight has written output so far, the page shows its last line (for a
   check step that waits for a build slot, that line says so). If a bound program's output cannot be read
   before it ends, the unit says what is missing and where (the runner or Loom), and the page shows the wait
   time alone.

## What a guard must hold

- The inspector stays read-only: it reads files of the run directory through its one checked open; it sends
  no signal and has no route that acts on a run.
- No line of a ticket, a prompt or a model's answer reaches the page through the last-line display unless it
  is escaped as text, and the display is limited to one line of bounded length.
- The record of the runner holds no environment, no argv and no path outside the run directory.

## Acceptance

Cases with stand-in programs: an action that sleeps shows `running` with its name within 2 s of its start;
the gap between two steps shows `between steps`; a runner killed mid-step shows `not live` naming the last
action started; the page text for each state, at both sizes. No test waits a fixed time.

## Origin

The operator's words of 2026-11-23, relayed by the consumer session with the measurements above.
