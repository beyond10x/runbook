---
format: aep.planning-md/3
id: story:prebuilt-release-binary
kind: story
status: active
title: Prebuilt release binary
summary: One Linux x86_64 runbook binary per release, built in CI and attached to the release by the operator's word.
tags:
- one-binary
relations:
- decomposes: epic:runbook-one-binary-and-daemon
- depends_on: story:run-in-process-and-cli-as-daemon-client
- depends_on: story:ui-embedded-in-the-binary
- depends_on: story:presets-embedded-and-runbook-test
- informed_by: story:runbook-plugin
scope:
- confidence: cited
  path: .github/workflows
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: docs/releasing.md
- confidence: cited
  path: plugin/README.md
- confidence: cited
  path: runbook/Cargo.toml
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:47:36Z", actor: "human:operator", revision: 2, executor: "agent:plan-onebin", correlation: "operator-requirement-2026-11-22-one-binary"}
- {from: "proposed", to: "active", at: "2026-11-23T13:48:40Z", actor: "human:operator", revision: 4, executor: "agent:runbook"}
---
# Story: prebuilt-release-binary

## Outcome

Each Runbook release carries one prebuilt `runbook` binary for Linux x86_64 on its GitHub Release,
built in CI from the tagged commit, with a checksum.

## Source

The operator, 2026-11-22: "What we want is one pre-built runbook binary".

## Context

Today a release is a source release: the GitHub Release carries the notes only and there is no
release workflow (`docs/releasing.md`); `runbook/Cargo.toml` is at its own version and
`task release:check` does not read it. Publishing a release is the operator's word per release; this
story builds the means and publishes nothing.

## Acceptance

- `task release:check VERSION=<version>` fails when `runbook/Cargo.toml` or its lock entry differs
  from `<version>`, and passes when they agree.
- A release workflow in `.github/workflows/` builds `runbook` for `x86_64-unknown-linux-gnu` with the
  rustc of `rust-toolchain.toml` on a tag, uploads `runbook-<version>-x86_64-linux` and its
  `sha256` file as release assets, and the step "Check the rustc pin" still passes.
- A downloaded asset of a dry-run build, with a clean `PATH` holding no `canon`, `loom`,
  `b10x-loom`, `sdc-drive`, `node`: `runbook --version` prints the version, `runbook list` prints
  the presets, and `runbook serve --ui --listen 127.0.0.1:0` answers `GET /` with 200.
- `runbookd` is the same binary: `ln -s runbook runbookd && ./runbookd --listen 127.0.0.1:0`
  behaves as `runbook serve`; the release assets include the link name in a documented install
  step.
- `docs/releasing.md` lists `runbook/Cargo.toml` among the files that carry the version and names the
  asset step; the `runbook` plugin's README names where the binary comes from.
- The workflow's classification of changed files (`task check:ci-paths`) is unchanged for
  planning-only and prose-only changes.
- Nothing is published by this story: no tag, no release (the operator's word per release).

## Scope

`.github/workflows/` (new release workflow), `Taskfile.yml` (`release:check`),
`docs/releasing.md`, `runbook/Cargo.toml`, `plugin/README.md`, `CHANGELOG.md`. Overlaps
`story:runbook-plugin` on `docs/releasing.md` and `plugin/`.
