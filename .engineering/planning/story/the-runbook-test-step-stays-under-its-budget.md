---
format: aep.planning-md/3
id: story:the-runbook-test-step-stays-under-its-budget
kind: story
status: active
title: The runbook test step stays clearly under its 120 s budget on the workstation under load
summary: Measured 121.4 s alone and 126.0 s in the gate at load 40 to 46 on 2026-11-27; the per-test costs are found and made once.
tags:
- test-budget
relations:
- informed_by: story:runbook-test-step-budget
- informed_by: story:runbook-test-step-within-budget-on-the-node
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T17:12:11Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-27T17:12:12Z", actor: "agent:runbook", revision: 3}
---
## Outcome

`task test:runbook` ends clearly below its 120 s budget on the shared workstation under its usual
load, and the sum of its test times is lower than before.

## Source

- The rule "Tests stay fast" of the organisation's `AGENTS.md`: a step over the budget is a defect,
  it gets a story and is fixed before the next feature; raising a timeout, adding shards or waiting
  longer is not a fix.
- Measured on 2026-11-27 on the workstation (load average 40 to 46): 2000 passed in 126.0 s inside
  the gate and 121.4 s alone, on the branch that added two stories to main; earlier the same day
  1980 passed in 82.1 s to 117.7 s at load averages of 33 to 55.

## Acceptance

- One timed run on the base names the ten slowest tests and the expensive steps made per test (a
  daemon or a server per test, a fixture per test, a binary built or copied per test, a sleep on
  the host clock).
- The top costs are fixed by the accepted forms: one shared fixture per run keyed by a digest under
  `target/`, the test clock instead of a sleep, fewer daemon starts by grouping cases that only
  read. No timeout is raised, no shard added, no test's coverage lowered.
- The step's wall time on the workstation, under load, is clearly below 120 s, and nextest's sum of
  test times is lower than on the base; the report names both times with the load average read at
  each start.
- Every test over 5 s is named with its time.
