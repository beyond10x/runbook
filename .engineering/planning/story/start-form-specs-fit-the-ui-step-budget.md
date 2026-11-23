---
format: aep.planning-md/3
id: story:start-form-specs-fit-the-ui-step-budget
kind: story
status: active
title: The start-form specs of the UI step share their daemon and end within 40 s
summary: The 14 start-form spec files take 171 s alone against a step budget of 120 s; they share the binary and their daemons, lose no case, and the dead skip lines on the project name go.
relations:
- informed_by: story:ui-test-step-budget
scope:
- confidence: cited
  path: ui/test
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T13:48:40Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
- {from: "proposed", to: "active", at: "2026-11-23T13:48:40Z", actor: "human:operator", revision: 3, executor: "agent:runbook"}
---
## Defect

Measured 2026-11-23 while the unit of `story:ui-test-step-budget` was reviewed: the 14 spec files of the
daemon page's start form (`daemon-start` and 13 `security-start-*`) hold 65 cases and take 171 s on their
own, at a load average of about 68 on 20 cores. The budget of the whole UI test step is 120 s. With them
the step ends on its bound: one run counted 261 passed and 134 not run.

Each of these files builds on the `runbook` binary and starts a daemon of its own. That is the form the
budget rule names: an expensive product made once per file, and a server per test.

Found in the same review: 54 spec files that run at the wide size only still carry a skip line on the
project name. The line is dead since the lanes list decides the sizes, and a file moved to both sizes
would still skip the laptop size while the budget check stays green.

## Outcome

1. The start-form specs share what is expensive: the `runbook` binary is built once before the step, and
   the daemons they need are started once per run of the step, or once per file where a case changes the
   daemon's state, never once per case.
2. The 14 files end within 40 s together on a host at that load, and no case in them is over 20 s.
3. No case is removed, skipped or made weaker to get there: the list of file and title pairs before and
   after is the same, and every guard of the start form (the token lives in page memory only, no level is
   preset, a start below the daemon's floor starts nothing, one filled form starts at most one run) still
   has its case.
4. The dead skip lines are gone, and the budget check fails for a file listed for both sizes that skips on
   the project name.

## Acceptance

`playwright test --list` names the same file and title pairs before and after; the 14 files alone end
within 40 s with the runner's own summary line and the load average beside it; the budget check goes red
for a both-sizes file with a skip on the project name; the whole UI step is run once and its summary line
is reported as measured.

## Origin

The adversary passes on the unit of `story:ui-test-step-budget`, 2026-11-23.
