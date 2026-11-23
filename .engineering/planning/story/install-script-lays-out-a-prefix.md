---
format: aep.planning-md/3
id: story:install-script-lays-out-a-prefix
kind: story
status: active
title: scripts/install.sh --prefix lays out runbook, its helpers and its files
summary: One command builds the binary and the programs it starts and writes them under one prefix; the installed runbook runs from any directory without the checkout.
tags:
- backlog-2026-11-23
relations:
- informed_by: story:prebuilt-release-binary
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: README.md
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: docs/releasing.md
- confidence: cited
  path: runbook/src/assets.rs
- confidence: cited
  path: runbook/src/main.rs
- confidence: cited
  path: runbook/tests
- confidence: cited
  path: scripts/install.sh
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T03:08:20Z", actor: "human:operator", revision: 2, executor: "agent:runbook", correlation: "operator-2026-11-23-consumer-gaps"}
- {from: "proposed", to: "active", at: "2026-11-23T03:08:20Z", actor: "human:operator", revision: 3, executor: "agent:runbook", correlation: "operator-2026-11-23-consumer-gaps"}
---
## Outcome

A person who has a checkout of a release gets a `runbook` they can put on `PATH` with one command:
`scripts/install.sh --prefix <dir>` builds the binary and the programs it starts and lays them out
under one prefix. `<dir>/bin/runbook` then runs from any directory without the checkout.

## Source

- The consumer session of the operator's own runbooks, 2026-11-23, on the operator's order that its
  gaps go first: it has no installed runbook and runs 0.5.0 from another session's scratch
  directory; "I need a release on PATH (0.6.0 or later)".
- Read at `main` `566a0c3f`: release 0.6.0 holds no binary asset; `README.md` ("Try it") tells a
  reader to build two crates and to start `runbook/target/debug/runbook`; the binary finds its
  files under `RUNBOOK_HOME`, then `<exe>/../share/runbook`, then the checkout
  (`runbook/src/assets.rs:4-9,135-155`), and finds a helper program beside the executable before
  `PATH` (`runbook/src/assets.rs:158-171`).

## Acceptance

- `scripts/install.sh --prefix <dir>` (POSIX sh) builds with `--locked` in release profile and
  writes `<dir>/bin/runbook`, the helper programs `runbook` starts (the binaries of `driver/` the
  bindings name, `sdc-watch`), and `<dir>/share/runbook/` with the protocols, bindings, tools,
  instructions, views and the files of the page that the binary reads. It writes nowhere else,
  refuses a prefix that is not an absolute path, and never writes into `~/.cargo/bin`.
- `<dir>/share/runbook/VERSION` holds the version and the commit; `runbook --version` prints both.
- A second run over the same prefix replaces the files of the first (no stale helper stays: the
  script removes what an earlier install listed in `<dir>/share/runbook/MANIFEST` and is gone now).
- A test installs into a directory under `target/`, moves to an empty working directory with a
  `PATH` of the prefix's `bin` and the system directories only, and runs `runbook list`,
  `runbook doctor` and `runbook run sdc --dry --intake <file> …`: the dry run ends as it does from
  the checkout (same `outcome.json` but for times and paths). No model call. The build of the test
  is made once per run, not per test; the test is in the slow lane if it runs over 60 s, with the
  fast lane holding the layout check on a prefix made from prebuilt files.
- `README.md` shows the command and the three lines to put `<dir>/bin` on `PATH`; `docs/releasing.md`
  names the script as the source of the release asset of `story:prebuilt-release-binary`.

## Scope

A new `scripts/install.sh`, `runbook/src/assets.rs`, `runbook/src/main.rs`, `runbook/tests/`,
`Taskfile.yml`, `README.md`, `docs/releasing.md`, `CHANGELOG.md`.

## Not in this story

- The release asset built in CI (`story:prebuilt-release-binary`).
- Installing into a user's `PATH`: the person who runs the script names the prefix.
