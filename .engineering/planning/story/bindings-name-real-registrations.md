---
format: aep.planning-md/3
id: story:bindings-name-real-registrations
kind: story
status: active
title: The bindings files name the protocols' real registrations and their program_root
summary: 'bindings/*.yaml name <id>@<revision> and program_root: bindings; check-bindings.sh refuses a mismatch.'
relations:
- decomposes: epic:conductor-succession
- informed_by: story:loom-bindings-files
scope:
- confidence: cited
  path: .engineering/planning/story/bindings-name-real-registrations.md
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: bindings
- confidence: cited
  path: docs/loom-bindings.md
- confidence: cited
  path: scripts/check-bindings.sh
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T05:07:24Z", actor: "human:operator", revision: 3, correlation: "operator-standing-acceptance-2026-11-22"}
- {from: "proposed", to: "active", at: "2026-11-22T05:07:25Z", actor: "human:operator", revision: 4, correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: bindings-name-real-registrations

## Outcome

Every `bindings/*.yaml` names the registration Loom's catalog gives its protocol file, and a
bindings file that binds a program names its `program_root`, so Loom's tree runtime opens the
files as they are, with no rewrite of a copy.

## Context

Loom's tree runtime ran `stub-cycle` from a copy of `main` and had to rewrite the bindings files
before `tree open` accepted them. Every file wrote `protocol: <name>@1` and `from: sdc-direction@1`
on its `copy` executors; the protocol files are at higher revisions, and Loom's catalog registers a
protocol file as its id with `.` and `_` written as `-`, then `@<the document's revision>`, and
refuses another major ("registration major does not match document revision", Loom at
`af8de2c9`, `crates/loom-protocols/src/lib.rs`). No file named `program_root`; Loom's copy added
`program_root: bindings`, which makes a relative program path such as `tools/sdc-tool.sh` name a
file beside the bindings file (`crates/loom-bindings/src/decode.rs`). The design owner decided that
the bindings name the real registrations, for example `protocol: sdc@8` and
`from: sdc-direction@7`.

## Acceptance

- `scripts/check-bindings.sh` fails, naming the file and the value, when a bindings file's
  `protocol:` is not the registration of `protocol/<name>.protocol.yaml`, when a `copy` executor's
  `from:` names a registration that no protocol file has, and when a bindings file that binds a
  program has no `program_root: bindings`; it exits 0 on the nine files as they are committed.
- Each of the nine `bindings/*.yaml` names `protocol: <id with . and _ as ->@<revision>` of its
  protocol file, and each `copy` executor's `from:` names `sdc-direction@<its revision>`.
- Each of the nine bindings files that binds a program has `program_root: bindings`.
- The added check costs well under one second of the script's run time.
- `docs/loom-bindings.md` and `AGENTS.md` state that the registration moves with the protocol's
  revision and that `scripts/check-bindings.sh` refuses a mismatch.
