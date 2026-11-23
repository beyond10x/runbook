---
format: aep.planning-md/3
id: story:runbook-toml-carries-defaults
kind: story
status: active
title: 'runbook run starts with a runbook and an intake: runbook.toml carries the defaults'
summary: A runbook.toml found by the binary states defaults for every option and setting of a run; the confinement has no built-in default, and caps are optional.
tags:
- backlog-2026-11-23
relations:
- informed_by: epic:runbook-one-binary-and-daemon
- depends_on: story:run-settings-are-toml
scope:
- confidence: cited
  path: README.md
- confidence: cited
  path: docs/loom-run.md
- confidence: cited
  path: runbook/src/client
- confidence: cited
  path: runbook/src/config.rs
- confidence: cited
  path: runbook/src/loom_run/here.rs
- confidence: cited
  path: runbook/src/loom_run/settings.rs
- confidence: cited
  path: runbook/src/main.rs
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T01:45:49Z", actor: "human:operator", revision: 2, executor: "agent:runbook", correlation: "operator-2026-11-23-run-defaults"}
- {from: "proposed", to: "active", at: "2026-11-23T01:45:49Z", actor: "human:operator", revision: 3, executor: "agent:runbook", correlation: "operator-2026-11-23-run-defaults"}
---
## Outcome

`runbook run <runbook> --intake <file>` starts a run. Everything else a run takes has a default in
a file `runbook.toml` that the binary finds by itself: the directory of the runs, the name of the
run, the spend ledger, the caps, the confinement, the autonomy level, the models, the decider, the
efforts. An option on the command line still wins.

## Source

- The operator, 2026-11-23, typed in the runbook session: "And runs dir and run. So it would be
  cool if there would be, you know, some defaults. Default parameters for these things, because
  like this it's just an insane wall of arguments to specify to get something running." And: "then
  also can have these defaults for, I don't know, LLM settings, run dir, and so on, confinement,
  level of approval, and so on. Anything basically."
- Read at `main` `566a0c3f`: `runbook run` needs ten arguments (`runbook/src/client/mod.rs:44-65`,
  `runbook/src/loom_run/here.rs:22-106`): the runbook, `--intake`, `--run`, `--runs-dir`,
  `--confinement`, `--settings`, `--spend-ledger`, `--cap-micro-usd`, `--max-llm-calls`,
  `--max-wall-seconds`. The binary reads no configuration file (`runbook/Cargo.toml` has no TOML
  crate).

## Acceptance

- The binary reads `runbook.toml` from the first place that has one: `--config <file>`, the
  working directory and its parents, then the user's configuration directory. `runbook config`
  prints every effective value with the place it came from (option, file with path, built-in).
- The file can state every option of `runbook run` and every member of the run settings
  (`story:run-settings-are-toml`): no `--settings` file is needed when the file states them.
- Built-in defaults exist only where a wrong value does no harm: the directory of the runs
  (`runs/` beside the file), the name of the run (generated, unique, printed), the spend ledger
  (in the directory of the runs).
- No built-in default for the confinement and the wall time: the file or an option states them, and
  a run without them is refused with the name of the missing value and the line to add. A value of
  the file never lowers a confinement that an option or the daemon's floor sets.
- The spend cap, the call cap and the spend ledger are optional (`story:caps-are-optional`): none
  named, in an option or in the file, means no limit; a named cap stops the run when reached.
- A login file is named by path in the file, never by content; the file is refused when it holds a
  token or a key (the names the push scan knows).
- With a file that states the four values, `runbook run sdc --intake idea.txt --dry` starts a dry
  run. `README.md` shows that command and a starter `runbook.toml`; `runbook init` writes the
  starter.
- Tests with stub programs; no model call. No test over 60 s.

## Scope

`runbook/src/main.rs`, `runbook/src/client/`, `runbook/src/loom_run/here.rs`,
`runbook/src/loom_run/settings.rs`, a new `runbook/src/config.rs`, `runbook/Cargo.toml`,
`runbook/tests/`, `README.md`, `docs/loom-run.md`, `docs/loom-run-settings.md`, `CHANGELOG.md`.

## Not in this story

- Where runbooks are loaded from (`story:runbooks-load-from-configured-paths`).
- Choosing a runbook for an input (`story:router-picks-a-runbook-for-an-input`).
