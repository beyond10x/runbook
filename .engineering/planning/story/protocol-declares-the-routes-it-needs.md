---
format: aep.planning-md/3
id: story:protocol-declares-the-routes-it-needs
kind: story
status: active
title: A runbook declares the routes it needs and the daemon admits them
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-25T20:25:16Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-25T20:25:16Z", actor: "agent:runbook", revision: 3}
---
## Outcome

A runbook may carry <runbook>.routes.yaml (runbook.routes/1): per role a daemon route by name or requirements (wire, min_context, tool_calling, kind), and routes_allowed; the daemon admits or refuses each entry by name.

## Acceptance

- A role naming a declared route is admitted; an undeclared one is refused naming runbook and role.
- Requirements met by one route are admitted; met by none or several are refused naming the requirement.
- A route outside routes_allowed is refused at submit.

Design: `docs/model-routes.md`, section 2 (D-1). Tests use a stand-in gateway on loopback: no model call, no pod, no network beyond loopback.
