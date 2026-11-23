---
format: aep.planning-md/3
id: story:loom-pin-db2726ff
kind: story
status: active
title: 'Runbook links Loom db2726ff: actions dated by their own step, cycles beside each other, the stop, approve and replay steps'
summary: 'Pin unit after wave 2026-11-22-w1: the Loom commit with the fix for the stuck run 2 of record parity-loom-strong-4.'
relations:
- informed_by: story:driver-runtime-removed
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: cycles
- confidence: cited
  path: docs
- confidence: cited
  path: driver
- confidence: cited
  path: runbook
- confidence: cited
  path: ui
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T23:10:41Z", actor: "human:operator", revision: 8, executor: "agent:runbook", correlation: "pin-db2726ff"}
- {from: "proposed", to: "active", at: "2026-11-22T23:10:42Z", actor: "human:operator", revision: 9, executor: "agent:runbook", correlation: "pin-db2726ff"}
---
## Outcome

The crate `runbook` links Loom at `db2726ff78440b8187cf956dd84e0847986e7e96` (branch
`wave/2026-11-21-w7` on the forge, 81 commits over `70b0e6e0`). A run no longer ends `Stuck`
because an action is dated by the next `evaluated` line: Loom dates an action by the revision of
its step and the revision its step left. `runbook test` can run the cycles of a tree suite beside
each other, and the entries of `cycles/tree-suite.yaml` that waited for Loom's stop, approve and
replay steps run.

## Source

- The operator's goal for this repository: a case runs on Loom, and the measured runs deliver.
  Run 2 of record `experiments/parity-loom-strong-4.yaml` stopped `stuck` on the dating rule of
  Loom `70b0e6e0` (the record's facts).
- The Loom session, 2026-11-22: the sha is on the forge; it holds
  `story:action-dated-by-revisions-its-step-left`, `story:tree-replay-command`,
  `story:tree-suite-stop-resume-reopen-steps`, the approve entries with `result`, `reasons` and
  `times`, cycles in parallel (`SuiteSettings.workers`), the signalled-program refusal, the
  terminated refusals, the hold of a cgroup root, ESS 0.97.4, Canon 0.11.0 and Substrate 0.24.0.

## Acceptance

- `runbook/Cargo.toml` names the rev `db2726ff78440b8187cf956dd84e0847986e7e96` for every Loom
  crate; `runbook/Cargo.lock` holds no other Loom commit; no `[patch]`, no path, no `file://`.
- The crate builds and `cargo clippy --locked --all-targets -- -D warnings` exits 0 in `runbook/`.
  Each new member, variant and argument of the embedder interface is handled by name: a new
  refusal (`Terminated`, `ProgramSignalled`, `PersonProgramFailed`, `ConfinementRefused`, a held
  cgroup root) reaches the run's outcome, the exit code and `run-stopped.json` as a named cause,
  never as a panic or a catch-all text; the run-log kinds `StopRecorded` and `StopLifted` are read
  by every reader of a run log (the inspector's server and the daemon included) without an error.
- A test, with no model call, holds the state of run 2: an action runs, a second action writes,
  one `evaluated` line follows, a check exits non-zero and the first action's handoff check
  passes; the run takes the first action again and does not end `Stuck`.
- `runbook test <suite>` has an option for the number of cycles run beside each other (Loom's
  `workers`), default 1, refused above 1 under a confinement Loom does not serve it for, and
  `--only` if Loom's `only` member serves it; `runbook test --help` says so.
- `cycles/tree-suite.yaml`: `run-stop` checks the `StopRecorded` log line; the entries that named
  a Loom story that is merged (`replay-sibling`, `decider-rework`, `decider-rework-spent`,
  `rework-fresh-allowance`, `run-stop`) are run; one that is as expected loses `pending`, one that
  still fails keeps `pending` with its difference and the Loom story or the gap it waits for,
  under the name Loom's store has. No entry changes from `expected` to another state.
- `docs/` says the pinned commit and what changed for a user (`docs/loom-run.md`,
  `docs/cycles-suite.md`, the page that lists the pins); `CHANGELOG.md` has the bullets.
- The tests of the touched code pass; no test runs over 60 s.

## Scope

`runbook/` (Cargo.toml, Cargo.lock, `src/loom_run/`, `src/test_suite*`, tests), `cycles/`,
`docs/`, `CHANGELOG.md`, `ui/` and `driver/` only where a reader of a run log needs the two new
kinds.

## Not in this story

- A settable bound per cycle: Loom's 60 s per cycle stays a constant.
- New paid runs.
- The exits of the bindings (`story:bindings-state-their-exits`).
