---
format: aep.planning-md/3
id: story:loom-pin-f39f19b6
kind: story
status: active
title: 'Pin Loom f39f19b6: a rejection gives the action a new allowance of runs'
summary: Loom f39f19b6 counts runs per rejection (budget.runs_per, rework_limit, rework_on); change.implement takes them, the two rework entries of the tree suite lose pending.
tags:
- backlog-2026-11-23
relations:
- depends_on: story:loom-pin-c9b4c177
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
  path: runbook/src/check.rs
- confidence: cited
  path: runbook/src/engine
- confidence: cited
  path: runbook/src/loom_run
- confidence: cited
  path: runbook/tests
- confidence: cited
  path: scripts/check-bindings.sh
- confidence: cited
  path: website/data
revision: 16
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T03:32:04Z", actor: "human:operator", revision: 2, executor: "agent:runbook", correlation: "operator-2026-11-23-two-fixes"}
- {from: "proposed", to: "active", at: "2026-11-23T03:32:04Z", actor: "human:operator", revision: 3, executor: "agent:runbook", correlation: "operator-2026-11-23-two-fixes"}
---
## Outcome

The crate `runbook` links Loom at `f39f19b62aa84d8220d05bbdafe7df5ccd1af4bb` (branch
`wave/2026-11-21-w7` on the forge, in no tag). A rejection gives the action that is sent back a new
allowance of runs: `change.implement` counts its runs per rejection, and the number of rejections
it may answer is bounded by a limit of its own. The suite entries `decider-rework-spent` and
`rework-fresh-allowance` are as expected and lose `pending`.

## Source

- The operator, 2026-11-23, typed in the runbook session: "prioritize this: Two fixes for that are
  not on main yet: the Loom pin db2726ff and Loom's not-called fix." The rework allowance is the
  third cause of the stuck runs of record `experiments/parity-loom-strong-4.yaml`; the three
  measured runs follow this pin.
- The Loom session, 2026-11-23: the sha holds `story:rework-allowance-after-a-rejection`
  (`budget.runs_per: case | rejection`, default `case`; `budget.rework_limit`, default 2;
  `budget.rework_on`, a map of evidence kind to results like `retry_when`), the second part of the
  not-called fix (a set-aside not-called writer is idle in the history, so two moving not-called
  writers no longer alternate), new load refusals (`InvalidBudget` at `budget.runs_per`,
  `budget.rework_limit`, `budget.rework_on`, `budget.rework_on.<kind>`; `MemberNotForExecutor` on
  agent, open and copy executors) and two texts of the run refusal `RunsSpent`. The ESS contract
  digest of Loom's bindings moved to
  `b1059a76e03d9c610f8ec466fc4753d09ac15c538c99fdd88c3e19cd41529600`.
- Decisions D633 and D634 of the runbook session: the binding of `change.implement` takes
  `runs_per: rejection` and `rework_on: {code_review: [rejected], merge_approval: [rejected]}`;
  the value of `rework_limit` is set from what the two suite entries expect.

## Acceptance

- `runbook/Cargo.toml` names the rev `f39f19b62aa84d8220d05bbdafe7df5ccd1af4bb` for every Loom
  crate; `runbook/Cargo.lock` holds no other Loom commit; no `[patch]`, no path, no `file://`.
- The crate builds and `cargo clippy --locked --all-targets -- -D warnings` exits 0 in `runbook/`.
  Each new member, variant and refusal of the embedder interface is handled by name
  (`RunsScope`, `ActionBudget.runs_per`, `rework_limit`, `rework_on`); the readers and checkers of
  bindings in this repository (`runbook check`, `scripts/check-bindings.sh`, the copy of the
  bindings a run makes, the page of the bindings format) know the three members.
- `bindings/sdc-story.yaml`: `change.implement` has `budget.runs_per: rejection`,
  `budget.rework_on: {code_review: [rejected], merge_approval: [rejected]}` and a
  `budget.rework_limit` whose value the unit report derives from the two suite entries. A failed
  check or a refused coverage stays inside the allowance of the case or the window
  (`conformance_run: [failed]`, `implementation_change: [coverage_refused]` open no window).
- `cycles/tree-suite.yaml`: `decider-rework-spent` (three `merge_approval` rejections, then exit
  4) and `rework-fresh-allowance` (four runs of `change.implement`: rejected, rejected, approved,
  exit 0) are run alone; one that is as expected loses `pending`; one that still differs keeps it
  with its difference in `docs/cycles-suite.md`. No entry changes from `expected` to another
  state.
- A run that ends because the rework limit is spent says so: the outcome, the exit code and the
  cause the runner gives for Loom's `RunsSpent` are stated in `docs/loom-run.md` and held by a
  test with sh programs.
- `docs/` and `AGENTS.md` name the pinned commit; `CHANGELOG.md` has the bullets.
- The tests of the touched code pass; no test runs over 60 s.

## Scope

`runbook/` (Cargo.toml, Cargo.lock, `src/loom_run/`, `src/engine/`, `src/check.rs`, tests),
`bindings/`, `cycles/`, `scripts/check-bindings.sh`, `docs/`, `AGENTS.md`, `CHANGELOG.md`,
`website/data/`.

## Not in this story

- The measured runs.
- A release: the pin is not released before the Loom session reports its review of the sha as
  done (a correction push moves the sha in a follow-up).
