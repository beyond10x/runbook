---
format: aep.planning-md/3
id: story:runbooks-load-from-configured-paths
kind: story
status: active
title: runbook.toml names the directories runbooks load from
summary: runbook list, run and serve resolve a runbook name through one list of directories from runbook.toml and the built-in set.
tags:
- backlog-2026-11-23
relations:
- informed_by: epic:runbook-one-binary-and-daemon
scope:
- confidence: cited
  path: runbook/src/assets.rs
- confidence: cited
  path: runbook/src/config.rs
- confidence: cited
  path: runbook/src/engine/layout.rs
- confidence: cited
  path: runbook/src/list.rs
- confidence: cited
  path: runbook/src/loom_run/here.rs
- confidence: cited
  path: runbook/src/serve.rs
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T01:45:49Z", actor: "human:operator", revision: 2, executor: "agent:runbook", correlation: "operator-2026-11-23-run-defaults"}
- {from: "proposed", to: "active", at: "2026-11-23T01:45:49Z", actor: "human:operator", revision: 3, executor: "agent:runbook", correlation: "operator-2026-11-23-run-defaults"}
---
## Outcome

`runbook.toml` names the directories runbooks are loaded from. `runbook list` shows every runbook
of those directories and of the built-in set, and `runbook run <name>` starts any of them. The
daemon reads the same list.

## Source

- The operator, 2026-11-23, typed in the runbook session, on `runbook run sdc`: "the next argument
  there is SDC. Which is the named protocol, I guess. So technically, when running runbook, we
  would want to load available or make available protocols, I don't know, through worktree or some
  runbook toml, and there we specify path of from where we want to load that".
- Read at `main` `566a0c3f`: `runbook run` loads a runbook from outside the repository only with
  `--bindings DIR --protocols DIR` (`runbook/src/loom_run/here.rs:125-171`); the names
  `runbook list` calls startable are fixed in code (`runbook/src/list.rs:7`); the daemon has its
  own options and layout (`--runbook-home`, `runbook/src/engine/layout.rs:25-50`).
- A consumer keeps its runbooks in its own repository (`protocol/`, `bindings/`, `views/`) and
  cannot list or start them by name.

## Acceptance

- `runbook.toml` has a list of runbook directories (absolute, or relative to the file). A directory
  holds `protocol/` and `bindings/` and may hold `views/` and `instructions/`.
- `runbook list` prints every runbook with its name, its version and the directory it comes from;
  two directories with the same name are an error that names both.
- `runbook run <name>` and `runbook serve` resolve a name through the same code; the daemon's
  `--runbook-home` and `--runbook-dir` stay as options that add to the list. The fixed list of
  startable names in `list.rs` is gone: a protocol says itself whether it can be started.
- `runbook doctor --for <name>` and `runbook check` work on a runbook of a configured directory.
- A test with a runbook directory outside the repository tree, written by hand; no model call. No
  test over 60 s.

## Scope

`runbook/src/assets.rs`, `runbook/src/list.rs`, `runbook/src/loom_run/here.rs`,
`runbook/src/engine/layout.rs`, `runbook/src/serve.rs`, `runbook/src/config.rs`, `runbook/tests/`,
`docs/`, `README.md`, `CHANGELOG.md`.
