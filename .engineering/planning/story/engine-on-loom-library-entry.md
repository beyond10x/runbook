---
format: aep.planning-md/3
id: story:engine-on-loom-library-entry
kind: story
status: proposed
title: Engine on Loom's library entry
summary: Loom and Canon linked as pinned cargo dependencies; the runtime trait implemented by Loom in process, with no runtime binary started.
tags:
- one-binary
relations:
- decomposes: epic:runbook-one-binary-and-daemon
- depends_on: story:library-entries-from-loom-and-canon
- depends_on: story:daemon-skeleton-on-a-runtime-trait
scope:
- confidence: cited
  path: runbook/Cargo.lock
- confidence: cited
  path: runbook/Cargo.toml
- confidence: inferred
  path: runbook/src/engine
- confidence: inferred
  path: runbook/src/runtime.rs
- confidence: cited
  path: runbook/tests
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:47:32Z", actor: "human:operator", revision: 2, executor: "agent:plan-onebin", correlation: "operator-requirement-2026-11-22-one-binary"}
---
# Story: engine-on-loom-library-entry

## Outcome

The runtime trait of `story:daemon-skeleton-on-a-runtime-trait` has a second implementation that
calls Loom's library entry in process: Runbook links Loom and Canon as cargo dependencies pinned by
commit and starts, answers and cancels a run without starting a `loom`, `b10x-loom`, `canon` or
`sdc-drive` process.

## Source

The operator, 2026-11-22: "runbook should not rely on binaries [of] canon, loom. Instead, it
should just embed it as a dependency directly."

## Context

Needs the entries that `story:library-entries-from-loom-and-canon` finds or requests; the Loom
function that drives a case tree (`tree run`) as a library call is the one that blocks. The pin is
the cargo lock file: `driver/canon-pin.json` and the Canon binary resolution of
`scripts/evaluate-cases.sh` are deleted by `story:driver-runtime-removed`, and the premise of
`story:canon-0-10-0-pin` changes here. Bound programs of the bindings (`bindings/tools/`) are still
started by Loom as today.

## Acceptance

- `runbook/Cargo.toml` names Loom and Canon as git dependencies with `rev`, and
  `grep -c 'source = "git+' runbook/Cargo.lock` prints at least 2; the commit of each is the one
  `docs/library-entries.md` names.
- With `PATH` holding no `canon`, `loom`, `b10x-loom` and `sdc-drive`, `runbook run sdc --intake
  cases/<fixture>/intake.md` runs the stub cycle and exits 0 (the command itself is
  `story:run-in-process-and-cli-as-daemon-client`; here the engine test calls `runbook::engine`
  directly and asserts the root case ended `delivered`).
- During that run `ps -e -o comm` lists no `canon`, `b10x-loom`, `loom` or `sdc-drive`; the test
  samples the process tree while the run lasts.
- The Loom runtime is selected in the daemon by `runbook serve --runtime loom` and is the default;
  `--runtime stub` selects the stub; the conformance suite of
  `story:daemon-api-specified-in-ess` is green on both.
- Every record of the Loom run reaches the event sink unchanged: for the stub cycle the sink's
  records equal the records in the run's case store, record for record.
- Budget: the engine tests run the stub cycle once per test run (a cache on disk keyed by the
  digest of the protocol set and the bindings, or a setup step), no test over 60 s, the step within
  120 s; the compile time of the linked build is reported before and after.

## Scope

`runbook/src/engine/` (new), `runbook/src/runtime.rs` (one more implementation), `runbook/Cargo.toml`,
`runbook/Cargo.lock`, `runbook/tests/`. No change in `../loom` or `../canon`. Overlaps C on
`runbook/Cargo.toml` and `runtime.rs`: D starts after C.
