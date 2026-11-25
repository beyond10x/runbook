---
format: aep.planning-md/3
id: story:program-commands-have-a-deadline-and-a-retry
kind: story
status: active
title: A command of a program step has its own deadline and retry, and a long step has one pattern
summary: A runner-supplied helper gives a command inside a program step a deadline and a retry with each try recorded; one documented long-step pattern with an example runbook.
tags:
- upstream-asks-2026-11-24
refs:
- provider: consumer
  reference: upstream-asks-filed/A4
relations:
- depends_on: story:runner-records-action-times-and-output
- informed_by: story:bounded-command-leaves-no-process
- informed_by: story:bugfix-checks-are-one-action-each
scope:
- confidence: inferred
  path: bindings/tools
- confidence: inferred
  path: cycles
- confidence: cited
  path: docs/loom-bindings.md
- confidence: inferred
  path: runbook/src/loom_run/run_dir.rs
- confidence: inferred
  path: tools
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-25T12:41:42Z", actor: "agent:runbook", revision: 4}
- {from: "proposed", to: "active", at: "2026-11-25T12:41:42Z", actor: "agent:runbook", revision: 5}
---
## Outcome

A command inside a program step has its own deadline and its own retry, supplied by the runner, and a long
step has one documented pattern with an example. A runbook author writes neither in shell.

## Source

Ask A4 of a consumer, filed in its story `upstream-asks-filed` (read 2026-11-24): a per-command deadline and
retry, a long-step pattern and a worktree step as runner features; the consumer has each of them 3 to 5 times
in shell. The worktree step is `story:runner-worktree-step`.

## What exists

- A binding names `timeout_seconds` and `budget.max_runs` for a whole action, and `retry_when` sends a writer
  back (`docs/loom-bindings.md`, "Timeouts" and "Retries"). The action is the unit; a command inside the
  program has nothing.
- `--set timeout_s.<name>` and `max_runs.<name>` change those two for a recorded re-drive.
- The bug-fix runbook gives each check its own time through the pool (`slot --timeout`):
  `story:bugfix-check-names-its-pool-time` and `story:bugfix-checks-are-one-action-each` (units on branches,
  not merged). That is a pool bound for builds, not a runner feature.
- `story:bounded-command-leaves-no-process` (draft): a bound that ends a command should end what it started.

## What this story does

1. A helper supplied beside `runbook` (written in Rust or POSIX sh) runs one command with `--deadline <time>`
   and `--retry <n>` with `--retry-delay <time>` and `--retry-on <exit codes>`. At the deadline it ends the
   command and every process the command started, and exits 124. A retry runs the command again; each try is
   a line (try, exit, duration, deadline reached or not) in the step's record.
2. The deadline of a command is held under the action's `timeout_seconds`: a command deadline longer than the
   time left of its action is refused by name, never extended.
3. The long-step pattern: an action that starts work, an action that polls it with the helper's retry under
   its own deadline, and a total deadline; the pattern is written in `docs/loom-bindings.md` and one example
   runbook is committed with a cycle that runs it.

## Acceptance

With sh stand-ins and times of a second or two (no test over 5 s):

- A command that sleeps past `--deadline 1s` ends at the deadline, leaves no process behind (one that it
  started is ended too), and the helper exits 124; the step's record holds the try and the deadline.
- A command that fails twice and then passes with `--retry 2` exits 0; the record holds three tries with
  their exits. With `--retry-on 7`, an exit 1 is not retried.
- A deadline longer than the action's remaining time is refused with the name of the option.
- The example runbook's cycle: a poll that passes on its third try ends the run as ended; a poll that never
  passes ends at the total deadline with a named outcome, and the evidence of the first action stays.
- `docs/loom-bindings.md` states the helper, its exit codes and the pattern. `runbook check` accepts the
  example.

## Scope

- `tools/`, `bindings/tools/`, `runbook/src/loom_run/run_dir.rs` (the step record), `cycles/`, `bindings/`,
  `docs/loom-bindings.md`, `CHANGELOG.md`. No Python.

## Not in this story

- A wait for the build pool (`slot`), which the pool bounds.
- Changing `timeout_seconds` of an action.
