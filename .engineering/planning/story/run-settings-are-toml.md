---
format: aep.planning-md/3
id: story:run-settings-are-toml
kind: story
status: active
title: The settings of a run are TOML, each setting in one place
summary: runbook run reads a TOML settings file with no doubled members; sdc-experiment writes it; the records of a run directory stay JSON.
tags:
- backlog-2026-11-23
relations:
- informed_by: epic:runbook-one-binary-and-daemon
scope:
- confidence: cited
  path: docs/loom-run-settings.md
- confidence: cited
  path: driver/src/bin/sdc-experiment.rs
- confidence: cited
  path: runbook/src/loom_run/decider.rs
- confidence: cited
  path: runbook/src/loom_run/settings.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T01:45:49Z", actor: "human:operator", revision: 2, executor: "agent:runbook", correlation: "operator-2026-11-23-run-defaults"}
- {from: "proposed", to: "active", at: "2026-11-23T01:45:49Z", actor: "human:operator", revision: 3, executor: "agent:runbook", correlation: "operator-2026-11-23-run-defaults"}
---
## Outcome

The settings of a run are written in TOML, and each setting has one place. A person writes and
reads the file without a JSON nesting that has no reason.

## Source

- The operator, 2026-11-23, typed in the runbook session: "And then settings.json. I don't know, is
  there like a really good reason for having this nesting and JSON? Other than that, I would really
  prefer Tome." (TOML.)
- Read at `main` `566a0c3f`: only the runner reads the file (`runbook/src/loom_run/settings.rs:10-109`,
  `decider.rs:62`, `bindings_copy.rs:110,191`, `dry.rs:49`, `replay.rs:132`); the bound programs
  read none. One program writes it: `sdc-experiment` (`driver/src/bin/sdc-experiment.rs:2032-2060`).
  The members `confinement`, `cap_micro_usd` and `login_file` exist under `loom` and some again at
  the top level. So no reader outside the runner needs JSON.

## Acceptance

- `runbook run --settings <file>.toml` reads TOML. The members are those of
  `docs/loom-run-settings.md`; a member has one place (no second spelling under `loom`), and a
  table exists only where it groups (`[decider]`, `[models.<role>]`, `[efforts.<role>]`,
  `[replay]`). An unknown key is refused by name, as today.
- A JSON settings file is refused with a message that names the TOML form and the document.
- `sdc-experiment` writes the TOML form; its byte-for-byte test compares with a TOML fixture.
- The records a run writes stay as they are: `settings.json` (redacted, the marker of a run
  directory, `driver/src/run_kind.rs:20-30`) and `run-settings.jsonl` (`sdc.run-settings/1`). No
  reader of a run directory changes.
- `docs/loom-run-settings.md` shows the TOML form with one complete example; `CHANGELOG.md` names
  the change as breaking.
- Tests with stub programs; no model call. No test over 60 s.

## Scope

`runbook/src/loom_run/settings.rs`, `runbook/src/loom_run/decider.rs`, `runbook/Cargo.toml`,
`driver/src/bin/sdc-experiment.rs`, `runbook/tests/`, `driver/tests/`, `docs/loom-run-settings.md`,
`CHANGELOG.md`.

## Not in this story

- A file with defaults that the binary finds by itself (`story:runbook-toml-carries-defaults`).
