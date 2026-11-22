---
format: aep.planning-md/3
id: story:bugfix-any-language
kind: story
status: active
title: Bug-fix runbook on a repository of any language
summary: The suite, repro and gate commands are case data (work/checks.yaml, sdc.checks/1) from --checks, the repository's .runbook/checks.yaml or a marker-file detection table; the Rust path is unchanged.
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: docs/bugfix.md
- confidence: cited
  path: driver/examples/sdc/checks-detect.yaml
- confidence: cited
  path: driver/examples/sdc/fixtures
- confidence: cited
  path: driver/examples/sdc/tools/llm-node.sh
- confidence: cited
  path: driver/src/bin/sdc-bugfix.rs
- confidence: cited
  path: driver/tests
- confidence: cited
  path: website/docs/flow.mdx
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T20:46:43Z", actor: "human:operator", revision: 3, executor: "agent:bugfix-any-language", correlation: "operator-standing-acceptance-2026-11-21"}
- {from: "proposed", to: "active", at: "2026-11-21T20:46:43Z", actor: "human:operator", revision: 4, executor: "agent:bugfix-any-language", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: bugfix-any-language

## Outcome

The bug-fix runbook `sdc-bugfix` works on a repository of any language. The commands that run the
suite, one reproducing test and the gate are data of the case (`work/checks.yaml`, `sdc.checks/1`),
not code of the binary. The Rust path behaves as before.

## Context

The operator, 2026-11-21, on "the bug-fix runbook works on Rust repositories only today":
"UNACCEPTABLE".

## Scope

- `sdc.checks/1` in a case's `work/checks.yaml`: `test` (argv of the whole suite), `repro` (argv
  template running one test, placeholders for its file and name), `gate` (ordered named argv
  commands), `test_file` (pattern the reproducing test's path must match), optional `env`, optional
  `red` rule.
- Seed lookup, first match wins: `sdc-bugfix seed --checks <file>`; the repository's own
  `.runbook/checks.yaml`; detection by marker files from a data table shipped with the runbook
  (`driver/examples/sdc/checks-detect.yaml`); otherwise the seed fails naming `--checks`.
- `repro-install`, `repro-run`, `tests-run`, `gate-run` read `checks.yaml` and run its argv in a copy
  of `impl/` under `.sdc/bugfix/`, through `slot` when on PATH, never through a shell; a missing
  program exits 127.
- Prompts of `llm-node.sh` and the dissection schema are language-neutral and quote `checks.yaml`.
- `docs/bugfix.md`, `website/docs`, `CHANGELOG.md`.

Not in scope: a hand-back branch, a feature protocol, running Go, Python or Java toolchains in tests.

## Acceptance

- `cargo test --bin sdc-bugfix --test bugfix_prompt --test bugfix_languages` passes, failing first:
  a JavaScript fixture (`node --test`, no dependencies) and a plain executable test script behind
  `--checks`, each seeded, `repro-run` red on the unchanged code, `tests-run` passed after the fix,
  `gate-run` with a failing gate command exits 1 naming it; an unknown repository without `--checks`
  fails at seed naming `--checks`; an argv that does not exist exits 127.
- With no `checks.yaml` supplied, a Rust repository gets today's cargo commands: the existing unit
  tests and bug-fix cycles keep their results.
