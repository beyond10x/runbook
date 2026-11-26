---
format: aep.planning-md/3
id: story:relay-embeds-the-gateway
kind: story
status: draft
title: The relay embeds the gateway
revision: 1
---
## Outcome

runbookd uses the llm and llm-gateway crates in process with `llm.routes/1` and drops the HTTP hop to the separate
gateway process: a run's model call goes from the daemon's relay to the route's provider without a second process
between them.

## Scope

- The relay serves a route through the llm-gateway crate in the daemon's process, read from `llm.routes/1`.
- Pod start and stop of a managed route run in process.
- Spend ceilings are kept and answered in process.
- Route status (readiness and phase) is read in process, not over HTTP.

## Not now

Outside the model-routes goal; no work on it yet. The operator's pick of 2026-11-26: keep forwarding to the separate
gateway now, and file this for later.

## Acceptance

To be written when the story is taken up.
