---
format: aep.planning-md/3
id: story:daemon-skeleton-on-a-runtime-trait
kind: story
status: proposed
title: Daemon skeleton on a runtime trait with a stub runtime
summary: runbook serve keeps many runs behind the specified API and SSE with resume, on a stub runtime, with the loopback and token floor.
tags:
- one-binary
relations:
- decomposes: epic:runbook-one-binary-and-daemon
- depends_on: story:daemon-api-specified-in-ess
scope:
- confidence: cited
  path: runbook/Cargo.toml
- confidence: inferred
  path: runbook/src/api
- confidence: inferred
  path: runbook/src/commands.rs
- confidence: cited
  path: runbook/src/main.rs
- confidence: inferred
  path: runbook/src/runtime.rs
- confidence: inferred
  path: runbook/src/state.rs
- confidence: inferred
  path: runbook/src/stub.rs
- confidence: cited
  path: runbook/tests
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:47:32Z", actor: "human:operator", revision: 2, executor: "agent:plan-onebin", correlation: "operator-requirement-2026-11-22-one-binary"}
---
# Story: daemon-skeleton-on-a-runtime-trait

## Outcome

`runbook serve` is a daemon that keeps many runs, answers the API specified in ESS and streams
events over SSE, on a runtime trait with a stub runtime. No real runtime is needed for the
conformance suite of `story:daemon-api-specified-in-ess` to be green.

## Source

The operator, 2026-11-22: "using the CLI would allow us to start a case, and this would either
happen basically in process or via a daemon, and the daemon would maintain all the different
runs. And different sessions."

## Context

One command layer (`runbook/src/commands.rs`) holds submit, list, read, cancel, decisions and
sessions; HTTP is one face of it, MCP (`story:mcp-exposure`) and the CLI the others. The runtime
trait has the shape of `runbook::engine` (design decision 2: start a run, answer a decision,
cancel, event sink); the stub runtime emits scripted records and decision requests; Loom
replaces it in `story:engine-on-loom-library-entry`. The security floor of design decision 10
belongs here: loopback by default, a bearer token required for any other address, one check for
every route.

## Acceptance

Every route below is one of the generated OpenAPI document `ess/generated/openapi/openapi/runbookd.yaml`
(commands are `POST /daemon/commands/<name>`, views are `GET /daemon/views/<name>`); the daemon
serves no other path for the API.

- `runbook serve --listen 127.0.0.1:0` prints the bound address on one line and keeps running; with
  `--listen 0.0.0.0:7000` and no token (`--token-file` or `RUNBOOK_TOKEN`) it exits 1 and prints
  that a token is required; with a token, a request without `Authorization: Bearer` gets 401 on
  the `/daemon` routes, the event streams and the UI route, and a request with it gets 200.
- `POST /daemon/commands/submit-run` answers 202 with outcome `submitted` and a run id;
  `GET /daemon/views/runs` lists it; `GET /daemon/views/run?run_id=<id>` shows its state;
  `POST /daemon/commands/cancel-run` moves it to `Cancelled`; a second cancel is refused 409 with
  `RunAlreadyEnded` and the state stays `Cancelled`.
- `GET /daemon/views/events?run_id=<id>&after_position=0` with `Accept: text/event-stream` sends
  events whose SSE `id` is the event's `position`, strictly increasing, with the runtime's record
  unchanged plus run id and sequence number; reconnecting with `Last-Event-ID: <n>` sends exactly
  the events with a position after `n`, with none repeated and none lost; `session-events`
  (`session_id`, `after_position`) and `daemon-events` (`after_position`) do the same.
- A stub run that asks a decision shows up in `GET /daemon/views/open-decisions?run_id=<id>`;
  `POST /daemon/commands/answer-decision` accepts the answer once, and a second answer to the same
  request is refused 409 with `DecisionAlreadyAnswered`.
- A session groups runs: after `POST /daemon/commands/open-session` for `s1` and `s2`, two runs
  submitted into `s1` and one into `s2` are listed per session by
  `GET /daemon/views/runs-of-session?session_id=<id>`, and `session-events` of `s1` carries only
  the events of its two runs.
- A run's record (its store and logs) is under `$XDG_STATE_HOME/runbook/runs/<id>`, and the
  directory for its workspace is `$XDG_STATE_HOME/runbook/workspaces/<id>`, a child of the one
  shared directory `$XDG_STATE_HOME/runbook/workspaces` (the caller's duty of Loom's
  `concurrent-confined-runs`, `docs/library-entries.md`, "What the daemon must do for confined
  runs"); after the daemon is stopped and started again, `GET /daemon/views/runs` lists the same
  runs with the same states.
- The 1 MiB intake limit is counted in bytes by the handler (1,048,576 bytes of UTF-8), tested with a
  multi-byte text.
- The checks of `ContextOutsideRoots` and `NoSuchRunbook` read the host and are tested here.
- The conformance suite of `story:daemon-api-specified-in-ess` runs against the stub daemon and all
  its scenarios pass.
- Budget: no test over 60 s and the test step within 120 s from job start; one daemon is started
  once per test run (a setup step or a cache on disk, not once per test, and not an in-process
  `OnceLock` under cargo-nextest); the step prints its ten slowest tests; each test over 5 s is named
  with its time in the report. New tests go into the existing `runbook/tests/` binary.

## Scope

`runbook/src/` (new modules `api/`, `commands.rs`, `runtime.rs`, `stub.rs`, `state.rs`; one added
subcommand in `main.rs`), `runbook/Cargo.toml` (an HTTP server crate), `runbook/tests/`,
`Taskfile.yml`. Shared files with the later stories are `runbook/src/main.rs` and
`runbook/Cargo.toml`: C lands before D to H, which add their own modules and one registration line
each.
