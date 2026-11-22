# MCP

The run commands of the daemon as MCP tools, over standard input and output (`runbook mcp`) and over
streamable HTTP (`runbook serve --mcp`). MCP is the third face of the daemon's command layer, with the
HTTP API ([`daemon-api.md`](daemon-api.md)) and the command line: a tool calls the function the HTTP
route of the same name calls, and its result is the JSON body that route answers.

## Revision

The server implements the MCP specification revision **2026-07-28**, the current one
(`https://modelcontextprotocol.io/specification/2026-07-28`), checked on 2026-11-22. That revision has
no `initialize` handshake: every request carries its protocol version and the client's capabilities in
`_meta`, and `server/discover` tells a client what the server speaks.

The server is dual-era, as that revision's "Versioning and Compatibility" page allows. A client of the
earlier revisions `2025-11-25` and `2025-06-18` opens with `initialize` and
`notifications/initialized`, and is answered in the shape of those revisions. The era is chosen per
request by how the client opens: a request with `_meta["io.modelcontextprotocol/protocolVersion"]` is
served as `2026-07-28`; `initialize` selects the legacy shape.

| Method | Era | Answer |
|---|---|---|
| `server/discover` | 2026-07-28 | `supportedVersions` (`2026-07-28`, `2025-11-25`, `2025-06-18`), `capabilities` (`tools`), `instructions`, `_meta` with the server's name and version |
| `initialize` | 2025-11-25, 2025-06-18 | `protocolVersion` (the client's, when it is one of the two; else `2025-11-25`), `capabilities.tools`, `serverInfo` (`runbook` and the version), `instructions` |
| `notifications/initialized`, any other notification | legacy | accepted, no answer |
| `ping` | both | `{}` |
| `tools/list` | both | the six tools below |
| `tools/call` | both | the result of the tool |

A request of another protocol version is the error `-32022` (`UnsupportedProtocolVersion`) with
`data.supported` and `data.requested`. A modern request without `clientCapabilities` is `-32602`; so
is any other request before a legacy client has sent `initialize`. An unknown method is `-32601`, an
unknown tool or an argument the tool does not take `-32602`. A modern result carries
`resultType: "complete"`; a legacy result is bare. Not built: pagination (the list has six tools),
`ttlMs` and `cacheScope`, `subscriptions/listen`, multi round-trip requests, cancellation of a call,
progress, resources, prompts. The server sends no request and no notification of its own.

## The tools

`tools/list` prints exactly these six, in this order:

| Tool | Calls | Input |
|---|---|---|
| `submit_run` | command `submit-run` | `runbook_id`, `intake`; optional `runbook_version`, `context_path`, `settings`, `confinement`, `session_id`, `idempotency_key` |
| `list_runs` | views `runs`, `runs-of-session`, `runs-in-state`, `runs-of-session-in-state` | optional `session_id` and `state`; the view follows from which are given |
| `read_run` | view `run` | `run_id` |
| `read_events` | view `events` | `run_id`; optional `after_sequence` and `after_position` (both default to 0) |
| `answer_decision` | command `answer-decision` | `run_id`, `request_id`, `answer`, `answered_by` (a label; the principal of the answer is the daemon's own, in the result as `principal`, and no argument sets it) |
| `cancel_run` | command `cancel-run` | `run_id` |

Input schemas are derived from the generated OpenAPI document of the daemon
(`ess/generated/openapi/openapi/runbookd.yaml`, compiled into the binary): a command tool's schema is
the `Input` schema of its command with the members the HTTP handler fills removed (`run_id`,
`daemon_id`, `runbook_known`, `context_inside_roots` of a submit, and `at` of all) and with the
members the daemon defaults (`settings`, `confinement`) not required; a view tool's schema is built
from the query parameters of its view. Every `$ref` is inlined. The one member typed here is
`after_sequence` of `read_events`, which has no parameter in the view: the view pages by `position`;
a run's own `sequence` (from 1) is the filter the tool puts on the rows of that view.

A result is

```json
{"content": [{"type": "text", "text": "<the body>"}],
 "structuredContent": <the body>,
 "isError": false,
 "_meta": {"runbook/http-status": 202}}
```

where the body is the JSON the HTTP route answers, unchanged: `outcome` and `published` for a command
(`error` and `payload` for a refusal), `rows` for a view. `isError` is true when the route's status is
400 or more, and `_meta["runbook/http-status"]` is that status, so a refusal has the code the HTTP
route gives it: a `cancel_run` of an unknown run is `404` (`NoSuchRun`), a second answer is `409`.
This is a tool execution error in the sense of the specification, in the result and not a JSON-RPC
error: a model that reads it can correct itself.

Two tools are stricter than their route. `read_run` and `read_events` of a run id the daemon does not
know answer `404` with `{"refused": "no such run", ...}`, where the views answer `200` with no row.

Tools take no path the HTTP command does not take: `context_path` is the same member, held against the
daemon's context roots by the same handler. A result carries what the route carries and no more.

## Standard input and output: `runbook mcp`

```
runbook mcp --daemon http://127.0.0.1:7430 [--token-file FILE]
```

One JSON-RPC message per line on standard input; one per line on standard output, and nothing else
there. Messages and diagnostics for a person go to standard error. The process ends when standard
input ends.

It is a client of a running daemon: `--daemon <url>` or `RUNBOOK_DAEMON`, `http://host[:port]` only (no
TLS client in this binary). With no daemon named it exits 1 with a message: one daemon owns the state
of the runs, and a second process on its directory would not be that daemon. Each tool call is the
HTTP request of its route, so a result over standard input and output is the body the route answered.
A daemon that does not answer is a tool error with status `502` and `daemon unreachable`.

The token is read from `--token-file` (mode `600`) or `RUNBOOK_TOKEN`, never from an argument; it
appears in no message, no log and no error. A token for a host that is not loopback goes in clear over
`http://`; the command says so on standard error.

Registration with a client, for example Claude Code:

```
claude mcp add runbook --env RUNBOOK_DAEMON=http://127.0.0.1:7430 -- runbook mcp
```

## Streamable HTTP: `runbook serve --mcp`

`runbook serve --mcp` also serves the endpoint **`POST /mcp`** on the daemon's listen address. It is
the streamable HTTP transport of `2026-07-28`, without the parts that revision removed:

- Each POST is one request or notification. A request is answered with one `application/json` object
  (no event stream); a notification with `202` and no body.
- GET and DELETE are `405` with `Allow: POST`. An `Mcp-Session-Id` or `Last-Event-ID` header is ignored;
  no session id is minted.
- A modern request carries `MCP-Protocol-Version` and `Mcp-Method` (and `Mcp-Name`, the tool name, for
  `tools/call`; a name outside plain ASCII is accepted in the `=?base64?...?=` form). A header that
  is missing or differs from the body is `400` with `-32020` (`HeaderMismatch`); an unsupported version
  is `400` with `-32022`; an unknown method is `404` with `-32601`. A legacy client sends
  `MCP-Protocol-Version: 2025-11-25` (or `2025-06-18`) after its `initialize`, which needs no header.
- The daemon's one guard refuses a foreign `Host` or `Origin` with `403` (DNS rebinding) and a POST that
  is not `Content-Type: application/json` with `415`, before the endpoint runs (`docs/daemon-api.md`,
  "Requests from a browser"). A client that is not a browser sends no `Origin`.
- The body limit is the daemon's (`413`).

The endpoint is behind the daemon's one token check, the function that guards the API routes: with a
token (`--token-file` or `RUNBOOK_TOKEN`), a POST without `Authorization: Bearer <token>` is `401`
before anything is read, and a listen address that is not loopback needs a token to start. Without
`--mcp` there is no `/mcp`: the path is `404`.

```
curl -s http://127.0.0.1:7430/mcp \
  -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
  -H 'MCP-Protocol-Version: 2026-07-28' -H 'Mcp-Method: tools/list' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}'
```
