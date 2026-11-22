---
format: aep.planning-md/3
id: story:ui-test-step-budget
kind: story
status: active
title: The UI test step ends within 120 s, no test over 60 s, no fixed port, the same with or without runs/
summary: 'UI Playwright step: free ports, a gate lane and a slow lane, the runner holds 60 s a test and 120 s the run'
tags:
- test-budget
relations:
- decomposes: epic:live-view
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: ui/Taskfile.yml
- confidence: cited
  path: ui/package.json
- confidence: cited
  path: ui/playwright.config.mjs
- confidence: cited
  path: ui/playwright.slow.config.mjs
- confidence: cited
  path: ui/test
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T02:13:42Z", actor: "human:operator", revision: 2, executor: "agent:wave-rb2-ui-test-budget", correlation: "operator-standing-acceptance-2026-11-22"}
- {from: "proposed", to: "active", at: "2026-11-22T02:13:42Z", actor: "human:operator", revision: 3, executor: "agent:wave-rb2-ui-test-budget", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: ui-test-step-budget

## Outcome

The UI test step (`task test` in `ui/`, Playwright) ends within 120 s of run time, no test runs longer
than 60 s, the runner kills a test at 60 s, the step prints its ten slowest tests and fails when it runs
over 120 s. It reads no `runs/` directory, no built binary and no server somebody else started, so it
gives the same counts on a fresh clone and on a machine that holds every recorded run. Every server a
test starts takes a port from the kernel; no test listens on a fixed port.

Rule served (`../AGENTS.md`, "Tests stay fast"): the test step ends within 2 minutes, no test over 60 s,
the runner enforces it, the step prints its ten slowest tests, "raising a timeout, adding shards or
waiting longer is not a fix".

## Measured before any change (base a793b75, no `runs/`, no built driver, `slot npx playwright test --workers=2`)

Wall 108.4 s of run time, 54 s waiting for a slot, 165 s from submit, machine load 25.6 29.7 33.0.
101 passed, 149 skipped, 0 failed. The sum of the test durations is 162.8 s.

Ten slowest (s): 21.8 autopilot (the decider's call), 12.2 and 8.4 and 6.3 stopped (three tests), 4.1
stub-runs list, 4.0 run-picker operator tile, 4.0 and 3.8 and 3.8 mobile, 3.8 screens agent step card.

What the time is spent on:

- No expensive product is made per test. One `fixture-run.mjs` play is 0.1 s and one server start is
  0.3 s (measured); each spec file starts its servers once in `beforeAll`, about a dozen in the run.
- Real-time waiting inside tests. The autopilot test plays a recorded decider call in real time at speed
  0.6 (17.0 s of the 21.8 s; 6.8 s at speed 1.5). The stopped tests hold 2.3 s and 2.1 s per viewport to
  show a clock stands still. The 26 `screens` tests wait 0.7 s each to let the page settle and
  screenshot it (50.7 s in all).
- Two workers. The files run in parallel, each in one worker; the longest file (stopped 30 s, autopilot
  24 s, mobile 25 s) bounds the run, and 162.8 s of test time on two workers is about 81 s.

Why 149 tests were skipped:

| Reason | Tests | Where |
| --- | --- | --- |
| the laptop project repeats a spec that is written for one viewport (`test.skip(project !== 'wide')`, existing) | 83 | autopilot 2, holder-model 7, meaning-model 11, mobile 10, now-model 19, paths 2, preflight 2, run-picker-experiments 5, run-picker 6, stall-model 1, stopped 5, stub-runs 4, view 9 |
| `runs/trial-todo-1` is not there (`needRun`) | 35 | now 22, changes 8, meaning-model 5 |
| `driver/target/debug/sdc-drive` and a canon binary are not built (`needDriver`) | 13 | scope 10, preflight 2, paths 1 |
| a server or a directory the caller names is not given (`SDC_UI_WRITE_URL`, `SDC_UI_LIVE_DIR`) | 18 | decisions 16, live 2 |

A skipped test hides coverage the step should have: the 35 that need `runs/trial-todo-1` (the now band
with an agent in flight, the changes feed, the YAML reader against the specification files). The
committed fixture `ui/test/recorded/trial-todo-1-slim/` holds the cases intake and direction only; it
serves what those cases serve (person, stopped, human, decider, autopilot) and nothing else. Widening
it needs a recorded run to cut from, which this checkout has not, and recorded text must not be
committed beyond what `slim-source.mjs` already scrubs. The 35 therefore move to the slow lane; the
proposal is to widen the slim copy in a later story.

A second finding: the 26 `screens.spec.mjs` tests were among the 101 passed, and passed only because a
server of another session listened on 7490 (the config's default `baseURL`) over real runs. Without
that server they skip; with it they pass and read data the step does not own. Hermetic count at the
base: 75 passed.

The three port failures:

- `shared-clock.mjs` has no port of its own; `now.spec.mjs` gave its clock server `port + 2`
  (7498 wide, 7499 laptop) and the other servers 7496 and 7497. Another session's server on 7499
  answers the clock fetch with HTML, so `now.spec.mjs:44` and `:131` fail with `SyntaxError: Unexpected
  token '<'`.
- `now.spec.mjs:245` ("no driver.json while observed.jsonl exists") depends on the port and not on
  the order of the tests: with a server holding 7496 to 7499, the base fails all 10 wide tests of the
  file, including `:245` (probe worktree at a793b75). The test reads source 6 of the server on 7496;
  when something else already answers there, the test reads that server's sources. Alone it passes
  because the port was free that time.
- Fixed ports in 11 specs (7503/7504, 7522, 7524, 7526, 7528, 7530, 7532, 7534, 7536, 7538, 7496 to 7499,
  and the config's 7490) and `--port + 50` in `scope.spec.mjs`; each spec checked only that its own
  port was free before it started.

## Change

- `ui/test/serve.mjs`: `startServer(args, {env})` starts `server.mjs --port 0`, reads the address the
  server prints and returns once it answers. Every spec that starts a server uses it; the
  `SDC_UI_*_PORT` knobs are gone.
- Two lanes (`ui/test/lanes.mjs`, each slow spec with the reason): the gate's lane
  (`playwright.config.mjs`, `task test`) and the slow lane (`playwright.slow.config.mjs`, `task slow`).
  `needRun` and `fixtureSource` take the committed slim copy in the gate's lane whether `runs/` exists
  or not.
- Specs split by what they need: `screens.spec.mjs` is the tour over the committed fixture (22 tests),
  `screens-runs.spec.mjs` the tour of the recorded runs with a server of its own; `meaning-model.spec.mjs`
  reads the work files of the committed fixture (3 more tests run), the two tests that need the
  specification files are `meaning-runs.spec.mjs`; `paths.spec.mjs` keeps the check of committed files,
  the served-pages check is `paths-served.spec.mjs`; the refused-principal test, which needs no driver,
  is `refused-principal.spec.mjs`.
- The runner holds the budget: `timeout` 60 s, `globalTimeout` 120 s, four workers in the task,
  `ui/test/slowest-reporter.mjs` prints the ten slowest tests and fails a run over its budget;
  `test.setTimeout(120000)` in the gate's specs is 60000. `budget.spec.mjs` holds the config and the
  specs to it (no timeout over 60 s, no fixed port, no `runs/` or binary in a gate spec).
- `SDC_UI_CANON` names the canon binary of the slow lane's fixtures (`ui/test/need-run.mjs`,
  `scope-fixture.mjs`).

No assertion changed, no test deleted, no `test.skip` added.

## Acceptance

- `task test` in `ui/` with no `runs/`: 105 passed, 83 skipped (all by the existing viewport rule),
  0 failed, in 60.9 s of run time (base: 101 passed, 108.4 s); the ten slowest tests are printed, the
  slowest is 22.7 s.
- The same step twice in a row, with `runs/` present and the old fixed ports held by other listeners:
  105 passed, 83 skipped both times, 92.5 s and 63.3 s of run time (machine load 28 to 38; slot wait
  108 s and 323 s, named apart).
- While listeners hold 7496, 7497, 7498, 7503, 7504, 7522, 7524, 7526, 7528, 7530, 7532, 7534, 7536, 7538
  and a server of another session holds 7490 and 7499, the slow lane's `now.spec.mjs` passes 22 of 22
  (including the clock tests at lines 44 and 131 and the test at line 245), where the base fails 10 of 10
  of its wide tests with the same listeners.
- A test that waits 70 s is killed at 60.0 s and fails the step (exit 201); the file is not kept.
- `git grep -nE '\b7[0-9]{3}\b' -- ui/test ui/playwright.config.mjs ui/playwright.slow.config.mjs`
  lists two lines, `fixture-run.mjs:351` (`sleep(7000 / speed)`) and `now-model.spec.mjs:230`
  (`fmtDur(7000)`), neither a port.
- `aep plan artifact validate` exits 0.

## Scope

`ui/test/`, `ui/playwright.config.mjs`, `ui/playwright.slow.config.mjs`, `ui/package.json` (script
entries), `ui/Taskfile.yml`, `AGENTS.md`.

## Out of Scope

- `ui/server.mjs` and `ui/decisions.mjs`: both look for the canon binary at a fixed path
  (`ui/server.mjs:744`, `:751`), so the slow lane's `scope.spec.mjs` and `preflight.spec.mjs` need the
  binary there; an environment variable (`SDC_CANON`) would remove the fixed path.
- `ui/README.md` still names port 7490 and the `SDC_UI_*_PORT` variables (lines 272 to 304).
- The autopilot test's real-time play (17 s of the longest test) and the 2 s settle waits.
- Widening `ui/test/recorded/trial-todo-1-slim` so that the 35 tests of the slow lane that need
  `runs/trial-todo-1` run in the step.
- The `ux/` loop (`ux/playwright.config.mjs`, its own 180 s timeout).
