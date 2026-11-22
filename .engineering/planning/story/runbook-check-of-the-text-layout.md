---
format: aep.planning-md/3
id: story:runbook-check-of-the-text-layout
kind: story
status: active
title: runbook check reads the text layout of a checkout
summary: A runbook check subcommand checks layout, bindings, instruction files, views and the cycles list, and replaces the shell of check:cycles-suite
tags:
- conductor-succession
- loom
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: docs/cycles-suite.md
- confidence: cited
  path: runbook
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:11:34Z", actor: "human:operator", revision: 2, executor: "agent:wave-rb2-u8", correlation: "operator-standing-acceptance-2026-11-22"}
- {from: "proposed", to: "active", at: "2026-11-22T01:11:34Z", actor: "human:operator", revision: 3, executor: "agent:wave-rb2-u8", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: runbook-check-of-the-text-layout

## Outcome

`runbook check` reads the text of a checkout (protocols, the two forms of bindings, instruction
files, views, the cycles list) and prints one finding per inconsistency: the file, what is wrong and
the fix. It is Rust, reads no shell, and replaces the `grep -Pzo` of `task check:cycles-suite`.

## Context

Wave rb2, unit rb2-u8. The text of a runbook was checked by shell: `scripts/check-bindings.sh` and the
`Taskfile.yml` task `check:cycles-suite`, which compared `cycles/suite.yaml` with the arrays of
`scripts/run-finding-cycles.sh` by id and state only, with a Perl regular expression that is not
portable. Tests of the `runbook` command run on scratch copies of the layout.

## Acceptance

- `runbook check` on this checkout exits 0 and prints `check: ok (...)`; on a clean scratch copy of the
  text layout it does the same, and `--json` prints `ok: true` and no findings (tests in
  `runbook/tests/cli.rs`).
- A part of the text layout that is missing or not executable (`LAYOUT` of `runbook/src/assets.rs`) is a
  finding that names it, and the command exits 1.
- A binding that names a missing instruction file, or a path with `..`, is a finding that names the
  binding file; two forms that name different files for one action are a finding that names the YAML
  file; a program that is not an executable `bindings/tools/<file>` is a finding.
- An instruction file with a placeholder that is not `{{[a-z_]+}}` is a finding that names the file, and
  so is an instruction file that no binding, tool or other instruction file names.
- A view file whose `format` is not `runbook.view/1`, or that is no JSON object, is a finding that
  names the view file.
- A duplicate cycle id, a state outside `expected`, `pending` and `unlisted`, a fixture that does not
  exist, and a state or id that differs from the `expected` and `pending` arrays of
  `scripts/run-finding-cycles.sh` are findings that name `cycles/suite.yaml`; without the script the
  comparison is skipped with one note.
- `task check:cycles-suite` runs `runbook check` and exits 0 on this checkout, and exits non-zero when
  `RUNBOOK_HOME` names a copy with one cycle's state changed; `git grep -n 'grep -P' -- Taskfile.yml`
  is empty.

## Scope

`runbook/`, the task `check:cycles-suite` of `Taskfile.yml`, `docs/cycles-suite.md`, `AGENTS.md` and
`CHANGELOG.md`. `scripts/check-bindings.sh` keeps all its checks.
