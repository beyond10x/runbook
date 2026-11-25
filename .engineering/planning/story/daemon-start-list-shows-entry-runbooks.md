---
format: aep.planning-md/3
id: story:daemon-start-list-shows-entry-runbooks
kind: story
status: active
title: The daemon's list of runbooks to start shows entry runbooks, not protocols opened as children
summary: GET /daemon/views/runbooks lists the protocols no other protocol opens as a child; the entry is read from data, not from a constant or a name prefix; runbook list and the start form agree.
tags:
- upstream-asks-2026-11-24
refs:
- provider: consumer
  reference: upstream-asks-filed/A7
relations:
- informed_by: story:daemon-page-starts-a-run
- informed_by: story:runbooks-load-from-configured-paths
- informed_by: story:daemon-api-specified-in-ess
scope:
- confidence: cited
  path: docs/daemon-api.md
- confidence: cited
  path: ess/domains/daemon.yaml
- confidence: inferred
  path: ess/generated
- confidence: cited
  path: runbook/src/engine/layout.rs
- confidence: cited
  path: runbook/src/list.rs
- confidence: inferred
  path: runbook/src/stub.rs
- confidence: inferred
  path: runbook/tests/daemon
- confidence: cited
  path: ui/daemon/start.js
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-25T12:41:41Z", actor: "agent:runbook", revision: 4}
- {from: "proposed", to: "active", at: "2026-11-25T12:41:41Z", actor: "agent:runbook", revision: 5}
---
## Outcome

`GET /daemon/views/runbooks` lists the runbooks a person starts, and not the protocols that another protocol
opens as its children.

## Source

Ask A7 of a consumer, filed in its story `upstream-asks-filed` (read 2026-11-24). Acceptance named there:
`GET /daemon/views/runbooks`.

## What the code does at `origin/main`

- `layout::list` in `runbook/src/engine/layout.rs` lists every `protocol/<name>.protocol.yaml` of the home
  that resolves. The SDC home therefore lists the root `sdc` and also `sdc-intake`, `sdc-direction`,
  `sdc-epic`, `sdc-spec`, `sdc-plan`, `sdc-story` and `sdc-release`, which the root protocol opens as child
  cases (`protocol/sdc.protocol.yaml`, the claim `children.passed`), beside `sdc-bugfix`, which is started on
  its own.
- `runbook list` already marks each protocol `startable`, from a constant of two names in
  `runbook/src/list.rs` (`ROOT_RUNBOOKS`: `sdc`, `sdc-bugfix`). The daemon's view does not read it.
- The view, the ESS entity `Runbook` and the start form of the page (`ui/daemon/start.js`) all read that list,
  so the start form offers the children.
- The view sits in `docs/daemon-api.md` and `ess/domains/daemon.yaml`, which say a name in the view is one a
  `SubmitRun` accepts.

## What this story does

1. The view lists the entry runbooks of the home: the protocols that no other protocol of the same home
   opens as a child.
2. How an entry is known is read from data, never from a name: the constant `ROOT_RUNBOOKS` goes, and the
   view and `runbook list` read one relation. Where the protocols carry a machine-readable child relation the
   unit reads it; where they do not, it adds one in the carrier a runbook already has (a protocol member, the
   bindings or the home's `runbook.toml`) and documents it. It does not guess from the `sdc-` prefix.
3. A protocol that is a child is still accepted by `SubmitRun` by its name (default taken; a consumer may start
   one on purpose). The view no longer lists it. The statement in the specification becomes: a name in the
   view is one a `SubmitRun` accepts, and not every name `SubmitRun` accepts is in the view.
4. The same list serves `runbook list` and the start form of the page, so the three agree.
5. Under the stub runtime the view keeps listing what the stub's scripts and `--runbook-dir` hold.

## Acceptance

With the committed SDC home and the stub or Loom runtime of the daemon tests:

- `GET /daemon/views/runbooks` lists `sdc` and `sdc-bugfix` and none of `sdc-intake`, `sdc-direction`,
  `sdc-epic`, `sdc-spec`, `sdc-plan`, `sdc-story`, `sdc-release`.
- `SubmitRun` with `runbook_id` `sdc-intake` is still accepted, as before.
- A home with two configured directories (`story:runbooks-load-from-configured-paths`) lists the entries of
  both once each, a name in the first directory hiding the same name in the second.
- A new protocol added to the home that no other protocol opens is in the view and in `runbook list` as
  `startable` with no code change (`ROOT_RUNBOOKS` no longer exists).
- The start form of the page offers the same names as the view.
- A row holds no path of the host (the existing case stays).
- `ess specify validate` and the generated OpenAPI check pass for the changed text of the view.

## Scope

- `runbook/src/engine/layout.rs`, `runbook/src/stub.rs`, `runbook/src/list.rs`, `ess/domains/daemon.yaml`,
  `ess/generated/`, `runbook/tests/daemon/`, `ui/daemon/start.js`, `docs/daemon-api.md`, `CHANGELOG.md`.

## Not in this story

- Hiding a child from `SubmitRun`.
- The view of the runs (`story:daemon-page-watches-run-directories`).
