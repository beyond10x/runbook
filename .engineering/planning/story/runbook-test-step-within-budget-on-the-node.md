---
format: aep.planning-md/3
id: story:runbook-test-step-within-budget-on-the-node
kind: story
status: active
title: The runbook test step stays within its budget when it runs on the build node again
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-26T14:25:00Z", actor: "agent:runbook", revision: 4}
- {from: "proposed", to: "active", at: "2026-11-26T14:25:01Z", actor: "agent:runbook", revision: 5}
---
## Outcome

The `test:runbook` step ends within its 120 s budget wherever the gate runs it, and its time is measured again when Runbook jobs go back to the build node.

## Notes

- 2026-11-25, 23:0x CET on: Runbook jobs run on the workstation, not the build node (the node lacks an aep release binary for its glibc, upstream beyond10x/aep#175, and the ess 0.98.0 cache file).
- Measured on the workstation with 1776 tests: 181.9 s alone (5 test threads, load average about 10); 313.9 s inside `task check` at load average 38.
- Measured on the build node the same evening with 1749 to 1763 tests: 22 s to 36 s.
- Until the step runs on the node again, a budget miss on the workstation with every test passing does not block a merge (the operator, 2026-11-25).

## Acceptance

- On the build node, `task check` runs `test:runbook` in under 120 s, three runs in a row; the times are recorded here.
- If the step is over 120 s there too, the slowest ten tests are named and the step is brought under the budget by the repository's rule (an expensive product made once, not per test).

## Measured on the forge, 2026-11-26

The budget is measured where the forge runs the step: the job "Build documentation" of `.github/workflows/pages.yml`, step "Test the runbook crate with the local gate's task" (`task test:runbook`, nextest profile `forge`, 4 test threads on a 4-CPU runner). The local forge way is `RUNBOOK_TEST_FORGE=1 slot task test:runbook` (2 CPUs of this host).

- 2026-11-26, pull request 112, job "Documentation validation": `Summary [ 123.306s] 1902 tests run: 1902 passed, 32 skipped`, "the test run took 123.306s, over the 120s budget", exit 201, load average 6.42 after the run. The ten slowest: 12.1 s `loom_run::gateway_signals_and_files::a_hangup_that_the_caller_ignores_does_not_end_the_run`; 8.1 s each the three `loom_run::gateway_dispositions_bound_and_kinds::the_runner_*`; 4.9 s `daemon::caps::a_program_an_answer_starts_reserves_its_budget_and_is_booked`; 4.6 s `daemon::copy_integrity::…`; 4.2 s `daemon::caps::a_file_member_covers_…`; 4.2 s `daemon::run_route::runs_naming_a_and_b_…`; 4.1 s `daemon::drain::three_runs_at_once_…`; 4.0 s `daemon::caps::a_program_an_answer_starts_that_changes_…`.
- 2026-11-26, run 45805772925 (branch `wave/bound-calls`), job 139860493306: 112.7 s for the same 1902 tests; the test times summed to 446 s, so the step is that sum over the 4 threads: a test that waits holds a thread as long as one that computes.
- On this workstation the step ran 231 s to 395 s at load averages of 18 to 85 during waves 4 and 5.
- The order (the operator, 2026-11-26, D1 A): the step is brought under the budget before any feature, by the repository's rule; a raised timeout, shards or a longer wait are no fix. The target is under 90 s on an idle runner of the forge's size.
