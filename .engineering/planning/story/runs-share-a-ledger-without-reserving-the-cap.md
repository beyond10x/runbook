---
format: aep.planning-md/3
id: story:runs-share-a-ledger-without-reserving-the-cap
kind: story
status: draft
title: Runs on one ledger reserve what a call may cost, not the whole cap
summary: A call whose bindings name no per-call budget reserves the whole cap, so a second run on the ledger is granted 0 and stops; the reservation gets a bound that is not the cap.
tags:
- upstream-asks-2026-11-24
refs:
- provider: consumer
  reference: upstream-asks-filed/A3
relations:
- depends_on: story:spend-cap-with-nothing-left-starts-nothing
- depends_on: story:caps-are-optional
- informed_by: story:handoff-under-a-named-cap-runs-one-step-at-a-time
scope:
- confidence: inferred
  path: docs/loom-run-settings.md
- confidence: cited
  path: docs/loom-run.md
- confidence: inferred
  path: runbook/src/loom_run/budget.rs
- confidence: cited
  path: runbook/src/loom_run/ledger.rs
- confidence: cited
  path: runbook/src/loom_run/run_loop.rs
- confidence: inferred
  path: runbook/tests/loom_run
revision: 3
---
## Outcome

Several runs on one spend ledger run side by side. The runner reserves what one call may cost, not what the
cap has left, so a run does not hold the whole cap while its call runs.

## Source

Ask A3 of a consumer, filed in its story `upstream-asks-filed` (read 2026-11-24): the runner reserves the
whole cap per run, so two runs on one ledger both end `stopped: spend cap`. The consumer shared one ledger
between runs of one day on purpose.

## What the code does at `origin/main`

- `costed` in `runbook/src/loom_run/run_loop.rs` takes `want` as the largest per-call budget of the bindings
  copy, and the whole `--cap-micro-usd` when the bindings name none:
  `budget::largest_call_budget(..)?.unwrap_or(self.args.cap_micro_usd)`.
- `Ledger::reserve` in `runbook/src/loom_run/ledger.rs` grants `min(want, cap - spent - reserved)` under the
  lock.
- So for a runbook whose bindings name no per-call budget, the first run's `tree run` holds everything that
  is left while it runs, and a second run on the same ledger is granted 0 and ends `stopped: spend cap`.
  `docs/loom-run.md` ("The spend ledger") states that two runs never grant more than the cap; it does not say
  they are held one after the other.
- The consumer reports that both runs end stopped. Reading shows the second one; the first case of the unit
  measures whether the first can end the same way (a call that books the whole reservation).

## What this story does

1. The amount reserved for one call has a bound that is not the cap: the per-call budget of the bindings when
   one is named, else a reservation setting of the run (`reserve_micro_usd`), else a stated default. The unit
   decides which default, writes it in `docs/loom-run-settings.md`, and may instead take one step per call
   (`story:handoff-under-a-named-cap-runs-one-step-at-a-time`).
2. A call that books more than it reserved is booked whole, as today (`docs/loom-run.md`, "A call can end above
   its budget"): the ledger can end above the cap by at most the documented amount.
3. The rule that a cap with nothing left starts nothing (`story:spend-cap-with-nothing-left-starts-nothing`) is
   unchanged: a grant of 0 stops the run before any program starts.

## Acceptance

With a stand-in model program whose calls cost a fixed small amount, and no per-call budget in the bindings:

- Two runs started at the same time on one ledger with a cap of at least their two spends together both end
  as ended; the ledger holds both spends and no run ends `stopped: spend cap`.
- Two runs on one ledger whose cap is less than their two spends together: the ledger ends above the cap by no
  more than the documented amount, and the run that finds nothing left ends `stopped: spend cap` before it
  starts a program.
- A ledger with 0 left starts nothing, as on `main` (the committed spend cases pass unchanged).
- A SIGKILL of a run leaves its reservation as `docs/loom-run.md` describes, and a next run finds less left,
  not more (the committed case stays).
- No test runs over 5 s, none over 60 s.

## Scope

- `runbook/src/loom_run/run_loop.rs`, `runbook/src/loom_run/ledger.rs`, `runbook/src/loom_run/budget.rs`,
  `runbook/tests/loom_run/`, `docs/loom-run.md`, `docs/loom-run-settings.md`, `CHANGELOG.md`.

## Not in this story

- Caps of the daemon, per day or per run (`story:daemon-caps-are-keys-of-one-file`).
