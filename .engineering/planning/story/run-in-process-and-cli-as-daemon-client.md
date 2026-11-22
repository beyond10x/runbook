---
format: aep.planning-md/3
id: story:run-in-process-and-cli-as-daemon-client
kind: story
status: proposed
title: runbook run in process and the CLI as a daemon client
summary: 'One command layer, two modes: runbook run in process, and runs, decide and events against a daemon.'
tags:
- one-binary
relations:
- decomposes: epic:runbook-one-binary-and-daemon
- depends_on: story:daemon-skeleton-on-a-runtime-trait
- depends_on: story:engine-on-loom-library-entry
- informed_by: story:runbook-cli-skeleton
scope:
- confidence: inferred
  path: runbook/src/client.rs
- confidence: cited
  path: runbook/src/main.rs
- confidence: inferred
  path: runbook/src/run.rs
- confidence: cited
  path: runbook/tests/cli.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:47:33Z", actor: "human:operator", revision: 2, executor: "agent:plan-onebin", correlation: "operator-requirement-2026-11-22-one-binary"}
---
# Story: run-in-process-and-cli-as-daemon-client

## Outcome

`runbook run` runs one case in process through the engine, and the same commands talk to a daemon
when one is named: one binary, two modes, one command layer.

## Source

The operator, 2026-11-22: "What we want is one pre-built runbook binary, which can just run a
certain case, given that context, the intake, and given the protocol which is being specified, and
given certain command line arguments." and "using the CLI would allow us to start a case, and this
would either happen basically in process or via a daemon".

## Context

Design decision 3 gives the commands. `runbook run` keeps the instruction rule: instructions come
from the runbook's files (`instructions/`), never from the binary. `story:runbook-cli-skeleton`
built `doctor`, `list` and `--version`; this story adds the rest of the commands to the same crate.

## Acceptance

- `runbook run sdc --intake <file> --context <dir> --set key=value` runs in process, prints the
  run id and one line per case outcome, and exits 0 when the root case ends `delivered`, 3 when it
  stops for a person's decision (the exit codes of `scripts/run-trial.sh` today), 1 on failure.
  `--intake -` reads standard input.
- `runbook run ... --detach` with `--daemon <url>` (or `RUNBOOK_DAEMON`) submits the run to the
  daemon, prints its run id and exits 0 at once; without a daemon, `--detach` exits 64 and says a
  daemon is needed.
- `runbook runs list`, `runbook runs show <run>`, `runbook runs events <run>` (follows the SSE
  stream until the run ends, `--no-follow` prints what exists), `runbook runs cancel <run>` and
  `runbook decide <run> <request> <answer>` print what the matching API routes return, against a
  daemon started with `runbook serve`; each has `--json`.
- Against a stub daemon `runbook runs events <run>` prints the same lines as `curl -N` of the run's
  SSE route, and after a dropped connection resumes with `Last-Event-ID` and prints no event twice.
- An unknown subcommand or a missing `--intake` exits 64 with the usage.
- `runbook run` on a fixture case ends with the same outcome as `story:engine-on-loom-library-entry`'s
  engine test and starts none of `canon`, `b10x-loom`, `loom`, `sdc-drive`.
- No test over 60 s, the step within 120 s; new tests go into `runbook/tests/cli.rs`.

## Scope

`runbook/src/main.rs`, `runbook/src/run.rs`, `runbook/src/client.rs` (new), `runbook/tests/cli.rs`,
`docs/` (one page for the commands), `CHANGELOG.md` (one line). Overlaps D, F, G, H on
`runbook/src/main.rs`: E starts after D.
