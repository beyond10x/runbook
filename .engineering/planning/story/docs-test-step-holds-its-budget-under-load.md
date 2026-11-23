---
format: aep.planning-md/3
id: story:docs-test-step-holds-its-budget-under-load
kind: story
status: active
title: The docs test step ends within 120 s under the host's load
summary: The local docs:test ran into its 120 s wall at a load average of 93 to 108 with 20 tests passed and none failed (2026-11-23); the same on 2026-11-22 at 86. A step over its budget is a defect.
relations:
- informed_by: story:docs-test-budget
scope:
- confidence: cited
  path: website/test
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T09:37:05Z", actor: "human:operator", revision: 3, executor: "agent:runbook"}
- {from: "proposed", to: "active", at: "2026-11-23T09:37:05Z", actor: "human:operator", revision: 4, executor: "agent:runbook"}
---
## Outcome

The step `task docs:test` ends within its 120 s budget on the build host under the load the host has while
other builds run.

## Source

2026-11-23, the merge of `story:run-settings-are-toml`: the local `docs:test` ran into its 120 s wall with
20 tests passed and none failed, at a load average of 93 to 108 on 20 cores. Its ten slowest tests took 12.7
to 41.5 s each, all in `smoke.spec.ts` (one page per test). The same was measured on 2026-11-22 for release
0.5.0 at a load average of 86. `story:docs-test-budget` moved the step under its budget on a quiet host; it
does not hold under load, and the rule of the host is that a step over the budget is a defect and that
waiting longer is not a fix.

The cost per test is one page load of the built site. What each test pays before its first assertion (a
server start, a browser context, the first load of the shared bundles) is not measured yet.

## Scope

- `website/` test configuration and `website/test/smoke.spec.ts`; the task `docs:test` in `Taskfile.yml`.

## Acceptance

- Measured first: per test, the time before the first assertion and the time of the page load, at a load
  average above 80.
- `task docs:test` ends within 100 s at a load average above 80, every case run and green, in three runs;
  the runner's lines, the wall time and the load are in the change's text.
- No case and no `expect` line is dropped, no timeout is raised, no retry is added.
- The step prints its ten slowest tests, and a test over 60 s is ended and counts as failed.

## Not in this story

- The UI test step (`story:ui-test-step-budget`) and the test step of the runbook crate
  (`story:runbook-test-step-budget`).
