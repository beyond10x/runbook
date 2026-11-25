---
format: aep.planning-md/3
id: story:the-ui-step-fits-the-forge-budget
kind: story
status: active
title: The UI test step ends within 120 s at the forge's CPU count and runs on the forge
summary: task test:ui on 2 CPUs at 4 workers reached 406-417 of 609 tests at its 120 s bound; cut the cost, keep every test and bound, then run it on the forge
tags:
- test-budget
relations:
- informed_by: story:ui-test-step-budget
- informed_by: story:local-gate-equals-forge
scope:
- confidence: cited
  path: .github/workflows/pages.yml
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: docs/forge-gate.md
- confidence: inferred
  path: ui/daemon
- confidence: cited
  path: ui/playwright.config.mjs
- confidence: inferred
  path: ui/server.mjs
- confidence: cited
  path: ui/test
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-24T18:28:47Z", actor: "human:operator", revision: 4, executor: "agent:runbook", correlation: "approval-record.the-ui-step-fits-the-forge-budget-order"}
- {from: "proposed", to: "active", at: "2026-11-24T18:28:47Z", actor: "human:operator", revision: 5, executor: "agent:runbook", correlation: "approval-record.the-ui-step-fits-the-forge-budget-order"}
---
# Story: the-ui-step-fits-the-forge-budget

## Outcome

The UI test step, `task test:ui`, ends within 120 s at the forge's CPU count with every test kept: on 2 CPUs
(`taskset -c 0-1`, the local forge way `UI_TEST_FORGE=1`) at 4 Playwright workers all of its tests pass inside the
runner's own 120 s bound, and the forge runs the same task for a code change. Then the local gate equals the forge for
every test step of `task check`.

Rules served: `../AGENTS.md` § "Tests stay fast" (a step over the budget is a defect, fixed before the next feature;
raising a timeout, adding shards or waiting longer is not a fix) and § "The local gate equals the forge". Order:
host-operator, 2026-11-24 11:17 CET: "The local gate equals CI: put them into the CI gate, or write in the repository
what the runner does differently and why".

## Measured before any change

By the CI-parity unit (`impl/the-forge-runs-the-driver-and-ui-tests`, head `07c03f42`) on the build node, 4 workers:

| CPUs | Result at the 120 s run bound | Whole run, extrapolated |
| --- | --- | --- |
| 2 (`taskset -c 0-1`, the forge's runner has 4 CPUs and comes closest to this) | 406, 412 and 417 of 609 tests done | about 177 s |
| 4 | 507 of 609 done | about 144 s |
| 64 (all) | 609 passed in 94 s | 94 s |

The slowest specs on the node (gate-budget unit): idle-slow-page drop up to 16 s, daemon-decide-review poll-rate
windows 6.3 s each, autopilot up to 9.5 s, security-start-* 4 to 5 s each, written-reader 20 s over 35 tests; about
355 s of test time in total.

Until this story is done the forge leaves `test:ui` out (`docs/forge-gate.md`, "Why `test:ui` stays out"; the
`OUTSIDE` line of `task check:ci-paths`), and a UI change is checked by the local gate only.

## Approach

Cut the cost, never the bound: a product made per test or per file (a server, a fixture run, a built page) is made
once per run; a wait for a time becomes a wait for an event or an injected clock (the page clock, `--round-ms`); a
case that checks a poll rate counts calls against the page clock where that keeps the assertion as strong. No
timeout or bound is raised, no shard added, no test skipped or dropped, the worker count stays 4.

## Acceptance

1. Three runs of `UI_TEST_FORGE=1 slot task test:ui` each print a Playwright summary line with every test of the step
   passed (609 or more, none skipped by a new condition, none failed) and a run time under 120 s; each run prints the
   load average before and after.
2. `ui/playwright.config.mjs` still holds 60 s a test and 120 s the run (`BUDGET`), and `test:ui` still starts
   Playwright with `--workers=4`.
3. `.github/workflows/pages.yml`, job "Build documentation", has a step `run: task test:ui` after the driver step,
   with the browser install, run only when the change class is `code`, and a `timeout-minutes` no larger than the
   runbook step's.
4. `task check:ci-paths` passes and prints that `test:ui` runs on the forge as `task test:ui`; it has no `OUTSIDE`
   line for `test:ui`.
5. `AGENTS.md` and `docs/forge-gate.md` no longer say that the UI step is left out of the forge.

## Scope

- `ui/test` cited (the specs, their fixtures and helpers; `docs/forge-gate.md` names the slow specs)
- `ui/playwright.config.mjs` cited (read for the bound; the bound itself does not change)
- `ui/server.mjs`, `ui/daemon` inferred (test options such as `--round-ms` live there)
- `Taskfile.yml` cited (`test:ui`, the `OUTSIDE` line of `check:ci-paths`)
- `.github/workflows/pages.yml` cited (job "Build documentation")
- `AGENTS.md`, `docs/forge-gate.md`, `CHANGELOG.md` cited

## Relations

Informed by `story:ui-test-step-budget` (the UI step's budget on the local host) and `story:local-gate-equals-forge`
(the rule this closes for the last test step).
