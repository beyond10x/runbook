---
format: aep.planning-md/3
id: story:runner-records-action-times-and-output
kind: story
status: active
title: The runner records times, exit and output of every action
summary: 'Per action: start, end, duration, exit, exit class, output digest and a bounded standard error file, in the run directory; the inspector reads them from there.'
tags:
- backlog-2026-11-23
relations:
- informed_by: story:driver-runtime-removed
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T03:08:20Z", actor: "human:operator", revision: 2, executor: "agent:runbook", correlation: "operator-2026-11-23-consumer-gaps"}
- {from: "proposed", to: "active", at: "2026-11-23T03:08:20Z", actor: "human:operator", revision: 3, executor: "agent:runbook", correlation: "operator-2026-11-23-consumer-gaps"}
---
## Outcome

The run directory of `runbook run` records, for every action, when it started, when it ended, how
long it ran, how it exited and where its standard output and standard error lie, so that a reader
of the run (the inspector, the daemon's page, a report, a shell script) shows them without reading
Loom's run log.

## Source

- The consumer session of the operator's own runbooks, 2026-11-23, on the operator's order that its
  gaps go first: "the runner writes no stderr or timings per action".
- Found while `story:inspector-shows-step-products` was built (2026-11-22): the events the runner
  projects drop Loom's `blob_digest` (`runbook/src/loom_run/run_dir.rs`, lines 301 to 312 at
  `a0f7ddc`), and the runner records no start, end or duration per action, only a step span in
  whole seconds (lines 715 to 727). The inspector therefore reads Loom's run log of the case under
  `store/` to find the output blob and says "not recorded by this run" for the times.
- The consumer's request, in the operator's words: "make sure we can look at the actual outputs".

## Acceptance

- Each action record of a run holds `started_at`, `ended_at`, `duration_ms`, `exit` (the code, or
  the signal), the class Loom gave the exit (done, not called, error, timed out) and the digest of
  its output blob, written by the runner from what Loom's run log states. One file per run lists
  them in order (`actions.jsonl`, one line per started action), and `docs/loom-run-directory.md`
  states its members.
- The standard error of a bound program is kept per action in the run directory, bounded in size
  (the bound and what is cut are stated in the page), and the action's line names the file. Where
  Loom's contract gives the runner no standard error of a program, the story says so in its unit
  report and files the request with the Loom session instead of reading around the contract.
- A stored standard error is treated as the output of a program: it is never printed to the
  runner's console or its events, and the inspector shows it as text through its run-path check.
- `outcome.json` gains the sum: actions started, wall time of the run, the slowest action.
- The inspector shows the times and the exit from the run's own records; its read of Loom's run
  log for the blob is removed.
- A run directory written before this change still loads; the page says what it does not record.
- Tests with sh programs hold the members and the bound; no model call; no test over 60 s.

## Scope

`runbook/src/loom_run/run_dir.rs`, `runbook/src/loom_run/run_loop.rs`, `runbook/tests/`, `ui/`,
`docs/loom-run-directory.md`, `CHANGELOG.md`.

## Not in this story

- The claims of an evaluation (`story:loom-run-records-its-claims`).
