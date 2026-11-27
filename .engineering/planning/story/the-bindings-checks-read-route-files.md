---
format: aep.planning-md/3
id: story:the-bindings-checks-read-route-files
kind: story
status: active
title: The bindings checks leave out a runbook's route entries and refuse an invalid one by name
summary: scripts/check-bindings.sh and runbook check read bindings/<id>.routes.yaml (runbook.routes/1) as a Loom bindings file and refuse it
relations:
- informed_by: story:protocol-declares-the-routes-it-needs
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: docs/loom-bindings.md
- confidence: cited
  path: driver/tests/check_bindings_exits.rs
- confidence: cited
  path: runbook/src/check.rs
- confidence: cited
  path: scripts/check-bindings.sh
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T09:56:19Z", actor: "agent:runbook", revision: 2, correlation: "operator-standing-acceptance-2026-11-21"}
- {from: "proposed", to: "active", at: "2026-11-27T09:56:19Z", actor: "agent:runbook", revision: 3, correlation: "operator-ordered-fix-2026-11-27"}
---
# The bindings checks leave out a runbook's route entries and refuse an invalid one by name

## Context

Reported on 2026-11-27 by a consumer repository that pins release 0.13.0. Since
`story:protocol-declares-the-routes-it-needs`, a runbook may carry its route entries beside its bindings, in the file
`bindings/<id>.routes.yaml` (format `runbook.routes/1`, Runbook's own file and not part of Loom's bindings;
`docs/daemon-api.md`, "The catalog"; `runbook/src/admission.rs`). The daemon reads that file at each submit of runbook
`<id>`.

Two checks of this repository read every `bindings/*.yaml` as a Loom bindings file and so refuse a valid route file:

- `scripts/check-bindings.sh --exits <root>` (the exits check a consumer runs over its own runbook, and a step of the
  full check): the route file is "not a bindings file, it is no mapping with an actions list". The registrations step
  of the full check would refuse it as "no protocol/<id>.routes.protocol.yaml".
- `runbook check` (`runbook/src/check.rs`, `bindings`, over the built-in set and over each runbook directory of
  `runbook.toml`): the route file "has no actions list".

The consumer copies the exits check and skips `*.routes.yaml` by a glob of its own; every other consumer would have to
know that glob too.

## Acceptance

1. `scripts/check-bindings.sh --exits <root>` over a root whose `bindings/` holds a valid bindings file and a valid
   `bindings/<id>.routes.yaml` (`format: runbook.routes/1`) prints `pass exits` and exits 0.
2. A file of that name that is not a route file the daemon reads (another format or none, an empty file, a link) is
   refused by the exits step by one line that names the file and says why, exit 1.
3. The budget step (`--budget <root>`) and the registrations step of the full check leave a route file out of what they
   read as bindings: no line names it.
4. `runbook check` reads a route file as the daemon reads it (`runbook::admission::read` and `parse`): a valid one is no
   finding, an invalid one is one finding under `bindings` that names the file and gives the daemon's reason.
5. `docs/loom-bindings.md` (the exits check) and the module text of `runbook/src/check.rs` say so; `CHANGELOG.md`
   `[Unreleased]` has the fix under Fixed.

## Evidence

- `scripts/check-bindings.sh:86-89`: the exits step takes every `bindings/*.yaml` as a bindings file.
- `scripts/check-bindings.sh:114`: a file with no `actions` list is refused as "not a bindings file".
- `scripts/check-bindings.sh:439-441`, `:458`: the registrations step reads every `bindings/*.yaml` and wants a protocol
  file of the same name.
- `runbook/src/check.rs:271`, `:289-295`, `:309-312`: `runbook check` reads every `bindings/*.yaml` and wants an
  `actions` list.
- A consumer repository, 2026-11-27: its copy of the exits check skips `*.routes.yaml` by a glob, because the original
  of release 0.13.0 refuses the route file of each of its runbooks that names a route.

## Scope

`scripts/check-bindings.sh`, `runbook/src/check.rs`, `driver/tests/check_bindings_exits.rs`, `docs/loom-bindings.md`,
`CHANGELOG.md`.
