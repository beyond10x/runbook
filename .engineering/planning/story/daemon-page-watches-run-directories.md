---
format: aep.planning-md/3
id: story:daemon-page-watches-run-directories
kind: story
status: active
title: One page shows the daemon's runs and the run directories of runbook run
summary: A daemon run is written in the layout of runbook run, and runbook serve --ui --runs DIR lists run directories read-only beside its own runs.
tags:
- backlog-2026-11-23
relations:
- informed_by: story:ui-embedded-in-the-binary
- depends_on: story:daemon-page-starts-a-run
scope:
- confidence: cited
  path: docs
- confidence: cited
  path: runbook/src/api
- confidence: cited
  path: runbook/src/engine
- confidence: cited
  path: runbook/src/loom_run/run_dir.rs
- confidence: cited
  path: runbook/src/serve.rs
- confidence: cited
  path: ui/server.mjs
- confidence: cited
  path: ui/test
- confidence: cited
  path: ui/web
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T01:25:46Z", actor: "human:operator", revision: 2, executor: "agent:runbook", correlation: "operator-2026-11-23-start-from-page"}
- {from: "proposed", to: "active", at: "2026-11-23T01:25:46Z", actor: "human:operator", revision: 3, executor: "agent:runbook", correlation: "operator-2026-11-23-start-from-page"}
---
## Outcome

One page shows both kinds of run: the runs the daemon started and the run directories that
`runbook run` wrote. A run of `runbook run` is shown in watch mode (read-only); a run of the daemon
in operate mode.

## Source

- The operator, 2026-11-23, relayed by the consumer session: one page with two modes, watch
  (read-only) and operate (start a run, take decisions). The consumer's question: a run started by
  the daemon lies under the daemon's state directory, the consumer's runs under its own `runs/`;
  can one page show both?
- Read at `main` `566a0c3f`: the daemon keeps a run at `<state>/runs/<run id>/tree`
  (`runbook/src/engine/mod.rs:24`), `runbook run` writes `<runs dir>/<name>/store` with
  `settings.json`, `status.json` and the call and decision logs (`runbook/src/loom_run/run_dir.rs`).
  The inspector finds only `<run>/store` (`ui/server.mjs:134`). The two layouts differ, so neither
  page shows the other's runs.

## Acceptance

- A run the daemon starts under `--runtime loom` is written in the layout of `runbook run` (one
  writer of a run directory, `runbook/src/loom_run/run_dir.rs`), or the difference is closed in the
  reader: after this story `ui/server.mjs --runs <state>/runs` shows a daemon run with its steps,
  decisions and status.
- `runbook serve --ui --runs DIR` (repeatable) lists the run directories under `DIR` beside the
  daemon's own runs. Such a run is read-only on the page: no decision can be answered for it and
  the page says "watch". Every read goes through the run-path check; a path with a dot-directory
  component is neither listed nor served.
- A test with a hand-written fixture and a stub daemon runtime; no model call. No test over 60 s.

## Scope

`runbook/src/engine/`, `runbook/src/loom_run/run_dir.rs`, `runbook/src/api/`, `runbook/src/serve.rs`,
`ui/web/`, `ui/server.mjs`, `ui/test/`, `docs/daemon-api.md`, `docs/engine.md`, `ui/README.md`.

## Not in this story

- The start form (`story:daemon-page-starts-a-run`).
