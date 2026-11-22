---
format: aep.planning-md/3
id: story:mcp-exposure
kind: story
status: proposed
title: MCP exposure over stdio and streamable HTTP
summary: The run commands as MCP tools by runbook mcp and runbook serve --mcp, on the one command layer.
tags:
- one-binary
relations:
- decomposes: epic:runbook-one-binary-and-daemon
- depends_on: story:daemon-skeleton-on-a-runtime-trait
scope:
- confidence: cited
  path: runbook/Cargo.toml
- confidence: cited
  path: runbook/src/main.rs
- confidence: inferred
  path: runbook/src/mcp
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:47:34Z", actor: "human:operator", revision: 2, executor: "agent:plan-onebin", correlation: "operator-requirement-2026-11-22-one-binary"}
---
# Story: mcp-exposure

## Outcome

An MCP client can submit, list, read and cancel runs, read events and answer decisions through the
same command layer as the HTTP API, over stdio (`runbook mcp`) and over streamable HTTP
(`runbook serve --mcp`).

## Source

The operator, 2026-11-22: "we also want MCP exposure".

## Context

Design decision 6: HTTP, MCP and the CLI are three faces of one command layer
(`runbook/src/commands.rs` of `story:daemon-skeleton-on-a-runtime-trait`); no handler logic is
written twice. The tools are named after the ESS commands and views.

## Acceptance

- `runbook mcp` answers an MCP `initialize` on standard input with its server info and the tools
  `submit_run`, `list_runs`, `read_run`, `read_events`, `answer_decision`, `cancel_run`; a
  `tools/list` request prints exactly those six names.
- A `tools/call` of `submit_run` against a stub runtime returns a run id, `list_runs` then lists it
  and `cancel_run` moves it to `cancelled`; the results equal the JSON of the HTTP routes for the
  same calls.
- `read_events` of a run returns the events after a given sequence number, in order, with the
  runtime's record unchanged.
- `runbook serve --mcp` serves the MCP endpoint over streamable HTTP at the path the story names in
  `docs/` and answers the same `tools/list`; on a non-loopback listener a request without the
  bearer token gets 401 (the daemon's one token check).
- A tool call with an unknown run id returns an MCP error with the same code as the HTTP 404.
- No test over 60 s, step within 120 s; tests go into the existing `runbook/tests/` binary.

## Scope

`runbook/src/mcp/` (new), `runbook/src/main.rs` (one registration), `runbook/Cargo.toml`,
`runbook/tests/`, `docs/` (one page), `plugin/` only if a skill names the command.
