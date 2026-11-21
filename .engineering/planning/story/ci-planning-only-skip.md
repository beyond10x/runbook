---
format: aep.planning-md/3
id: story:ci-planning-only-skip
kind: story
status: active
title: A planning-only change passes the required checks without building or testing
summary: Classify changed files once; planning or prose skips build and test, docs-site runs the docs check only, code runs everything
tags:
- test-budget
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: .github/workflows/b10x-docs-site.yml
- confidence: cited
  path: .github/workflows/pages.yml
- confidence: cited
  path: Taskfile.yml
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T21:47:10Z", actor: "human:operator", revision: 2, executor: "agent:ci-test-budget", correlation: "operator-standing-acceptance-2026-11-21"}
- {from: "proposed", to: "active", at: "2026-11-21T21:47:10Z", actor: "human:operator", revision: 3, executor: "agent:ci-test-budget", correlation: "ci-test-budget"}
---
# Story: ci-planning-only-skip

## Outcome

A pull request or push whose changed files are all planning or prose files passes the required
checks without building or testing. A change to the built documentation site runs the
documentation check and skips the code's own test; a change to code runs everything.

Rule served (`../AGENTS.md`, "Planning-only pull requests skip the tests"): "the required job runs,
finds only such paths, and exits 0. One path list per repository, in the workflow, with a test case
for both sides of it"; "A change to a built docs site still runs the docs check; it skips the build
and test jobs of the code"; "The ruleset ... is Timo's. Do not change it."

## Path list

The one list is the step "Classify the changed files" in `.github/workflows/pages.yml`. First
match wins:

- `.engineering/*`: planning.
- Site inputs, the docs-site class: `website/*`, and what the site build reads outside it:
  `experiments/*`, `docs/releases/*`, `docs/progress.json`, `docs/img/progress.svg`,
  `docs/check-output.txt`, `CHANGELOG.md` (`website/plugins/sdc-data.ts:242-300`,
  `driver/src/bin/sdc-protocol-site.rs:36`, `:756-775`).
- Any other `*.md`: prose, planning class.
- Everything else: code. An empty or unknown list (a new branch, a force push, a manual run) is
  code.

A list with any code file is code; else with any site input it is docs-site; else planning.

## Behaviour

- `Documentation validation` keeps its one job `Build documentation`; it always runs, lists the
  changed files (a pull request against its base, a push against the previous head) and classifies
  them. On `planning` every later step is skipped and the job exits 0. On `docs-site` the site is
  checked, built and tested, and `cargo test` of `sdc-protocol-site` is skipped. On `code` all
  steps run.
- `Documentation site` (deploy) looks for the site artifact of the validation run first. A
  planning-only push uploads none: nothing deploys, nothing fails, the deployed site stays at the
  last site change.
- `Shared source gates` calls the reusable workflow of `beyond10x/gates` (policy checks, about
  15 s, no build or test); it is not changed.

## Acceptance

- `task check:ci-paths` (run by `task check`) feeds the classifier step, extracted from
  `pages.yml`, 17 file lists: planning, docs-site, code, mixed and empty, both sides of each
  boundary; all pass in under a second.

## Found, not built

- No `rust-toolchain.toml`: CI installs `stable` (`.github/workflows/pages.yml`, rust-toolchain
  step) and the local gate uses whatever rustc is installed ("The local gate equals the forge").
- No push or watch script that exits non-zero when the forge run ends red: the push is
  `b10x-gates bot -- push`, and nothing in `scripts/` or `Taskfile.yml` waits on the CI run.
