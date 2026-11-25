---
format: aep.planning-md/3
id: story:daemon-runs-as-a-systemd-user-unit
kind: story
status: active
title: The daemon runs as an installed systemd user unit, restarted on failure, clearing old versioned exports at start
summary: The install writes a unit template with Restart=on-failure and Delegate=yes and no secret; at start the daemon removes only superseded home-<sha12> exports inside its configured exports directory.
tags:
- upstream-asks-2026-11-24
refs:
- provider: consumer
  reference: upstream-asks-filed/A8
relations:
- depends_on: story:install-script-lays-out-a-prefix
- informed_by: story:daemon-run-pins-its-runbook-at-submit
scope:
- confidence: inferred
  path: README.md
- confidence: cited
  path: runbook/src/assets.rs
- confidence: cited
  path: runbook/src/serve.rs
- confidence: inferred
  path: runbook/tests
- confidence: cited
  path: scripts/install.sh
- confidence: inferred
  path: scripts/runbook-daemon.service.in
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-25T12:41:41Z", actor: "agent:runbook", revision: 4}
- {from: "proposed", to: "active", at: "2026-11-25T12:41:41Z", actor: "agent:runbook", revision: 5}
---
## Outcome

The daemon can run as an installed systemd user unit, written from a template, restarted when it fails, and
it removes the old versioned exports of its runbook files when it starts.

## Source

Ask A8 of a consumer, filed in its story `upstream-asks-filed` (read 2026-11-24): the daemon as an installed
systemd user unit (a template), `Restart=on-failure`, removing old `home-<sha12>` exports at start. The
consumer runs the daemon every day and today starts it by hand.

## What exists

- `story:install-script-lays-out-a-prefix` (active) lays out `runbook`, its helpers and
  `<prefix>/share/runbook/` with a `MANIFEST`, and a second install removes what the first one listed.
  It writes no unit.
- `docs/library-entries.md` says the daemon's systemd unit sets `Delegate=yes` and that a daemon started by
  hand re-executes itself under `systemd-run --user --scope -p Delegate=yes`. No unit file is committed.
- Nothing at `origin/main` writes a directory named `home-<sha12>` (`git grep` over `runbook/src`,
  `scripts/`, `docs/`): those exports are made by the consumer's own wrapper around the versioned runbook
  files. The unit of this story settles who makes them before it writes any removal.

## What this story does

1. `scripts/install.sh` also writes a unit template, `<prefix>/share/runbook/runbook-daemon.service`, with the
   absolute path of the installed `runbook` filled in and `Restart=on-failure`, `RestartSec`, `Delegate=yes`
   and the daemon's start options. With `--unit-dir <dir>` it also copies the unit there; it writes into the
   user's systemd directory only when that directory is the one named.
2. The unit holds no secret. The daemon's token is read from a file the operator names by path in the unit's
   `EnvironmentFile` or credential line; the value is in no unit text, no argument and no log.
3. At each start the daemon removes superseded exports of its runbook files, and only those: the entries of
   its configured exports directory (`exports_dir`, none by default: nothing is removed without it) whose name
   is exactly `home-` and twelve lowercase hex digits and that are not the one the daemon starts with; and the
   `MANIFEST` entries of an earlier install that the current one does not list. It follows no link, stays
   inside the exports directory, never removes the directory it starts with, and prints what it removed.
4. `docs/` says how to install the unit, start it with `systemctl --user`, and read its log.

## What a guard must hold

- A name that is not exactly the pattern is never removed; a link named like the pattern is removed as a link
  and never followed; a directory outside `exports_dir` is never touched.
- The current export and a run's own pinned copy (`story:daemon-run-pins-its-runbook-at-submit`) are never
  removed while a run uses them.

## Acceptance

- After an install into a directory under `target/`, the template exists, parses as a unit file and holds
  `Restart=on-failure`, `Delegate=yes` and an `ExecStart` that names the installed binary by absolute path; no
  token value is in it.
- With `--unit-dir <dir>` the unit is copied into that directory and nowhere else; without it nothing is
  written outside the prefix.
- A start with an exports directory holding `home-aaaaaaaaaaaa`, `home-bbbbbbbbbbbb` (the current one), `home-x`,
  a link named `home-cccccccccccc` and a file `notes` removes `home-aaaaaaaaaaaa` and the link (not its
  target), and leaves the other three; with no `exports_dir` it removes nothing.
- A second install lists no file that the first listed and the second does not (`MANIFEST` rule of
  `story:install-script-lays-out-a-prefix`).
- Where `systemd-analyze` exists the template passes `systemd-analyze --user verify`; the case is part of the
  slow lane, and the fast lane holds the text checks above. No test starts a real unit.

## Scope

- `scripts/install.sh`, a new `scripts/runbook-daemon.service.in`, `runbook/src/serve.rs`,
  `runbook/src/assets.rs`, `runbook/tests/`, `docs/`, `README.md`, `CHANGELOG.md`.

## Not in this story

- Resume of runs that were live when the daemon was killed (`story:resume-cut-step`).
- A system-wide unit.
