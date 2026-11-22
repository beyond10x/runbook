---
format: aep.planning-md/3
id: story:experiment-runner-loom
kind: story
status: active
title: 'Experiment runner loom: sdc-experiment starts a model run through Loom'
summary: 'runner: loom in sdc.experiment/1, its launch, status, stop and report'
tags:
- experiment
- loom
relations:
- decomposes: epic:runbook-one-binary-and-daemon
- depends_on: story:parity-run-on-loom
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: docs/improving.md
- confidence: cited
  path: docs/workflow-metrics.md
- confidence: cited
  path: driver/src/bin/sdc-experiment.rs
- confidence: cited
  path: driver/tests/experiment_loom.rs
- confidence: cited
  path: driver/tests/main.rs
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T06:21:40Z", actor: "human:operator", revision: 8, executor: "agent:experiment-runner-loom", correlation: "operator-standing-acceptance-2026-11-22"}
- {from: "proposed", to: "active", at: "2026-11-22T06:21:40Z", actor: "human:operator", revision: 9, executor: "agent:experiment-runner-loom", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: experiment-runner-loom

## Outcome

`sdc-experiment` learns `runner: loom`: a registered experiment record starts a model run of Runbook
through Loom (`runbook loom-run`, built by another unit) instead of the driver's `run-trial.sh`, and
the same record is checked, reported, listed and stopped like a driver or free-agent record. This is
the launcher half of the evaluation run on Loom; the run directory it reads back is written by the
runner.

## Context

`story:parity-run-on-loom` § "Acceptance of the evaluation run": "The runs start only through
`sdc-experiment run` with a registered record". Before this story the binary knows `runner: sdc` and
`runner: free-agent`; every launch setting becomes an `SDC_*` variable of `run-trial.sh`. A Loom run
has no such variables: its settings are a file, its budget a cap over all runs of the record, and
`run_metrics::load` reads a run directory in the driver's layout (`docs/workflow-metrics.md`). The
file set that `report` needs from a run directory is written down in `docs/workflow-metrics.md`
§ "Run directories of a `loom` run" and is the contract of the unit that writes it. The login file
of a model run is named by path only; the launcher never reads, copies or prints it.

## Acceptance

- The record format `sdc.experiment/1` accepts `runner: loom` beside `sdc` and `free-agent`. A
  prospective `loom` variant that does not set `loom.bin`, `loom.sha256`, `loom.confinement`,
  `loom.cap_micro_usd`, the four `models.<class>.model`, `decider.model`, `max_llm_calls` or
  `max_wall_seconds` is refused by the name of the missing setting; a setting only the old driver
  understands (`decider.kind`, `adapter`, `autonomy`, the switches, efforts) is refused for a `loom`
  variant by its name, and `docs/workflow-metrics.md` lists the choice made for each setting (tests
  in `driver/tests/experiment_loom.rs` module of the integration binary).
- `sdc-experiment run <record> --print` for a `loom` record prints, per run, the unit name and the
  exact `runbook loom-run ...` command with the root protocol derived from the record's tree
  (`protocol/sdc.protocol.yaml`, not hard-coded), the idea file, the settings file, the spend ledger
  `<runs-dir>/<record id>.loom-spend.json` and the caps; the build step of a `loom` record builds the
  `runbook` crate of the record's `tree_commit` as its own job.
- A `loom` run starts through `systemd-run --user ... env -i` with the base environment of the other
  runners and no `SDC_*` variable; a test asserts the printed environment. The login file is named by
  path in the settings file only when `loom.login_file` is set; `run --print` and the unit's
  description print the setting's name and never its value (test with a marker path).
- `sdc-experiment status` and `stop` map the unit names of `loom` runs as for the other runners;
  `stop` stops the unit (SIGTERM to `runbook loom-run`) and does not run `run-trial.sh --stop`.
- `sdc-experiment check` accepts a planned `loom` record without runs and `run` refuses a `loom`
  record whose `tree_commit` has no `runbook/` crate, with a message naming the commit.
- `sdc-experiment report` of a `loom` record reads its run directories through
  `run_metrics::load`; `docs/workflow-metrics.md` holds the table of files and members `load` and the
  clean-pass rule require, and a test feeds `report` a run directory holding only those members and
  gets a report with `run.delivered` and `run.cost_usd`.
- An example `loom` record is in the docs and in a test fixture, and no record is added to
  `experiments/`.
