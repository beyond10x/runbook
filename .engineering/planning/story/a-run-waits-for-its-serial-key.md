---
format: aep.planning-md/3
id: story:a-run-waits-for-its-serial-key
kind: story
status: active
title: A run waits for the runs of its serial key
summary: A submit, a delegation and runbook run --detach may name a serial key; runs with one key run one at a time in submit order, across a restart.
tags:
- consumer-asks-2026-11-26
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-26T10:34:58Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-26T10:34:59Z", actor: "agent:runbook", revision: 3}
---
## Outcome

A submit, a delegation and `runbook run --detach` may name a serial key. Runs with one key run one at a time, in
the order they were submitted; runs with another key or no key are not held.

## Source

Order: the operator, 2026-11-26 (11:07, 11:17 and 11:20 CET), relayed by the host's operator session: the
highest goal is that the host operator's jobs run as runbook protocols, and runbook takes only what that goal
needs. The consumer that builds the first such protocol, merge-on-green of a pull request, asked for this (its
ask R2): two merges into one repository must not run at once, and the submitter, not the daemon, knows which
runs collide.

## What exists

- A submit is admitted and its run's thread starts at once; the only hold before the runtime takes a run up is
  the wait for its route (`WaitingForRoute`, `docs/model-routes.md`, section 3).
- `idempotency_key` (at most 200 characters) makes a second submit return the first run; it orders nothing.
- After a restart every run that was not taken up yet fails to start, and a run whose runtime is gone ends
  `orphaned` (`Daemon::mark_orphans`).

## What this story does

1. `serial_key` (1 to 200 characters) is a member of `SubmitRun` and `Delegate`, and `runbook run --detach
   --serial-key <k>` sends it. The daemon records it on the run and never derives one.
2. A run whose key a live run submitted before it holds is `WaitingOnKey`: no runtime start, nothing copied.
   The run views say `waiting on key <k> behind run <id>` (`key_wait`), naming the newest live run with the key
   submitted before it. Once no live run with the key was submitted before it, the run takes the key and starts
   as any submitted run does (its routes are read again then).
3. The key is free as soon as the run ahead ends in any way: ended, cancelled, failed to start, ended on its
   route, or orphaned at a restart. A cancel of a waiting run takes it out of the queue.
4. A restart keeps the waiting runs: the queue is rebuilt from the journal (submit order and key of each run),
   and each waiting run is taken up again once its key is free.
5. ESS: `serial_key`, the state `WaitingOnKey`, the commands `WaitOnKey` and `TakeKey` of the daemon process,
   their events, and a scenario; `ess/generated/` regenerated. `docs/daemon-api.md` states it.

## Acceptance

All in process against the stub runtime; no network but loopback, no test over 5 s.

- Two runs with key `k`: the second is `WaitingOnKey`, its view says `waiting on key k behind run <first>`,
  and it starts only after the first ended; three runs with one key start in submit order.
- A run with key `a` and a run with key `b` (and a run with no key) run at the same time.
- A cancel of the run that holds the key starts the next one; a cancel of a waiting run takes it out of the
  queue and the run behind it then waits on the run ahead of it.
- A holder that fails to start frees the key.
- A daemon restart while a run holds the key and two wait: the holder ends `orphaned`, the two waiting runs
  stay `WaitingOnKey` across the restart and start one after the other in submit order.
- A delegation and `runbook run --detach --serial-key k` are held the same way; a key of 201 characters, or
  an empty one, is refused before anything is written.
- `task check:ess-generated` passes with the new members, state and commands.

## Scope

- `runbook/src/commands.rs`, `runbook/src/state.rs`, `runbook/src/runtime.rs` (the route record keeps the
  setting of each role, so a waiting run's routes are rebuilt after a restart), `runbook/src/client/mod.rs`,
  `runbook/src/loom_run/here.rs`, `runbook/tests/daemon/`, `ess/`, `docs/daemon-api.md`, `CHANGELOG.md`.

## Not in this story

- A key the daemon derives (from the intake, the repository, the runbook).
- A limit of more than one run per key, or a pool of workers (`story:queue-and-worker-pool`).
