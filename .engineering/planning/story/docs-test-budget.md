---
format: aep.planning-md/3
id: story:docs-test-budget
kind: story
status: active
title: The docs test step ends within 120 s, no test over 60 s
summary: Docs Playwright suite from 495 s to the 120 s step budget with the same assertions
tags:
- test-budget
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: .github/workflows/pages.yml
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: website/playwright.config.ts
- confidence: cited
  path: website/tests
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T21:47:10Z", actor: "human:operator", revision: 2, executor: "agent:ci-test-budget", correlation: "operator-standing-acceptance-2026-11-21"}
- {from: "proposed", to: "active", at: "2026-11-21T21:47:10Z", actor: "human:operator", revision: 3, executor: "agent:ci-test-budget", correlation: "ci-test-budget"}
---
# Story: docs-test-budget

## Outcome

The documentation test step ends within 120 s of wall time and no test runs longer than 60 s,
locally and in CI, with the same assertions as before. Measured on 2026-11-21 in a gate round:
495 s for 126 tests.

Rule served (`../AGENTS.md`, "Tests stay fast"): "The test step of a repository's gate ends within
2 minutes of wall time." "No single test runs longer than 60 s." "An expensive product is made
once, never per test." "The runner enforces the budget: a test over 60 s is killed and counts as
failed (... Playwright `timeout`), and the step prints its ten slowest tests." "The step itself
fails when it runs over 120 s, locally and in CI."

## Where the time went

- No product was made per test: `docs:build` builds the site and the fixture site once, and
  `webServer` starts one `docusaurus serve` for each, once per run. There is no `globalSetup`.
- The cause: Playwright runs the tests of one file in one worker unless `fullyParallel` is set.
  `smoke.spec.ts` holds 69 of the 126 tests (17 routes in 4 modes, about 2.8 s each with axe), so
  one worker ran them one after another while the other workers ran out of files. Two workers,
  `site` project alone: 51 of 79 tests in 120 s, `smoke.spec.ts` 113 s of test time.
- Two pitch tests slept 25.5 s of real time (`waitForTimeout` of 8.5 s, three times).
- The internal-link test opened 17 pages in a browser to read links the server-rendered HTML
  already holds.
- 18 tests in `navigation.spec.ts` repeated the horizontal-scroll check of `smoke.spec.ts` and
  `context.spec.ts` at 390 px in both themes.

## Change

- `website/playwright.config.ts`: `fullyParallel: true`, `timeout: 60_000`, `globalTimeout:
  120_000`, a reporter that prints the ten slowest tests (`website/tests/slowest-reporter.ts`).
- Four workers in `task docs:test` and in CI; the CI step has `timeout-minutes: 2`, with the
  browser installed in a step before it.
- Pitch timer tests run on Playwright's clock (`page.clock.install`, `runFor`), same assertions.
- Axe runs with `resultTypes: ['violations']`: the violations it reports are the same, passes are
  not collected.
- The internal-link test reads each route's HTML over HTTP, without a browser; on the current build
  it finds the same internal paths as the browser on every one of the 17 routes.
- Removed: the 18 `390 navigation, light|dark › no horizontal scroll on /<route>` tests. Still
  covered by `smoke.spec.ts` `390 light|dark › /<route>` (same viewport, theme and assertion) for
  eight routes, and by `context.spec.ts` `context 390 light|dark` for `docs/context`.

## Acceptance

- `slot --tokens 4 task docs:test` ends within 120 s; each test is bounded by 60 s; the run prints
  its ten slowest tests. Measured on the workstation after the change: 108 tests in 59 s with four
  workers, slowest 4.5 s; 95 s with two workers.
- Before, measured the same way (original suite, four workers, capped at 120 s): 101 of 126 tests
  in 120 s, 25 not run; slowest 17.9 s (the pitch auto-advance test).
