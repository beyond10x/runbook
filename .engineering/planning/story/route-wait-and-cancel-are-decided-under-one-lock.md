---
format: aep.planning-md/3
id: story:route-wait-and-cancel-are-decided-under-one-lock
kind: story
status: draft
title: Leaving the route wait and a cancel of the run are decided under one lock
summary: 'wait_for_routes reads the cancel flag outside the state lock, and cancel_run sets the flag after it released the lock: a run cancelled as its route turns ready may start'
tags:
- backlog
revision: 1
---
## Context

A run whose model routes cannot all answer at the submit is held in `WaitingForRoute` by `wait_for_routes`
(`runbook/src/commands.rs:2374-2434`), called from the run's thread (`commands.rs:2638-2643`). The wait
reads the run's cancel flag at `commands.rs:2392`, `commands.rs:2405` and, when the routes answer,
`commands.rs:2417` (`Ok(None) => return !cancel.load(...)`). None of those reads holds the state lock
(`self.st()`, `commands.rs:1083`).

`cancel_run` (`commands.rs:1685-1706`) commits `RunCancelled` under the state lock (`commands.rs:1693-1695`),
withdraws the open decisions, drops the lock (`commands.rs:1700`), and only then sets the flag
(`commands.rs:1701-1703`). Between `commands.rs:1700` and `commands.rs:1702` the run is `Cancelled` in the
state while its flag still reads false. A route read that answers in that window makes `wait_for_routes`
return true; the run's thread then copies the context into the workspace (`commands.rs:2644-2652`) and
reads the flag once more, again without the lock, before it calls `runtime.start` (`commands.rs:2665`).

## Evidence

- The four lines above, on `origin/main` `c6dbfc31`.
- *Inferred*, not reproduced: a run cancelled at the instant its route becomes ready starts its runtime (or
  at least copies its context) after `CancelRun` was answered `cancelled`. The path `end_route_wait` takes
  is safe: it checks `WaitingForRoute` under the lock (`commands.rs:1825-1836`).

## Acceptance

- A deterministic red-first case under `runbook/tests/daemon/` shows the defect: a run whose `CancelRun` was
  answered `cancelled` while its route turned ready gets a context copy or a program start. If such a case
  cannot be built without a seam the product would carry only for that test, the story says so and goes to
  the operator before any product change.
- After the change, leaving the route wait and the cancel are decided under one lock (the flag is set before
  the state lock is released, or the wait's exit reads the run's state under that lock), so the window is
  closed by construction; the red case is green.
- The case runs under 5 s and uses the daemon's clock (`Daemon::start_with_clock`, `SetClock`) for the 5 s
  route read, never a sleep against the host clock.
