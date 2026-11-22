---
format: aep.planning-md/3
id: story:runbook-loom-run-command
kind: story
status: draft
title: 'runbook loom-run: one case tree through b10x-loom with a spend ledger and caps'
summary: The runner that drives one Runbook case tree through the b10x-loom binary as a child process, with caps and a shared spend ledger
relations:
- decomposes: epic:runbook-one-binary-and-daemon
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: docs/loom-run.md
- confidence: cited
  path: runbook/Cargo.lock
- confidence: cited
  path: runbook/Cargo.toml
- confidence: cited
  path: runbook/src/loom_run
- confidence: cited
  path: runbook/src/main.rs
- confidence: cited
  path: runbook/tests/cli.rs
- confidence: cited
  path: runbook/tests/loom_run
revision: 3
---
# Story: runbook-loom-run-command

## Outcome

A person (or a later experiment runner) starts one case tree of a Runbook protocol set with one
command, `runbook loom-run`, and gets a run directory, an outcome and a spend that never passes the
cap, whatever the number of runs that share the ledger.

## Context

The eval of Runbook on Loom needs a runner that drives `b10x-loom` as a child process with model
calls in the loop. Three units build it in parallel on the files named in the interfaces document of
wave rb3: this story is the core (the loop, the caps, the ledger, the child environment); the decider
and the bindings copy, and the run directory writer, are the stories of the other two units.
`docs/loom-run.md` holds the behaviour; the Loom facts it relies on (exit codes of `tree run`, the
four stable members of `decisions[]`, the cost source under `.sdc/llm/`) are the coordinator's brief
of 2026-11-22.

## Acceptance

- `runbook loom-run` refuses with exit 2 and writes nothing when `--loom` is not the file `--loom-sha256`
  names or when `<runs-dir>/<name>` exists; the run directory holds neither a login file's content nor
  its path (tests in `runbook/tests/loom_run/core.rs`).
- The child of `b10x-loom` gets a built environment of `PATH`, `HOME` and `XDG_DATA_HOME` only, and
  `--confinement` is required with no default and passed unchanged to every `tree run`.
- The loop runs `tree run --max-steps 1`, answers each decision through the decider by running its
  printed `command` as data, and ends `denied` when a denied decision returns, `error` with the exit
  code and the stderr tail when a printed command fails.
- The spend cap holds for two processes that share one ledger (0.60 USD per call against a cap of
  2.00 USD never books more than 2.00 USD); the wall-time, model-call and signal caps end the run with
  their own outcome and exit code.
- The run fails closed on an unreadable or invalid ledger, a decider answer without a cost, an unknown
  `b10x-loom` exit code, `tree run` output that is not JSON and a model step without a cost record.
- The end of a run stores `tree status` and `tree verify`, and a non-zero verify makes the outcome `error`.
- The tests of the crate run against a fake `b10x-loom` the tests generate: no model call, no test over
  60 s and the crate's test step within 120 s.

## Out of Scope

- The decider kinds that decide (rule, model) and the rewrite of the bindings copy by settings (the
  decider story of the same wave).
- The run directory files that `sdc-experiment report` and `runbook parity summarize` read (the run
  directory writer story).
- Any run against a model; the first real run is a separate step with its own cap.

## Ambiguities

- A SIGKILL of the runner leaves a reservation in the ledger. `requires-stakeholder-input`: the design
  owner decides whether a stale reservation expires or is reset by hand; until then it counts against the cap.
- One model call can overshoot its `--max-budget-usd` by what the model tool allows. `inferable`: the
  ledger books the real cost and the overshoot is documented in `docs/loom-run.md`.

## Open Questions

None beyond the ambiguities above.
