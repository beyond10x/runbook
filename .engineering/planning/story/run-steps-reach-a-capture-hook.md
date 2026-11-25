---
format: aep.planning-md/3
id: story:run-steps-reach-a-capture-hook
kind: story
status: draft
title: A run hands what each step did to a capture hook a consumer plugs a recorder into
summary: A step's record reaches only the run directory; a settings-named command receives one record per ended action so a consumer's recorder plugs in.
tags:
- consumer-asks-2026-11-25
relations:
- depends_on: story:daemon-runs-take-a-gateway
- depends_on: story:runner-records-action-times-and-output
- informed_by: story:runner-records-a-step-in-flight
- informed_by: story:program-calls-are-recorded-and-replayed
scope:
- confidence: cited
  path: docs/loom-run-directory.md
- confidence: inferred
  path: docs/loom-run-settings.md
- confidence: inferred
  path: runbook/src/config.rs
- confidence: inferred
  path: runbook/src/loom_run/run_dir.rs
- confidence: inferred
  path: runbook/src/loom_run/run_files.rs
- confidence: inferred
  path: runbook/src/loom_run/run_loop.rs
- confidence: inferred
  path: runbook/src/loom_run/settings.rs
- confidence: cited
  path: runbook/src/runtime.rs
- confidence: inferred
  path: runbook/tests/loom_run
revision: 6
---
## What is missing

A run already reports each program it starts and ends, but only inside the process and only as a start and an end: the trait `StartTarget` has `started` and `ended`, both given a `Started` of `serial`, `role`, `case`, `action` and `try_number` (`runbook/src/runtime.rs:156-179`), and `EventSink::steps` hands it to the runtime (`runbook/src/runtime.rs:182-189`). The end carries no exit, no time and no output. What a step did is written by the runner into the run directory (`story:runner-records-action-times-and-output`: start, end, duration, exit, exit class, output digest and a bounded standard error file; `docs/loom-run-directory.md`), where a consumer reads it after the fact. There is no place where a consumer can plug in a recorder that receives each step's record as the step ends, in its own process and its own store; today it would wrap the bound programs or watch the run directory.

## Outcome

One extension point, the capture hook. The form is a command named in the settings (`runbook.toml` and the run settings, a `[capture]` table with a `command`), which the runner starts once per ended action and gives one JSON record on its standard input, a record of the shape `runbook.capture/1` written in `docs/loom-run-settings.md`. A command and not a library trait, because a consumer is a separate process. The record has the run id, the case, the action, the try, the `serial` of `Started`, the start and end instants, the exit and its class, the digest and run-directory path of the standard output and of the standard error file, and the products the step wrote (path and digest). A step the runner records in `story:runner-records-action-times-and-output` gives the same fields to the hook as to the run directory, from the same code. Runbook's own recorder is the first implementation: a command of `runbook` that appends the records to a file and is covered by the tests, so the extension point is exercised without a consumer.

## What a guard must hold

- The hook cannot change the run: its exit, its output and its absence change no outcome, no spend and no decision of the run. A hook that fails or runs past its bound (named in `docs/loom-run-settings.md`) is killed and recorded as one event of the run; the run goes on.
- The hook receives the same redaction as the run directory: no gateway token, no login and no other secret that `story:model-programs-reach-a-gateway` keeps out of events and files appears in a record or in the hook's argument list or environment (`docs/loom-run-settings.md`).
- One record per `serial`, in `serial` order, exactly once: a run ended early or cancelled gives a record for each step that ended and none for one that did not.
- The command is named by the operator in a settings file; a submit through the daemon cannot name one (the rule `story:daemon-runs-take-a-gateway` states for the gateway).
- A recorded replay (`runbook run --recorded`) states in the story whether it calls the hook, and a test holds the answer.

## Acceptance

With a fixture run of three actions through the fake Loom of `runbook/tests/loom_run/`, the capture command of Runbook's own recorder and a settings file naming it: the recorder's file holds three records in `serial` order, each with the fields above and equal, for the fields both have, to the record of that step in the run directory; with a capture command that exits 1, the run ends with the outcome it has without a hook and one event names the failure; with a command that sleeps past its bound, the step's record is still written, the command is killed and the run goes on; a token placed in the gateway settings is in no record and no event. A submit through the daemon that names a capture command is refused by name.

## Scope

`runbook/src/runtime.rs`, `runbook/src/loom_run/run_loop.rs`, `runbook/src/loom_run/run_dir.rs`, `runbook/src/loom_run/run_files.rs`, `runbook/src/loom_run/settings.rs`, `runbook/src/config.rs`, `docs/loom-run-settings.md`, `docs/loom-run-directory.md`, `runbook/tests/loom_run`.

## Depends on

- `story:daemon-runs-take-a-gateway`: the order is behind the gateway release.
- `story:runner-records-action-times-and-output`: the record's fields come from it.
- Informed by `story:runner-records-a-step-in-flight` (the start record) and `story:program-calls-are-recorded-and-replayed` (a draft that logs the external calls of a program; this story records steps and does not log calls).
- Order: fifth; no other story waits on it.
