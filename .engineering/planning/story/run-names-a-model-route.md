---
format: aep.planning-md/3
id: story:run-names-a-model-route
kind: story
status: active
title: A run names a model route
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-25T20:25:15Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-25T20:25:15Z", actor: "agent:runbook", revision: 3}
---
## Outcome

A submit names model_route and model_routes.<role>; precedence run, protocol, route_policy.default; the run record names route, alias and source; runbook run --model-route.

## Acceptance

- Runs naming A, B and none reach A's alias, B's alias and the default's.
- An undeclared route is refused 422 route-refused listing the declared routes; an address, token or alias in a submit is refused by name; nothing is written.
- A run with no model step that names a route is refused.

Design: `docs/model-routes.md`, section 2. Tests use a stand-in gateway on loopback: no model call, no pod, no network beyond loopback.
