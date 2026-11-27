---
format: aep.planning-md/3
id: story:inproc-process-sample-judges-a-child-after-its-exec
kind: story
status: draft
title: The in-process process sample judges a child by its name after exec
summary: 'inproc.rs:401 saw a descendant named runbook on the forge (PR 122): a forked child sampled before its exec; judge the name the pid ends with'
tags:
- backlog
revision: 1
---
## Context

The case `only_the_programs_of_the_runbook_run_and_never_a_loom_canon_or_driver_process`
(`runbook/tests/loom_run/inproc.rs:375-403`) samples the processes below the runner every 10 ms with
`descendants` (`inproc.rs:163-183`), which takes each process's name from `/proc/<pid>/stat`, and asserts
every name seen is one of `sh`, `sleep`, `cp`, `cat`, `draft-write.sh`, `draft-check.sh`
(`inproc.rs:401`). A process the runner has forked and that has not yet called `exec` carries the runner's
own name, `runbook`. A sample taken in that window records `runbook`, and the case fails although the
process became one of the allowed programs a moment later.

## Evidence

- On the forge, pull request 122, 2026-11-27: `an unexpected process ran: runbook in {draft-write.sh,
  runbook, sleep}` at `inproc.rs:401`. Passed on rerun.
- *Inferred*: the `runbook` entry was a forked child sampled before its `exec`. No process of the runner
  starts `runbook` again in this case.

## Acceptance

- The sample judges a process by the name it has after its `exec`: a pid seen with the runner's own name is
  read again until its name changes or it ends, and only that last name is judged. A child that stays
  `runbook` until it ends still fails the case. The race is closed by construction, not by a slower sample.
- A red-first proof of the window: the sample, given a process that is forked and held before its `exec`,
  reports `runbook` before the change and the program's name after.
- The case passes 50 of 50 runs under load (`--stress-count 50`, judged by its Summary line) and stays
  under 5 s. No product code changes.
