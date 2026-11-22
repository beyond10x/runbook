---
format: aep.planning-md/3
id: story:runbook-command-knows-the-text-layout
kind: story
status: active
title: The runbook command knows the text layout, cites no line numbers and carries the repository version
summary: doctor checks bindings/, bindings/tools/, instructions/, views/ and cycles/ by name, drops line-number citations, and runbook/Cargo.toml joins the release version files
tags:
- conductor-succession
- loom
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: docs/releasing.md
- confidence: cited
  path: runbook
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T00:58:58Z", actor: "human:operator", revision: 3, executor: "agent:wave-rb2-u7", correlation: "operator-standing-acceptance-2026-11-22"}
- {from: "proposed", to: "active", at: "2026-11-22T00:58:58Z", actor: "human:operator", revision: 4, executor: "agent:wave-rb2-u7", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: runbook-command-knows-the-text-layout

## Outcome

The `runbook` command knows the directories a runbook's text lives in after the move onto Loom
(`bindings/` with its `*.yaml` files and `bindings/tools/`, `instructions/`, `views/`, `cycles/`),
checks the tools at their new place, cites no line numbers, and prints the repository version.

## Context

Wave rb2, unit rb2-u7. The command was merged at 0.1.4 before the text moved; its asset list names
`protocol/`, `scripts/`, `driver/examples/sdc/`, `bindings/tools/` and `ui/` only, its prerequisite
table cites tool lines by number (`sdc-tool.sh:23`) that are wrong since the tools moved to
`bindings/tools/`, and `runbook/Cargo.toml` is not among the release-version files, so
`runbook --version` prints 0.1.4 at 0.3.0.

## Acceptance

- `runbook doctor` on a checkout reports every directory of the text layout (`protocol/`, `scripts/`,
  `driver/examples/sdc/`, `bindings/`, `bindings/tools/`, `instructions/`, `views/`, `cycles/`, `ui/`)
  and the files `bindings/*.yaml` as present; with `instructions/` absent it prints a failing line
  named `instructions/` and exits 1 (test in `runbook/tests/cli.rs`).
- `runbook doctor` checks the tools `sdc-tool.sh`, `llm-node.sh`, `decider.sh` and
  `llm-adapter-claude-cli.sh` at `bindings/tools/`, one line each, failing when one is absent.
- `runbook doctor` cites no line number of any file (`git grep -n '\.sh:[0-9]' -- runbook/` is empty).
- `runbook --version` prints the repository version, and `task release:check VERSION=<version>` exits
  non-zero when `runbook/Cargo.toml` or the `runbook` entry of `runbook/Cargo.lock` says another
  version; `docs/releasing.md` lists both among the version files.

## Scope

`runbook/`, the `release:check` task of `Taskfile.yml`, `docs/releasing.md`, `CHANGELOG.md`. Not the
driver, `bindings/`, `scripts/` or `ui/`. A `runbook check` of the text layout is proposed in the
report of the wave, not built here.
