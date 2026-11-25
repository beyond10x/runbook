---
format: aep.planning-md/3
id: story:claude-plugin-serves-delegation
kind: story
status: draft
title: A Claude Code session with the Runbook plugin delegates through Runbook
revision: 1
---
## Outcome

The plugin in plugin/ gains an MCP server (runbook mcp with delegate, delegation_result, describe_catalog), a PreToolUse hook that denies every Agent call with a reason naming delegate and the session's route, and a skill and SessionStart line saying the same. Enabled per session with --plugin-dir or --settings.

## Acceptance

- The hook denies an Agent call with a reason naming the delegate tool and RUNBOOK_MODEL_ROUTE; it makes no network call.
- A headless session against a stand-in Messages server delegates through delegate and gets the run's result back (the consumer's test 7).
- A down route gives a tool error naming the route; no Agent call runs.

Design: `docs/model-routes.md`, Part two. Tests use a stand-in gateway on loopback: no model call, no pod, no network beyond loopback.
