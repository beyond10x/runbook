---
format: aep.planning-md/3
id: story:route-readiness-before-a-run
kind: story
status: draft
title: A run starts only on a route that can answer, else waits or is refused
revision: 1
---
## Outcome

Before a run starts: /ready, /v1/models, and the route's status (callable); a managed or local route that is not callable makes the run wait in waiting-for-route up to start_wait_seconds, then end stopped: route. No wake.

## Acceptance

- /ready 503 refuses the submit route-unavailable; an alias not listed refuses it route-refused.
- A route turning callable during the wait starts the run with no call before; past the wait the run ends stopped: route naming the phase.
- No wake is sent.

Design: `docs/model-routes.md`, section 3. Tests use a stand-in gateway on loopback: no model call, no pod, no network beyond loopback.
