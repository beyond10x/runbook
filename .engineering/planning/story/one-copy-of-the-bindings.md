---
format: aep.planning-md/3
id: story:one-copy-of-the-bindings
kind: story
status: draft
title: bindings/*.yaml is the only copy of the bindings
summary: 'Found while wave 2026-11-22-w1 ran (D585): the driver/examples/sdc bindings files stayed because their readers were not moved to the YAML.'
tags:
- backlog-2026-11-22-w1
relations:
- informed_by: story:driver-runtime-removed
- depends_on: story:loom-bindings-files
revision: 1
---
## Outcome

`bindings/*.yaml` (`loom.bindings/1`) is the only copy of the bindings. The
`driver/examples/sdc/*.bindings.json` files of the deleted driver (10 at `91f7c72`, six of them
with agent actions) are gone, and every tool that
read them reads the YAML.

## Source

- Decision of the design authority in wave `2026-11-22-w1` (D585): the files stay in that
  wave, the six with agent actions name `sdc-node` on their 14 agent actions, and `scripts/check-bindings.sh` compares them
  with `bindings/*.yaml`. Moving every reader was larger than the unit (unit B,
  `impl/line3-producer-ids`, `91f7c72`).
- Files that name `bindings.json` at that head (`git grep -l`; some mean the copy in a run
  directory, the first step of the story sorts them): `scripts/check-bindings.sh`,
  `driver/src/bin/sdc-check-routes.rs`, `sdc-protocol-site.rs`, `sdc-bugfix.rs`,
  `sdc-run-metrics.rs`, `driver/src/reply/rules.rs` (`model_step_argv`), `reply/records.rs`,
  `run_metrics.rs`, `runbook/src/check.rs`, `parity.rs`, `loom_run/run_dir.rs`,
  `loom_run/decider/approval.rs`, `ui/server.mjs`, `ui/web/model.js`. The JSON files carry members the YAML does
  not have: `llm` (the instructions and the options of a model step), `results`, `child`.
- `driver/examples/sdc/rule-ends.json` (block `runtime`) stays: it is not a bindings file.

## Acceptance

- `git ls-files 'driver/examples/sdc/*.bindings.json'` prints nothing.
- No tool, task, test or script opens a file under `driver/examples/sdc/` whose name ends in
  `.bindings.json` (the copy named `bindings.json` in a run directory is another file and stays).
- `scripts/check-bindings.sh`, `sdc-check-routes` and `sdc-protocol-site check` exit 0 and read
  `bindings/*.yaml`; the "Check output" block of `docs/reject-routes.md` equals the live output.
- `website/data/protocols/*.json` is regenerated from the YAML and `sdc-protocol-site check`
  says current.
- `bindings/PENDING.md` cites `bindings/*.yaml` lines instead of the JSON files' lines.
- The part of `scripts/check-bindings.sh` that compares the two copies is removed with them.

## Scope

- `driver/examples/sdc/*.bindings.json` (deleted), `bindings/*.yaml`, `bindings/PENDING.md`.
- `scripts/check-bindings.sh`, `driver/src/bin/sdc-check-routes.rs`,
  `driver/src/bin/sdc-protocol-site.rs`, `driver/src/reply/rules.rs`, `driver/tests/`.
- `website/data/protocols/`, `docs/reject-routes.md`, `docs/driver.md`.

## Not in this story

New members of `loom.bindings/1`: where the YAML cannot carry what a reader needs, that is asked
of the Loom session first.
