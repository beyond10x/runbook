---
format: aep.planning-md/3
id: story:the-ui-test-step-stays-under-its-budget
kind: story
status: active
title: The UI test step stays clearly under its 120 s budget on the workstation under load
summary: Measured 108.5 s to 120 s alone and stopped at 120 s inside task check on 2026-11-27; the waits and per-spec costs are found and cut.
tags:
- test-budget
relations:
- informed_by: story:the-runbook-test-step-stays-under-its-budget
- informed_by: story:ui-test-step-budget
- informed_by: story:the-ui-step-fits-the-forge-budget
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T20:02:21Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-27T20:02:21Z", actor: "agent:runbook", revision: 3}
---
## Outcome

`task test:ui` ends clearly below its 120 s budget on the shared workstation under its usual load, alone and
inside `task check`, and the sum of its test times is lower than before.

## Source

- The rule "Tests stay fast" of the organisation's `AGENTS.md`: a step over the budget is a defect, it gets a
  story and is fixed before the next feature; raising a timeout, adding shards or waiting longer is not a fix.
- Measured on 2026-11-27 on the workstation, `task test:ui` alone: 684 passed in 108.5 s (load average 22.6),
  115.2 s (17.9) and 117 s (32.9), and once 683 passed with 1 not run when the 120 s bound ended it (load 22 to
  24). Inside `task check`, beside the driver and runbook steps, most runs that day ended at 120 s with 190 to
  194 tests not run.
- The ten slowest at load 9.7 (step 82.8 s) were daemon and server cases of 3 s to 5.5 s each: the decide
  review refusals, the polled route rate, the late paint of the start form, the dropped idle page, the
  written reader's redraw, bounds and counted bytes.

## Acceptance

- One timed run on the base names the ten slowest tests and where the wall time goes: fixed sleeps or polls
  on the host clock, a server or fixture started per spec where one per worker would do, the worker count,
  the global setup, CPU against waiting.
- The top costs are fixed by the accepted forms: one shared fixture per run keyed by a digest, the server's
  own test clock or a fake timer instead of a real wait, a poll interval the test controls. No timeout is
  raised, no shard added, no case dropped or weakened.
- The step's wall time on the workstation, alone and inside `task check`, is clearly below 120 s; the report
  names the times before and after with the load average read at each start.
- Every test over 5 s is named with its time.
