---
format: aep.planning-md/3
id: story:daemon-declares-model-routes
kind: story
status: draft
title: The daemon declares named model routes and lists them
revision: 1
---
## Outcome

runbook.toml declares [gateways.<name>], [routes.<name>] (gateway, alias, kind managed|hosted|local, prices, start_wait_seconds) and [route_policy]; [gateway] is read as [gateways.default]. GET /daemon/views/routes and MCP list_routes list each route with alias, kind, wires and status.

## Acceptance

- Two routes on one stand-in gateway are listed with alias, kind and wires.
- A route naming an undeclared gateway, a gateway without base_url, or a token in the file is refused at start by key, never by value.
- A [gateway] table alone works as today and lists as gateways.default.

Design: `docs/model-routes.md`, section 1. Tests use a stand-in gateway on loopback: no model call, no pod, no network beyond loopback.
