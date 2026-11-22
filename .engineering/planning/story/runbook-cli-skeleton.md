---
format: aep.planning-md/3
id: story:runbook-cli-skeleton
kind: story
status: proposed
title: 'runbook CLI skeleton: doctor, list, version, asset lookup'
tags:
- conductor-succession
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: runbook
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T19:35:29Z", actor: "human:operator", revision: 3, executor: "agent:runbook-cli-skeleton", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: runbook-cli-skeleton

## Outcome

A new crate `runbook/` with one binary `runbook`, the single command people type. This story builds
the skeleton, `runbook doctor`, `runbook list` and `runbook --version`; later stories add `run`,
`ui` and `handback`.

## Context

The product is named Runbook (the operator, 2026-11-21); `sdc` stays the id of the first runbook.
The binary finds its assets (`protocol/`, `scripts/`, `driver/examples/sdc/`, `ui/`) and its helper
binaries in an installed layout or in a checkout; that lookup is one module the later stories call.

## Acceptance

- `runbook doctor` run with a stripped `PATH` prints one line per missing program and exits 1; with
  every program present it exits 0. `--json` and `--for <runbook id>` work.
- `runbook list` against the repository's `protocol/` prints the nine protocols, `sdc` and
  `sdc-bugfix` marked startable, the others child protocols. `--json` prints the same.
- Asset lookup honours `RUNBOOK_HOME`, then `<exe dir>/../share/runbook`, then a checkout, in that
  order (unit tests on temporary directories).
- `runbook --version` prints the crate version; an unknown subcommand exits 64 with the usage.
- `slot task check` exits 0 with the crate included.

## Scope

New crate `runbook/` and the `check` task of `Taskfile.yml`. No `run`, `ui`, `handback`, no release
workflow, no change to `scripts/run-trial.sh` or the driver.

## Acceptance note

- Accepted as a note under `approval-record:operator-standing-acceptance-2026-11-21`.
