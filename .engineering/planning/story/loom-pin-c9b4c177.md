---
format: aep.planning-md/3
id: story:loom-pin-c9b4c177
kind: story
status: active
title: 'Runbook links Loom c9b4c177: a not-called node is not started again in a loop; the bindings state their exits'
summary: Pin unit for the not-called fix of Loom (story:writer-not-called-leaves-the-due-set); carries the exits unit and the option for the bound per suite cycle.
tags:
- wave-2026-11-22-w1
relations:
- informed_by: story:driver-runtime-removed
- depends_on: story:bindings-state-their-exits
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: bindings
- confidence: cited
  path: cycles
- confidence: cited
  path: docs
- confidence: cited
  path: runbook/Cargo.lock
- confidence: cited
  path: runbook/Cargo.toml
- confidence: cited
  path: runbook/src/engine/refusal.rs
- confidence: cited
  path: runbook/src/loom_run
- confidence: cited
  path: runbook/tests
- confidence: cited
  path: website/data
revision: 14
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T02:59:25Z", actor: "human:operator", revision: 13, executor: "agent:runbook", correlation: "operator-2026-11-23-two-fixes"}
- {from: "proposed", to: "active", at: "2026-11-23T02:59:26Z", actor: "human:operator", revision: 14, executor: "agent:runbook", correlation: "operator-2026-11-23-two-fixes"}
---
## Outcome

The crate `runbook` links Loom at `c9b4c1777c646afdfae843dae691da36859f56ee` (branch
`wave/2026-11-21-w7` on the forge, in no tag). A model node that ends "not called" is not started
again in a loop: after a program exits a code its binding lists in `exits.not_called`, Loom does
not take that writer again until the case is evaluated again for a reason other than its own
moves. The bindings of this repository state their exits (`story:bindings-state-their-exits`), so
a not-called node and a failed tool no longer count against `budget.max_runs`.

## Source

- The operator, 2026-11-23, typed in the runbook session: "prioritize this: Two fixes for that are
  not on main yet: the Loom pin db2726ff and Loom's not-called fix."
- Run 1 of record `experiments/parity-loom-strong-4.yaml` ended `stuck`: a not-called
  `spec.author` counted against `budget.max_runs` (the record's facts).
- The Loom session, 2026-11-23: the sha is on the forge and holds
  `story:writer-not-called-leaves-the-due-set`, the bound of 60 s per cycle of a tree suite as an
  option (`tree test --cycle-timeout`, `--list`, the line of the slowest cycles) and the
  confinement of suite cycles under Substrate (`--cgroup-root`, at most 16 executions at once).
  The layout of a cycle's scratch changed: `<scratch>/work/<n>` and `<scratch>/store/<n>`.

## Acceptance

- `runbook/Cargo.toml` names the rev `c9b4c1777c646afdfae843dae691da36859f56ee` for every Loom
  crate; `runbook/Cargo.lock` holds no other Loom commit; no `[patch]`, no path, no `file://`.
- The branch holds `impl/bindings-state-their-exits` at `2492c749` (merged, not copied).
- The crate builds and `cargo clippy --locked --all-targets -- -D warnings` exits 0 in `runbook/`.
  Each new member, variant and argument of the embedder interface is handled by name
  (`DifferenceReason::CycleTimedOut`, `SuiteSettings::cycle_timeout`, the new members of
  `TreeTestArgs`); a new refusal reason has its own cause in `runbook/src/engine/refusal.rs`.
- A test with sh programs and no model holds the state of run 1: a program exits the not-called
  code while a finding waits for a person; it is started once per evaluation, no run is counted,
  and the run waits for the person instead of ending `Stuck`. The same test is red at Loom
  `db2726ff` (the program is started again until the step bound).
- `runbook test` takes `--cycle-timeout <seconds>` (Loom's range) and `--list`; `--help` says so.
- `cycles/tree-suite.yaml`: `spec-person-wait` and `cite-fault-sent-back` are run; one that is as
  expected loses `pending`; one that still fails keeps it with its difference. Every pending
  entry is run once, alone. No entry changes from `expected` to another state. The rows of
  `docs/cycles-suite.md` name Loom's story `story:writer-not-called-leaves-the-due-set`.
- Tests and fixtures that read a cycle's scratch use the new layout.
- `docs/` and `AGENTS.md` name the pinned commit; `CHANGELOG.md` has the bullets.
- The tests of the touched code pass; no test runs over 60 s.

## Scope

`runbook/` (Cargo.toml, Cargo.lock, `src/loom_run/`, `src/engine/refusal.rs`, `src/test_suite*`,
tests), `cycles/`, `bindings/`, `docs/`, `AGENTS.md`, `CHANGELOG.md`, `website/data/`.

## Not in this story

- Loom's `story:rework-allowance-after-a-rejection` (`budget.runs_per`, `rework_limit`,
  `rework_on`): its own pin after Loom's push.
- An option for `--cgroup-root` on `runbook test`.
- The measured runs.
