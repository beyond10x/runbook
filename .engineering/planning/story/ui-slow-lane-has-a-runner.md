---
format: aep.planning-md/3
id: story:ui-slow-lane-has-a-runner
kind: story
status: proposed
title: The UI slow lane runs in a job that someone reads
summary: the specs of ui/test/lanes.mjs SLOW are run by no job, so what moved out of the gate is covered by nothing automatic
tags:
- review-rb2
scope:
- confidence: cited
  path: .github/workflows
- confidence: cited
  path: ui/Taskfile.yml
- confidence: cited
  path: ui/playwright.slow.config.mjs
- confidence: cited
  path: ui/test/lanes.mjs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T02:50:58Z", actor: "human:operator", revision: 3, executor: "agent:rb2-review-fixes-ui", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: ui-slow-lane-has-a-runner

## Outcome

The slow lane of the UI tests runs on a machine that has `runs/`, the built driver and a canon binary, and its result is read before a release.

## Context

Found in the wave rb2 review of runbook/, protocol/, ui/, views/, docs/ (S3 list): ui/test/lanes.mjs (SLOW list), ui/playwright.slow.config.mjs, review answer 8. Filed by the review-fix unit of wave rb2; the fix is not part of that unit.

## Acceptance

- A release is refused by `task release:check` when no slow lane result for the release commit is recorded, and a recorded run of `task slow` with every spec of `ui/test/lanes.mjs` SLOW passing makes it accept.

## Scope

`ui/test/lanes.mjs`, `ui/playwright.slow.config.mjs`, `ui/Taskfile.yml`, `.github/workflows`.
