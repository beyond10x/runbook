---
format: aep.planning-md/3
id: story:daemon-page-shows-the-route
kind: story
status: active
title: The daemon page shows a run's route and the daemon's routes
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-25T20:25:17Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-25T20:25:17Z", actor: "agent:runbook", revision: 3}
---
## Outcome

The run header names route, kind and status (waiting-for-route while it waits); a Routes view lists each route with alias, kind, status and today's calls and booked spend.

## Acceptance

- Playwright against a daemon with two stand-in routes: the header shows the run's route; the Routes view lists both.
- The 4-requests-a-second bound of a refusing daemon holds with the Routes view open.

Design: `docs/model-routes.md`, section 6. Tests use a stand-in gateway on loopback: no model call, no pod, no network beyond loopback.
