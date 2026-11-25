---
format: aep.planning-md/3
id: story:mcp-opens-and-closes-sessions
kind: story
status: draft
title: MCP tools open_session and close_session
summary: The two session commands exist over HTTP and submit_run takes session_id; the MCP tool list has no way to open or close a session.
tags:
- consumer-asks-2026-11-25
relations:
- depends_on: story:daemon-runs-take-a-gateway
- informed_by: story:mcp-exposure
scope:
- confidence: cited
  path: docs/mcp.md
- confidence: cited
  path: runbook/src/mcp/rpc.rs
- confidence: inferred
  path: runbook/src/mcp/schema.rs
- confidence: cited
  path: runbook/src/mcp/tools.rs
- confidence: inferred
  path: runbook/tests/daemon
revision: 6
---
## What is missing

The daemon already has the commands `open-session` and `close-session` over HTTP (`runbook/src/api/routes.rs:14` and `:95-96`; refusals `SessionExists`, `NoSuchSession`, `SessionStateConflict` in `docs/daemon-api.md:50-51`), and the MCP tool `submit_run` already takes `session_id` (`docs/mcp.md:44`, `runbook/src/mcp/tools.rs:43`). What no MCP client can do is open or close a session: the seven tools are `submit_run`, `list_runs`, `read_run`, `read_events`, `read_open_decisions`, `answer_decision` and `cancel_run` (`docs/mcp.md:40`), and `story:mcp-exposure` lists "the tools `open_session` and `close_session`" as not built (its "Not built" line). So an MCP client can name a session in a submit only after another client opened it over HTTP.

Two texts are also stale and the story fixes them with the change: the server's instructions string names six tools (`runbook/src/mcp/rpc.rs:24`), and `runbook/src/mcp/tools.rs:1` says "The seven tools".

## Outcome

`tools/list` of `runbook mcp` and of `runbook serve --mcp` prints nine tools: the seven and, in this order after them, `open_session` (`session_id`, `client`) and `close_session` (`session_id`). Each calls the command of the same name through the one command layer (`runbook/src/commands.rs:656-680`), with the input schema derived from the generated OpenAPI document like every other command tool (`docs/mcp.md`, "The tools"). The HTTP commands need no change.

## What a guard must hold

- No handler logic is written twice: the tools call the command layer, and their results equal the JSON of the HTTP routes for the same calls (the rule of `story:mcp-exposure`).
- A refusal keeps its meaning: `session-exists` is `isError` with the HTTP status 422 in `_meta["runbook/http-status"]`, `no-session` 404, `wrong-state` 409.
- `client` stays a display label that grants nothing (`docs/daemon-api.md:9-11`); the same checks as the HTTP route apply to it.
- A `submit_run` with the id of a closed session is refused `SessionClosedToRuns`, as over HTTP.
- `runbook mcp --daemon` (the remote path, `runbook/src/mcp/remote.rs`) reaches the two tools the same way, with the token the other tools use.

## Acceptance

Against the stub runtime, over stdio: `tools/list` prints exactly the nine names in order; `open_session` with `session_id` `s1` returns the HTTP route's body; `submit_run` with `session_id` `s1` returns a run id and `list_runs` with `session_id` `s1` lists it; a second `open_session` of `s1` is `isError` with status 422; `close_session` of `s1` answers closed and a further `submit_run` with `s1` is `isError` (`SessionClosedToRuns`); `close_session` of `s1` again is `isError` with 409. The same sequence over `runbook serve --mcp` gives the same bodies. `docs/mcp.md` and the instructions string name nine tools.

## Not in this story

A tool for the view `sessions` (listing sessions) is not asked and not added; the view is read over HTTP.

## Scope

`runbook/src/mcp/tools.rs`, `runbook/src/mcp/rpc.rs`, `runbook/src/mcp/schema.rs`, `docs/mcp.md`, `runbook/tests/daemon`.

## Depends on

- `story:daemon-runs-take-a-gateway`: the order is behind the gateway release; there is no code dependency.
- Informed by `story:mcp-exposure` (the tool layer and the line that names these two tools as not built).
- Order: second, after `story:decision-options-come-from-a-case-artifact`: a consumer needs it for the step that submits a run in a named session.
