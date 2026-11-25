---
format: aep.planning-md/3
id: story:program-calls-are-recorded-and-replayed
kind: story
status: active
title: Every external call of a program is logged, answers are redacted by one list, and a run replays from the log
summary: A call helper logs route, input, try, exit and answer file of each external call; one redaction list applies to inputs, answers and log; a replay answers every call from the log and starts no external command.
tags:
- upstream-asks-2026-11-24
refs:
- provider: consumer
  reference: upstream-asks-filed/A5
relations:
- depends_on: story:runner-records-action-times-and-output
- informed_by: story:recorded-run-replay
- informed_by: story:replay-plan-out-of-the-run-copy
- informed_by: story:model-entry-for-bound-programs
scope:
- confidence: inferred
  path: bindings/tools
- confidence: cited
  path: docs/loom-run-directory.md
- confidence: cited
  path: docs/loom-run-settings.md
- confidence: cited
  path: docs/loom-run.md
- confidence: inferred
  path: runbook/src/loom_run/replay.rs
- confidence: inferred
  path: runbook/src/loom_run/run_dir.rs
- confidence: inferred
  path: runbook/src/loom_run/settings.rs
- confidence: inferred
  path: runbook/tests/loom_run
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-25T12:41:42Z", actor: "agent:runbook", revision: 4}
- {from: "proposed", to: "active", at: "2026-11-25T12:41:42Z", actor: "agent:runbook", revision: 5}
---
## Outcome

Every external call a bound program makes through the runner's call helper is logged, its answer is redacted
by one list, and a run can be driven again from that log with no external call.

## Source

Ask A5 of a consumer, filed in its story `upstream-asks-filed` (read 2026-11-24): record and replay of program
calls with one redaction list. Every external call is logged (route, input, try, exit, answer file), answers
are redacted, and `replay=<run>` answers from the log. The consumer has this in shell.

## What exists

- `runbook run --recorded <source>` re-drives a run with every model call and every decider call answered from
  a recording or a run directory (`docs/loom-run.md`, "A recorded run"; `story:recorded-run-replay`). It covers
  model and decider calls only, and stops as `stopped: recording diverged` (exit 11) on a call with no reply.
- `story:runner-records-action-times-and-output` (active; not on `origin/main`: no `actions.jsonl` is
  written there) records the times, exit and output digest of every action. It records the action, not the
  calls a program makes inside it.
- `story:replay-plan-out-of-the-run-copy` (draft): the plan and the call count of a recorded run must be out of
  reach of the programs it starts. The log of this story has the same property to hold.
- The gateway unit filters the token out of what a model process prints before it writes
  `events.jsonl` (`story:model-programs-reach-a-gateway`). That is one value in one place, not a list.

## What this story does

1. A call helper supplied beside `runbook` (Rust or POSIX sh; the model entry of
   `story:model-entry-for-bound-programs` is one route of it) runs one external call for a bound program. For
   each call it appends a line to `calls.jsonl` of the run directory: the route (a name the program gives:
   a model, a connector operation, a host), the input (a file and its digest), the try, the exit and the answer
   file.
2. One redaction list names what must not stay: literal values, patterns, and the names of environment
   variables whose values are read at the start. It is one file of the run's settings. It is applied to the
   input file, the answer file and the log line before they are written; a value of the list is in no file of
   the run directory.
3. A run started with a replay of an earlier run (the consumer's spelling `replay=<run>`; the option's name is
   the unit's, and it may be the same mechanism as `--recorded`) starts no external command: the helper
   answers each call from the log by route, the digest of the redacted input and the try. The answers it gives
   are the redacted ones.
4. A call with no line in the log ends the run `stopped: recording diverged` with the call named, as the
   existing recorded run does.

## What a guard must hold

- The log and the redaction list are out of reach of a bound program under `--confinement substrate`; under
  `local` the page says what a program can rewrite (as `docs/loom-run.md` does for the replay plan).
- A replay answers only from the log of the named run; a route or a digest outside it is not answered from
  anywhere else.
- A value of the list is in no event, no log line and no refusal text.
- A program that does not use the helper is neither logged nor replayed; the page names that limit.

## Acceptance

With sh stand-ins for the external commands (no network):

- A run whose program makes three calls through the helper leaves three lines in `calls.jsonl` with the
  members above, and the answer files exist.
- A value in the redaction list that the stand-in prints appears in no file of the run directory, in the
  input file, the answer file and the log line alike.
- A replay of that run ends as the first did, the stand-ins are not started (a marker file shows it), and the
  answers are the redacted ones.
- A replay with a program that makes a fourth call ends `stopped: recording diverged` naming it.
- A retry of one call is two lines with try 1 and try 2.
- Each case runs under 5 s; the page `docs/loom-run.md` states the log's members and the limit above.

## Scope

- `bindings/tools/`, `runbook/src/loom_run/` (`replay.rs`, `run_dir.rs`, `settings.rs`),
  `runbook/tests/loom_run/`, `docs/loom-run.md`, `docs/loom-run-directory.md`, `docs/loom-run-settings.md`,
  `CHANGELOG.md`.

## Not in this story

- Replay of a model's quality under changed conditions (`story:recorded-run-replay`).
