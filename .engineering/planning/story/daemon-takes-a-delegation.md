---
format: aep.planning-md/3
id: story:daemon-takes-a-delegation
kind: story
status: active
title: The daemon takes a delegation and returns its result by run id
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-25T20:25:16Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-25T20:25:16Z", actor: "agent:runbook", revision: 3}
---
## Outcome

POST /daemon/commands/delegate and MCP delegate take {request, model_route, protocol?, workspace?}, answer 202 with the run id; GET /daemon/views/delegation-result and MCP delegation_result answer state, outcome, result text, products, route, path, tokens and cost. runbook delegate is a client of it. A delegation without a protocol and without classify runs the free run.

## Acceptance

- A delegation naming a protocol runs it with the request as intake; the result is readable by run id.
- A free run writes result.md and its text is in the result.
- The refusals of submit-run and route-refused/route-unavailable apply.

Design: `docs/model-routes.md`, section 8. Tests use a stand-in gateway on loopback: no model call, no pod, no network beyond loopback.
