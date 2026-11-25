---
format: aep.planning-md/3
id: story:flaky-daemon-step-and-drain-tests
kind: story
status: implemented
title: The daemon step-in-flight and drain submit-race tests pass on every gate run
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-25T09:03:42Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
- {from: "proposed", to: "active", at: "2026-11-25T09:03:42Z", actor: "human:operator", revision: 3, executor: "agent:runbook"}
- {from: "active", to: "implemented", at: "2026-11-25T10:45:01Z", actor: "agent:runbook", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

The two daemon tests below pass on every run of the gate: each failure seen in the gate runs of
2026-11-25 has a named cause that is fixed in the test or in the daemon, or the test is quarantined
with a reason and a story that brings it back. A flaky test is not settled by a further green run.

1. `daemon::engine::a_step_is_recorded_and_shown_while_its_program_runs_and_gone_when_it_ends`
   (`runbook/tests/daemon/engine.rs:538`).
2. `daemon::drain::a_submit_racing_the_signal_leaves_no_run_submitted_after_draining_and_none_unended_at_drained`
   (`runbook/tests/daemon/drain.rs:414`).

## The failures seen

Both on the build node, inside `task check` of the R1 branch (`impl/decision-options-come-from-a-case-artifact`),
R1 touched neither test. Each test passed in the other gate runs of that day, and the engine test
passed 5 of 5 alone (about 1 s each).

Run 1, head 25792bec, 08:15 CET, step `test:runbook` 1690 of 1691 passed:

```
FAIL [  20.629s] ( 703/1691) runbook::cli daemon::engine::a_step_is_recorded_and_shown_while_its_program_runs_and_gone_when_it_ends
thread 'daemon::engine::a_step_is_recorded_and_shown_while_its_program_runs_and_gone_when_it_ends' panicked at tests/daemon/engine.rs:91:9:
gave up waiting for the writer's pause to be seen and ended
```

The wait at `engine.rs:558` looks for a child process named `sleep` among the test's descendants
(`descendants()`, read from `/proc`) and ends the pause; it gave up after 20 s.

Run 2, head 25792bec, 08:21 CET, step `test:runbook` 1690 of 1691 passed:

```
FAIL [   2.770s] ( 385/1691) runbook::cli daemon::drain::a_submit_racing_the_signal_leaves_no_run_submitted_after_draining_and_none_unended_at_drained
thread 'daemon::drain::a_submit_racing_the_signal_leaves_no_run_submitted_after_draining_and_none_unended_at_drained' panicked at tests/daemon/drain.rs:438:5:
["delivered" x22, "runtime-failure"]
```

23 runs were accepted before the drain; 22 ended `delivered`, one `runtime-failure`. The assertion
prints the outcomes only, not the run's reason or the daemon's stderr, so the log does not say why.

## Acceptance

- Each failure has a loop that makes it fail on demand (for example the test repeated under the load
  of the full step), shown red before the fix.
- After the fix the same loop is green, and `task check` passes.
- The drain assertion prints the reason of a run that did not end `delivered`.
- A `runtime-failure` that is a daemon defect is fixed in the daemon, not hidden in the test.
- No timeout is raised as the fix (`AGENTS.md` § Tests stay fast).
