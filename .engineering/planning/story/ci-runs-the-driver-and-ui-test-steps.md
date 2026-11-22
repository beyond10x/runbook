---
format: aep.planning-md/3
id: story:ci-runs-the-driver-and-ui-test-steps
kind: story
status: proposed
title: CI runs the driver's and the UI's test steps
summary: no workflow runs the driver tests (nextest) or the UI tests (Playwright); only the docs site and the classifier run in CI
tags:
- review-rb2
scope:
- confidence: cited
  path: .github/workflows
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: ui/Taskfile.yml
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T02:50:59Z", actor: "human:operator", revision: 3, executor: "agent:rb2-review-fixes-ui", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: ci-runs-the-driver-and-ui-test-steps

## Outcome

A pull request that changes code runs the driver test step and the UI test step on the forge, each inside the 120 s budget of the test step, and a red result blocks the merge.

## Context

Found in the wave rb2 review of runbook/, protocol/, ui/, views/, docs/ (S3 list): .github/workflows/pages.yml (no step for driver/ or ui/ tests), Taskfile.yml (test:driver), ui/Taskfile.yml (test), review S2-5 and answer 8. Filed by the review-fix unit of wave rb2; the fix is not part of that unit.

## Acceptance

- A pull request that makes one driver test and one UI test fail shows both steps red on the forge, and a pull request that changes only planning files runs neither (the classifier of `pages.yml`).

## Scope

`.github/workflows`, `Taskfile.yml`, `ui/Taskfile.yml`.
