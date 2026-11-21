---
format: aep.planning-md/3
id: story:local-gate-equals-forge
kind: story
status: proposed
title: The local gate and CI read one pinned rustc, and a red forge run fails the watch
summary: rust-toolchain.toml pins 1.99.1 for the local gate and CI; test binaries carry no full debug info; task ci:watch exits non-zero on a red run
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: .cargo/config.toml
- confidence: cited
  path: .github/workflows/pages.yml
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: rust-toolchain.toml
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T22:50:58Z", actor: "human:operator", revision: 2, executor: "agent:rb1-gate-equals-forge", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: local-gate-equals-forge

## Outcome

The local gate and the forge read one pinned rustc: `rust-toolchain.toml` pins 1.99.1, and the
pages workflow installs that version and fails when `rustc --version` does not match the pin.
Test binaries carry no full debug info (`.cargo/config.toml` sets `debug = false` for `[profile.dev]`
and `[profile.test]`). `task ci:watch SHA=<sha>` waits for the workflow runs of a commit and exits
non-zero when any run ends other than success.

Rule served (`../AGENTS.md`, "The local gate equals the forge"): "`rust-toolchain.toml` pins one
rustc; the local gate and CI both read it"; "A forge or CI run that ends red makes the push or watch
script exit non-zero".

## Acceptance

`task ci:watch SHA=<sha>` exits 0 for a commit whose runs all succeeded and non-zero for a commit with
a failed run, both shown on past runs of `beyond10x/runbook`, and a test binary of `runbook/` built
through the build pool has 0 debug_info sections.

## Scope

- `rust-toolchain.toml` (new): the pin.
- `.cargo/config.toml` (new): the test profiles without debug info.
- `.github/workflows/pages.yml`: the rust toolchain step and a guard step that compares the pin.
- `Taskfile.yml`: task `ci:watch`.
- `AGENTS.md`, `CHANGELOG.md`: one line each.

## Found, not built

Nothing. The gap this story closes is named in `story:ci-planning-only-skip` ("Found, not built").
