---
format: aep.planning-md/3
id: story:loom-inproc-named-pipe-case-holds-under-load
kind: story
status: draft
title: The in-process named-pipe findings case passes under the load of the whole test step
summary: 'loom_run::inproc::a_named_pipe_a_program_put_at_findings_yaml_does_not_hold_the_end_of_the_run failed once in task check at load 29 and passed alone 3 of 3: fix the test''s wait or quarantine it'
revision: 1
---
## Outcome

The case `loom_run::inproc::a_named_pipe_a_program_put_at_findings_yaml_does_not_hold_the_end_of_the_run`
(`runbook/tests/loom_run/inproc.rs:1446`) passes in every run of `task check`, also under the load of the whole
step, or it is quarantined with a reason and a story that brings it back. A further green run does not settle it.

## The failure seen

One `task check` on this workstation during the model-routes wave (2026-11-26, load average about 29), step
`test:runbook`:

```
FAIL [   8.784s] (1106/1779) runbook::cli loom_run::inproc::a_named_pipe_a_program_put_at_findings_yaml_does_not_hold_the_end_of_the_run
panicked at tests/loom_run/inproc.rs:1452:5:
the run did not end within 6 s of the plant: the copy of work/findings.yaml into the store reads a pipe that a program put there and waits for a writer ()
Summary [ 308.001s] 1779 tests run: 1778 passed (1 slow), 1 failed, 32 skipped
```

Alone, the case passed 3 of 3 (1.030 s, 1.050 s, 1.029 s; load average 21.5 to 22). The wave did not touch the
case or the code it covers.

The case waits for the run to end within `RECORD_AFTER_PLANT_SECS` (6 s) of the plant (`ended_after_plant_within`,
`runbook/tests/loom_run/inproc.rs:988`), timed by the host's clock while the step runs about 1,800 other cases.

## Acceptance

- A loop makes the failure happen on demand (for example the case repeated under the load of the whole step), shown
  red before the fix.
- The fix is in the test's wait (an event the case waits for in place of a time measured on a loaded host), or the
  case is quarantined with its reason and a story that brings it back. No production code changes for it.
- After the fix the same loop is green, and `task check` passes.
- No timeout or bound is raised as the fix.
