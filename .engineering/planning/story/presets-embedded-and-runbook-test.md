---
format: aep.planning-md/3
id: story:presets-embedded-and-runbook-test
kind: story
status: proposed
title: Presets embedded in the binary and runbook test
summary: The repository's runbooks embedded as presets listed with version and measured quality; --runbook-dir; runbook test through Loom's suite runner.
tags:
- one-binary
relations:
- decomposes: epic:runbook-one-binary-and-daemon
- depends_on: story:engine-on-loom-library-entry
- depends_on: story:run-in-process-and-cli-as-daemon-client
scope:
- confidence: cited
  path: Taskfile.yml
- confidence: inferred
  path: runbook/build.rs
- confidence: cited
  path: runbook/src/assets.rs
- confidence: cited
  path: runbook/src/list.rs
- confidence: inferred
  path: runbook/src/presets.rs
- confidence: inferred
  path: runbook/src/test_cmd.rs
- confidence: cited
  path: runbook/tests
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:47:36Z", actor: "human:operator", revision: 2, executor: "agent:plan-onebin", correlation: "operator-requirement-2026-11-22-one-binary"}
---
# Story: presets-embedded-and-runbook-test

## Outcome

The runbooks of this repository are embedded in the binary as its presets, `runbook list` shows each
with its version and measured quality, a runbook from outside the binary runs with
`--runbook-dir`, and `runbook test` runs a cycles suite through Loom's suite runner.

## Source

The operator, 2026-11-22: "it comes with protocol presets which are optimized and self-improved
through the evals."

## Context

A preset is a runbook: protocols (`protocol/`), bindings (`bindings/`), instructions
(`instructions/`), views (`views/`) and cycles (`cycles/`). It changes only through a release with a
registered hypothesis and measured runs (`docs/improving.md`, `experiments/releases.yaml`).
`runbook list` exists as a skeleton (`story:runbook-cli-skeleton`) and looks assets up in a
checkout or an installed layout (`RUNBOOK_HOME`); the embedded set becomes the first source. The
done test of the epic is `runbook test cycles/suite.yaml`.

## Acceptance

- Started from an empty directory with no `RUNBOOK_HOME`, `runbook list` prints `sdc` and
  `sdc-bugfix` as startable presets, each with its version and its measured quality (the figure of
  its row in `experiments/releases.yaml`), and `--json` prints the same.
- `runbook list --runbook-dir <dir>` lists the runbooks of `<dir>` instead, and `runbook run
  --runbook-dir <dir> <id>` runs one of them; a directory that is not a runbook exits 1 with the
  failing file named.
- A check fails when an embedded file differs from its source in the repository: `task check`
  prints the list of embedded paths and a digest, and exits 1 after one byte of `protocol/sdc.protocol.yaml`
  changes without a rebuild.
- `runbook test cycles/suite.yaml` runs every entry through Loom's suite runner linked in, prints one
  line per entry with `expected` or `unexpected`, and exits 0 when every entry is as expected;
  with the suite's `stub-cycle` entry changed to an expectation that does not hold, it exits 1
  and names the entry.
- `runbook test` starts no `scripts/run-finding-cycles.sh`, `canon` or `sdc-drive` process.
- The step that runs the suite ends within 120 s, no test over 60 s; the suite's cycles run once per
  test run (`AGENTS.md`, "Tests stay fast"); any entry over 5 s is named with its time.

## Scope

`runbook/src/presets.rs`, `runbook/src/test_cmd.rs` (new), `runbook/src/list.rs`, `runbook/src/assets.rs`,
`runbook/build.rs`, `runbook/tests/`, `cycles/suite.yaml` (read), `Taskfile.yml`, `docs/`.
No change to a protocol's text.
