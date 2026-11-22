---
format: aep.planning-md/3
id: story:confinement-level-per-run
kind: story
status: proposed
title: Confinement level per run, with a daemon floor
summary: A level chosen per run, default confined, a daemon floor that refuses below it, passed to the runtime.
tags:
- one-binary
relations:
- decomposes: epic:runbook-one-binary-and-daemon
- depends_on: story:daemon-skeleton-on-a-runtime-trait
- depends_on: story:engine-on-loom-library-entry
- informed_by: story:confined-bound-commands
- informed_by: upstream-blocker:loom-confinement
scope:
- confidence: inferred
  path: runbook/src/api
- confidence: inferred
  path: runbook/src/engine
- confidence: cited
  path: runbook/src/main.rs
- confidence: cited
  path: runbook/tests
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:47:35Z", actor: "human:operator", revision: 2, executor: "agent:plan-onebin", correlation: "operator-requirement-2026-11-22-one-binary"}
---
# Story: confinement-level-per-run

## Outcome

Every run has a confinement level that the submitter chooses, the daemon has a floor below which it
refuses, and the level reaches the runtime that starts the bound programs. The default is the
confined level.

## Source

The operator, 2026-11-22: "of course we also want to specify the confinement, the level of
sandboxing." and "a utility daemon which is able to run multiple such cases in an isolated, secure
way".

## Context

Design decision 8: the levels are the ones the runtime offers through Substrate; Runbook invents no
sandbox of its own. The level is a member of the ESS submit command
(`story:daemon-api-specified-in-ess`). The security floor of the daemon's listener (loopback and
token, design decision 10) is not here: it belongs to
`story:daemon-skeleton-on-a-runtime-trait`. Loom's confinement of bound commands is an open
upstream request (`upstream-blocker:loom-confinement`, `story:confined-bound-commands`), which this
story reads and does not edit; the facts are in `docs/library-entries.md`.

## Acceptance

- `runbook run <runbook> --intake <file>` without `--confinement` runs at the confined level:
  `runbook runs show <run> --json` prints `"confinement": "<confined level name>"`.
- `runbook run ... --confinement <level>` for each level the runtime lists
  (`runbook run --list-confinement` prints them, one per line) shows that level in
  `runbook runs show`; an unknown name exits 64 and prints the list.
- The unconfined level runs only when asked for by its name; no other flag or environment variable
  selects it.
- `runbook serve --min-confinement <level>` refuses a submit below the floor: the HTTP answer is 409
  with the floor and the asked level, the MCP `submit_run` returns the same error, `runbook run
  --daemon <url> --confinement <below>` exits 1 with that message, and no run is created
  (`GET /daemon/views/runs` count unchanged).
- A bound program of a run at the confined level cannot write outside its declared paths or open a
  network connection: a fixture binding that tries both ends with the refusal recorded in the run's
  events, and the same fixture at the unconfined level succeeds.
- No secret appears in an event: a test greps the run's events for the token and finds none.
- No test over 60 s, step within 120 s; the confinement tests share one daemon per test run.

## Scope

`runbook/src/engine/` (passes the level to Loom), `runbook/src/api/` (floor check),
`runbook/src/main.rs`, `runbook/tests/`, `cases/` or `runbook/tests/fixtures/` (the fixture
binding), `docs/`. Overlaps D, E, F, G on `main.rs` and `api/`: H starts after C and D.
