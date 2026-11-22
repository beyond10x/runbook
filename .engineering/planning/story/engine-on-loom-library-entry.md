---
format: aep.planning-md/3
id: story:engine-on-loom-library-entry
kind: story
status: active
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
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:47:32Z", actor: "human:operator", revision: 2, executor: "agent:plan-onebin", correlation: "operator-requirement-2026-11-22-one-binary"}
- {from: "proposed", to: "active", at: "2026-11-22T10:08:39Z", actor: "human:operator", revision: 5, executor: "agent:onebin-d-engine", correlation: "operator-standing-acceptance-2026-11-22"}
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

## Slice 1

Delivered by `agent:onebin-d-engine` on `impl/engine-on-loom-library-entry` (2026-11-22).

- Built: `runbook/src/engine/` (the `Runtime` over Loom's library at `5af3f9c2`), `runbook serve
  --runtime loom|stub` and `--runbook-home`, eight gate-lane tests (`runbook/tests/daemon/engine.rs`,
  fixture `runbook/tests/fixtures/engine-runbook/`), the slow lane `task test:engine-real`,
  `docs/engine.md`, "States at `5af3f9c2`" in `docs/library-entries.md`.
- Acceptance met: Loom as git dependencies with `rev` (Canon through Loom, no direct dependency);
  records equal record for record (`every_run_log_line_reaches_the_record_unchanged`); process
  sample with no `canon`, `b10x-loom`, `loom` or `sdc-drive`; the cold `cargo build --locked` of `runbook/`
  went from 90 s (77 crates compiled, 90 lockfile packages) to 226 s (208 compiled, 258 packages).
- Not met as written: the Loom runtime is not the default of `runbook serve` (the coordinator held
  `stub` as the default until the daemon's Host and Origin check exists); the stub cycle runs once
  per test run in the slow lane only, not in the gate lane (the gate lane runs a two-level fixture
  runbook).
- Loom gaps: a running bound program is not stopped by a cancel (`run.rs:166`, row L15); a person
  action's principal is not in `NextDecision` (`person.rs:65`).
- Not built: resume of an orphaned run from its store; the `linux` level under test.

## Slice 2

Delivered by `agent:onebin-e-inproc` on `impl/loom-run-in-process` (2026-11-22). Goal story moved:
`engine-on-loom-library-entry` (the run loop over Loom as a library, one copy of the loop).

- Built: `runbook loom-run --in-process`. The loop (`runbook/src/loom_run/run_loop.rs`), the decider,
  the budget, the ledger, the settings, the bindings copy, the private directory and the run
  directory files exist once; they reach Loom through the trait `LoomHost`
  (`runbook/src/loom_run/host.rs`) with a child implementation (`child_host.rs`, behaviour and tests
  unchanged) and a library implementation (`library.rs`). The Loom-calling code is one module,
  `runbook/src/engine/calls.rs`, used by the daemon's engine and by `loom-run`. Exactly one of
  `--in-process` and `--loom` with `--loom-sha256` (usage error, exit 2); `--confinement` stays
  required; a level the library cannot give is refused before anything is written. Decisions are
  answered from structured members. `loom_exit_code` and `stderr_tail` are `null` under the library;
  the run records the pinned commit (`build.rs` reads it from `Cargo.lock`). `sdc-experiment`
  accepts `loom.in_process: 'true'` and refuses a record that names both ways.
- Tests: nine gate-lane tests in `runbook/tests/loom_run/inproc.rs` (failing at the base commit:
  `unexpected argument '--in-process'`), two in `driver/tests/experiment_loom.rs`, the slow lane
  `task test:loom-in-process`: the `sdc` stub cycle through the child and through `--in-process`,
  `parity: identical`.
- Loom gaps: `NextDecision` has no principal (`loom-tree/src/person.rs:65`; it is only inside the
  printed `command`, `loom-tree/src/run.rs:886-899`) and `DEFAULT_PRINCIPAL` is not re-exported
  (`loom-tree/src/run.rs:70`, `lib.rs:70`); a cancel and the wall deadline end no running bound
  program (`loom-tree/src/run.rs:166`); `b10x-loom-tree` re-exports neither `SubstrateRunner` nor
  `ProtocolCatalog` (`loom-tree/src/lib.rs:48-75`). The environment of a bound program is `PATH`
  and `LOOM_*` only (`loom-bindings/src/child.rs`): no gap.
- Not built: removal of `child.rs` and `--loom`; the daemon's default runtime, the guard, `ui/`;
  Loom's suite runner and presets.
