---
format: aep.planning-md/3
id: story:daemon-catalog-lists-what-a-delegation-may-name
kind: story
status: draft
title: The daemon lists what a delegation may name
revision: 1
---
## Outcome

GET /daemon/views/catalog and MCP describe_catalog list the routes (id, kind, wires, status), the runbooks (id, one-line purpose, inputs, route admission) and the delegation members.

## Acceptance

- The catalog of a daemon with two routes and two runbooks lists both of each with those fields.
- A runbook whose route entries cannot be admitted is listed with the reason.

Design: `docs/model-routes.md`, section 10. Tests use a stand-in gateway on loopback: no model call, no pod, no network beyond loopback.
