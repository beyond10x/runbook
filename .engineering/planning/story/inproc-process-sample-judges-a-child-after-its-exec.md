---
format: aep.planning-md/3
id: story:inproc-process-sample-judges-a-child-after-its-exec
kind: story
status: active
title: The in-process process sample judges a child by its name after exec
summary: 'inproc.rs:401 saw a descendant named runbook on the forge (PR 122): a forked child sampled before its exec; judge the name the pid ends with'
tags:
- backlog
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T11:03:48Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-27T11:03:48Z", actor: "agent:runbook", revision: 3}
---
## Context

Two cases sample the processes below a run every 10 ms and refuse any name outside a fixed list:

1. `only_the_programs_of_the_runbook_run_and_never_a_loom_canon_or_driver_process`
   (`runbook/tests/loom_run/inproc.rs`) samples the processes below the `runbook run` process.
2. `only_the_programs_of_the_runbook_run_and_nothing_is_written_beside_the_state_directory`
   (`runbook/tests/daemon/engine.rs`) samples the processes below the test process, whose daemon runs the engine in
   process.

Each took the name of a process from `/proc/<pid>/stat`. A process that its parent has forked and that has not yet
called `exec` carries the name of the thread that forked it: `runbook` below the runner, `run-<id>` below a daemon run's
thread. A sample taken in that window recorded that name, and the case failed although the process became one of the
allowed programs a moment later. A third copy of the same walk (`runbook/tests/test_suite/mod.rs`) refuses only the
forbidden names and could not fail this way.

## Evidence

- On the forge, pull request 122, 2026-11-27: `an unexpected process ran: runbook in {draft-write.sh, runbook, sleep}`
  in case 1. Passed on rerun.
- Case 2 alone, 1 in 20: `an unexpected process ran: run-01a21c8a-f8 in {draft-check.sh, draft-write.sh,
  run-01a21c8a-f8, sleep}`.
- Measured on the base (`--stress-count 50`, each case alone, this host under load): case 1 47 passed, 3 failed (each
  `runbook`); case 2 49 passed, 1 failed (`run-01a21c94-e1`).

## Acceptance

- The sample judges a process by the program it runs after its `exec`. A child that runs its parent's executable with
  its parent's arguments has not called `exec` yet: it is not judged and is sampled again by the next tick. The race is
  closed by the order in which the sample reads what the kernel's `exec` writes (executable, then name, then
  arguments), not by a slower sample: a child whose arguments differ from its parent's is judged by its name read after
  them, a child that has an executable and shows no arguments is inside its `exec` and is read again, and a child
  without memory is ending or ended and is judged by the name it keeps.
- A child that runs its parent's program with arguments of its own is judged by its name (a `runbook` started again
  below the runner fails case 1), a child that ended without an `exec` (a zombie) is judged by the name it carries, and
  a forbidden program (`b10x-loom`, `loom`, `canon`, the driver) still fails both cases.
- One helper serves every case that samples a run's processes (`runbook/tests/process_sample/mod.rs`); the three
  copies are gone.
- A red-first proof of the window: the sample, given a process that is forked and held before its `exec`, reports the
  forking thread's name before the change and nothing until the program's name after it.
- Each case passes 50 of 50 runs alone (`--stress-count 50`, judged by its Summary line) and stays under 5 s; a mutant
  that starts a program named like a forbidden one makes each case red. No product code changes.
