---
format: aep.planning-md/3
id: story:daemon-relays-and-books-model-calls
kind: story
status: draft
title: Every model call of a run goes through the daemon's relay and is booked
revision: 2
---
## Outcome

runbookd relays a run's model calls (/daemon/relay/<run>/v1/...) to its route's gateway with the alias as model, holds the gateway credential, checks caps per call, writes one ledger line per call (route, alias, tokens, cost) and gives every model process of the run route.json through RUNBOOK_ROUTE_FILE.

## Acceptance

- A call through the relay reaches the stand-in with the route's alias and books one line with tokens and cost.
- A role without a route is refused 400 route-refused; a call past max_llm_calls is refused 429 cap-reached; neither reaches the stand-in.
- A gateway refusal fails the step route-unavailable naming the route, the gateway's code and the setting that chose it; no other route is called.
- No process of the run holds the gateway's credential.

Design: `docs/model-routes.md`, sections 4, 5 and 11 (D-7). Tests use a stand-in gateway on loopback: no model call, no pod, no network beyond loopback.
