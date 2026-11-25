---
format: aep.planning-md/3
id: story:delegation-over-a2a
kind: story
status: active
title: A delegation is reachable over A2A
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-25T20:25:17Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-25T20:25:17Z", actor: "agent:runbook", revision: 3}
---
## Outcome

The delegation of section 8 is served as an A2A task through the a2a repository's server crate.

## Acceptance

- An A2A client sends a task with request text and a route and gets the run's result as the task's artifact.

Design: `docs/model-routes.md`, section 8. Tests use a stand-in gateway on loopback: no model call, no pod, no network beyond loopback.
