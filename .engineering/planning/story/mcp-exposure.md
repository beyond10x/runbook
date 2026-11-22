---
format: aep.planning-md/3
id: story:mcp-exposure
kind: story
status: active
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
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:47:34Z", actor: "human:operator", revision: 2, executor: "agent:plan-onebin", correlation: "operator-requirement-2026-11-22-one-binary"}
- {from: "proposed", to: "active", at: "2026-11-22T09:21:00Z", actor: "human:operator", revision: 5, executor: "agent:impl-mcp", correlation: "operator-standing-acceptance-2026-11-22"}
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

## Implementation (impl/daemon-mcp)

Built on `impl/daemon-skeleton` at `7965b98b`. Files: `runbook/src/mcp/` (`rpc`, `tools`, `schema`, `http`, `stdio`, `remote`), `runbook/tests/daemon/mcp.rs`, `docs/mcp.md`; in shared files only the registration lines (`main.rs` one subcommand arm, `serve.rs` the `--mcp` flag and one call, `lib.rs` one `mod`) and a small refactor of `runbook/src/api/routes.rs` and `api/mod.rs` so that an MCP tool and an HTTP route call one function (`routes::command_answer` and `view_answer` return the status and the JSON body; `command` and `view` wrap them; `api::serve_extra` mounts the MCP router behind the same token check). No new dependency.

Decisions:

- Revision. The current revision of the MCP specification is 2026-07-28 (looked up 2026-11-22). It has no `initialize`: every request carries the protocol version and the client capabilities in `_meta`, and `server/discover` replaces the handshake. The acceptance names `initialize`, so the server is dual-era, as that revision's Versioning page allows: a request with the per-request `_meta` is served as 2026-07-28; `initialize` (revisions 2025-11-25 and 2025-06-18) is answered with the server info and the tools in the legacy shape. `docs/mcp.md` names both.
- The stdio server is a client of a running daemon (`--daemon <url>` or `RUNBOOK_DAEMON`, `http://` only) and refuses to start with a message when none is named: the story does not say, and one daemon must own the state of the runs. Each tool call is then the HTTP request of its route, so the result is the route's body.
- Schemas come from the generated OpenAPI document (compiled in with `include_str!`), not from Rust types and not typed by hand. The only member typed in `mcp/tools.rs` is `after_sequence` of `read_events` (the view pages by position; the story asks for a sequence number, which the tool applies as a filter on the rows of the view).
- A refusal is a tool execution error (`isError: true`) with the route's body, and `_meta["runbook/http-status"]` carries the HTTP status. "The same code as the HTTP 404" is that status; the JSON-RPC error `-32602` is kept for an unknown tool or an argument the schema does not have.
- `read_run` and `read_events` of an unknown run id answer 404 (the views answer 200 with no row); the one place a tool is stricter than its route.
- The HTTP endpoint is `POST /mcp` (`runbook serve --mcp`), no stream, no session.

Acceptance, with the test that holds it (`runbook/tests/daemon/mcp.rs`; times from `slot task test:runbook`):

- `initialize` answers server info and the tools; `tools/list` prints exactly the six names: `stdio_initialize_answers_server_info_and_the_six_tools` (0.55 s), `stdio_a_client_of_the_current_revision_discovers_and_lists_exactly_six_tools` (0.24 s).
- submit, list, cancel; results equal the JSON of the HTTP routes: `stdio_submit_list_and_cancel_give_what_the_http_routes_give` (0.80 s).
- `read_events` after a sequence number, in order, record unchanged: `stdio_read_events_returns_the_events_after_a_sequence_in_order_unchanged` (0.76 s).
- `serve --mcp` serves the endpoint and the same `tools/list`; 401 without the token on a non-loopback listener: `http_endpoint_answers_the_same_tools_list_as_stdio` (0.41 s), `http_endpoint_without_the_bearer_token_gets_401_on_a_non_loopback_listener` (0.35 s), `serve_with_the_flag_serves_the_endpoint_and_without_it_there_is_none` (1.5 s).
- unknown run id: an MCP error with the status of the HTTP 404: `an_unknown_run_id_is_an_error_with_the_status_of_the_http_404` (1.7 s).
- no test over 60 s, step within 120 s, tests in the existing binary: the slowest test of the module is 2.3 s; the step ran 195 tests in 20 s.

Not built: pagination, `ttlMs` and `cacheScope`, `subscriptions/listen`, multi round-trip requests, cancellation and progress, resources and prompts, a TLS client for `--daemon`, an in-process daemon for `runbook mcp`, the tools `open_session` and `close_session` (the story names six).
