---
format: aep.planning-md/3
id: story:daemon-default-confinement-is-stated
kind: story
status: active
title: A daemon with a stated default confinement starts a submit that names none
summary: The daemon takes --default-confinement or the value of its runbook.toml; a submit without a level runs at it. No built-in default; the floor is not read as one.
relations:
- informed_by: story:runbook-toml-carries-defaults
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T10:20:13Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
- {from: "proposed", to: "active", at: "2026-11-23T15:29:53Z", actor: "human:operator", revision: 3, executor: "agent:runbook"}
---
## Outcome

A daemon whose operator stated a default confinement starts a submit that names none at that level. The
level is stated by the person who starts the daemon: the option `--default-confinement <level>`, or the
`confinement` value of the `runbook.toml` the daemon reads.

## Source

The consumer's fourth gap after its first run through the daemon (2026-11-23): every submit must name
`confinement`, also against a daemon started with `--min-confinement local`. The operator, 2026-11-23, on
defaults from a file: "confinement, level of approval, and so on. Anything basically."

The standing rule stays: the binary has no built-in default for the confinement. The floor
(`--min-confinement`) is a lower bound and is not read as a default.

## Scope

- `ess/`: the daemon's specification: the start of a run without a level.
- `runbook/src/main.rs`, `runbook/src/serve.rs`, `runbook/src/engine/`: the option and its use.
- `runbook/src/mcp/schema.rs`: `confinement` of `submit_run` becomes optional.
- `runbook/tests/`, `docs/engine.md`, `docs/mcp.md`, `CHANGELOG.md`.

## Acceptance

- A daemon started with `--default-confinement L` starts a submit without `confinement` at L, and the run's
  record names L and that it came from the daemon's default.
- A daemon started without the option and without a file value refuses a submit without `confinement` by
  name, as today.
- A daemon whose default is below its floor does not start, and says which two values disagree.
- A submit that names a level is unchanged: at or above the floor it runs at the named level, below it the
  daemon refuses and writes nothing.
- The start form of the daemon's page still presets no level.
- A security review pass covers the change before it merges.

## Not in this story

- The `runbook.toml` reader: `story:runbook-toml-carries-defaults` delivers the file value; this story
  delivers the option and the rule.
