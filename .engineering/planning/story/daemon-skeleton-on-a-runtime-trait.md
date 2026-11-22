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
revision: 5
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

- `runbook serve --listen 127.0.0.1:0` prints the bound address on one line and keeps running; with
  `--listen 0.0.0.0:7000` and no token (`--token-file` or `RUNBOOK_TOKEN`) it exits 1 and prints
  that a token is required; with a token, a request without `Authorization: Bearer` gets 401 on
  the API, the SSE routes and the UI route, and a request with it gets 200.
- `POST /v1/runs` returns a run id; `GET /v1/runs` lists it; `GET /v1/runs/<id>` shows its state;
  `POST /v1/runs/<id>/cancel` moves it to `cancelled` and a second cancel answers with the same
  state (paths as the ESS-generated OpenAPI document gives them).
- `GET /v1/runs/<id>/events` (SSE) sends events with strictly increasing `id` and the runtime's
  record unchanged plus run id and sequence number; reconnecting with `Last-Event-ID: <n>` sends
  exactly the events after `n`, with none repeated and none lost; the session and daemon streams
  do the same.
- A stub run that asks a decision shows up in `GET /v1/runs/<id>/decisions`; the answer is
  accepted once, a second answer to the same request is refused with 409.
- A session groups runs: two runs submitted into session `s1` and one into `s2` are listed per
  session and the session stream of `s1` carries only the events of its two runs.
- A run's record (its store and logs) is under `$XDG_STATE_HOME/runbook/runs/<id>`, and the
  directory for its workspace is `$XDG_STATE_HOME/runbook/workspaces/<id>`, a child of the one
  shared directory `$XDG_STATE_HOME/runbook/workspaces` (the caller's duty of Loom's
  `concurrent-confined-runs`, `docs/library-entries.md`, "What the daemon must do for confined
  runs"); after the daemon is stopped and started again, `GET /v1/runs` lists the same runs with
  the same states.
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
