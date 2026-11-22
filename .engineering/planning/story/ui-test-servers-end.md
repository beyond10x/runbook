---
format: aep.planning-md/3
id: story:ui-test-servers-end
kind: story
status: implemented
title: The UI test step ends its servers when it is ended
summary: Every server, player and daemon a UI spec starts ends with the process that started it, also after SIGKILL
tags:
- test-budget
relations:
- decomposes: epic:live-view
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: ui/test
- confidence: cited
  path: website/playwright.config.ts
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T15:36:57Z", actor: "human:operator", revision: 3, executor: "agent:ui-orphans", correlation: "operator-standing-acceptance-2026-11-22"}
- {from: "proposed", to: "active", at: "2026-11-22T15:36:58Z", actor: "human:operator", revision: 4, executor: "agent:ui-orphans", correlation: "operator-standing-acceptance-2026-11-22"}
- {from: "active", to: "implemented", at: "2026-11-22T15:47:41Z", actor: "human:operator", revision: 5, decided_on: {"recorded":{"test_result":1}}, executor: "agent:ui-orphans", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: ui-test-servers-end

## Outcome

The UI test step ends every server and other long-lived child it started when it is ended, also when it
is ended with SIGKILL and no teardown hook runs. A server a person starts by hand behaves as before.

Rule served (`../AGENTS.md`, "Tests stay fast": a test step is a bounded step; a run ended at its bound
must leave nothing behind that holds the checkout, a port or memory).

## Observed

`task test:ui` was ended at its 120 s bound on a loaded host twice on 2026-11-22. Afterwards
`node server.mjs --port 0 ...` processes of that run were still alive, re-parented to the user's service
manager, with their working directory in `ui/`: one for 36 minutes, two for 77 minutes. The worktree tool
refuses to remove a tree a process sits in, and the processes kept ports and memory.

## Cause

Specs started their children with `child_process.spawn` and ended them in `afterAll`. A run ended at its
bound kills the Playwright workers before that hook runs, and a child whose parent died is not ended by
anything.

## Change

- `ui/test/guard.mjs` runs a command in a process group of its own and ends that group with SIGKILL when
  the pid of the process that started it is gone (polled every 100 ms). It forwards SIGTERM, SIGINT and
  SIGHUP to the group. No process is looked up or ended by name.
- `ui/test/child.mjs` (`spawnChild`) starts a command through the guard; `ui/test/serve.mjs`
  (`startServer`) uses it. Every `spawn` of a server, fixture player or daemon in `ui/test/` uses it:
  `serve.mjs`, `daemon-ui.spec.mjs`, `daemon-decide.spec.mjs`, `daemon-decide-review.spec.mjs`,
  `autopilot.spec.mjs`, `now.spec.mjs`, `changes.spec.mjs`, `live.spec.mjs`. The two servers that
  `website/playwright.config.ts` starts run under the same guard.
- The product (`ui/server.mjs`, the `runbook` binary) is unchanged: the guard is test-side only.

## Acceptance

- `ui/test/child.spec.mjs`: a parent process starts the inspector's server through `startServer`, is
  ended with SIGKILL, and the server's pid is gone within 2 s; started without the guard
  (`endWithParent: false`) the server stays until the test ends it by its pid.
- After `slot task test:ui`, no process started by the run is left.
