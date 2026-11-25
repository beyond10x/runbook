---
format: aep.planning-md/3
id: story:client-token-carries-its-principal
kind: story
status: draft
title: A client token carries its own principal; design first, to the operator before code
summary: One principal per daemon records every answer under the same name; a token file with a principal per token lets two clients answer as two principals.
tags:
- consumer-asks-2026-11-25
relations:
- depends_on: story:daemon-runs-take-a-gateway
- informed_by: story:unattended-run-records-no-human-actor
- informed_by: story:amendment-keeps-person-decisions
scope:
- confidence: cited
  path: docs/daemon-api.md
- confidence: cited
  path: docs/mcp.md
- confidence: cited
  path: ess/domains/daemon.yaml
- confidence: inferred
  path: ess/generated
- confidence: inferred
  path: runbook/src/api/auth.rs
- confidence: inferred
  path: runbook/src/api/mod.rs
- confidence: cited
  path: runbook/src/commands.rs
- confidence: inferred
  path: runbook/src/config.rs
- confidence: inferred
  path: runbook/src/serve.rs
- confidence: inferred
  path: runbook/tests/daemon
revision: 6
---
## What is missing

The daemon acts as one principal. `--principal` is required at the start and fixed for the life of the process (`docs/daemon-api.md:253`; `runbook/src/commands.rs:61-63`); every answer is handed to the runtime under it (`runbook/src/commands.rs:867-885`); `answered_by` is a label the client typed and nothing else, and a `principal` member in an answer's body is not read (`docs/daemon-api.md:77-85`). The bearer token decides only who may call: one token from `--token-file` or `RUNBOOK_TOKEN`, no principal attached (`docs/daemon-api.md:9-11`, `:262`, `:268-276`; `runbook/src/api/auth.rs`). So two clients of one daemon, for example two people or a person and an agent, answer decisions under the same principal and the record cannot tell them apart; the contract expects the principal who decides to be one of the request's eligible principals (`ess/domains/decision.yaml:239`), which the daemon cannot check for a client.

## Outcome

A client token carries a principal. The token file names several tokens, each with a principal (`person:<id>` or `agent:<id>`, the form `--principal` takes, `runbook/src/commands.rs:38-49`); a request carries the principal of the token that authenticated it; an answer is handed to the runtime under that principal and recorded under it in the `DecisionAnswered` event, the row of the view `decisions`, the answer of the MCP tool and the journal (`docs/daemon-api.md:77-85`). Two clients with two tokens answer under two principals.

## This story is design first

The change touches who may answer a decision. The first part of the story is a design (a `design` artifact related to this story with `designs`, and its text in `docs/daemon-api.md` once accepted), put to the operator before any code is written; the story stays at `draft` and no code starts until the operator has accepted the design. The design answers, each with the option it recommends:

- the token file's format, and how a token with no principal and the `--principal` flag combine;
- whether the daemon also checks that an answering principal is an eligible principal of the request, or only records it;
- the UI's login cookie (`docs/daemon-api.md:449-521`), which a token hands over once: it carries the principal of the token;
- the MCP endpoint and `runbook mcp --daemon` (`runbook/src/mcp/remote.rs`), which pass a token through;
- the view `configuration`, which names the principal today (`docs/daemon-api.md:77-85`), and what it names with several;
- how a token is added or revoked (a restart, or a reload), and what a journal written before this change shows.

## What a guard must hold

- A principal comes from the server-side token file and from nowhere a client controls: no header, no body member, no MCP argument sets it (the rule of `docs/daemon-api.md:80-83`).
- A token and its principal are never logged, echoed or written to the journal beyond the principal; the comparison of a presented token against every configured token takes constant time and does not tell which one matched; the file's mode and token length are checked as for the one token today (`docs/daemon-api.md:270-276`).
- With one token and `--principal`, and with no token, the daemon behaves as today, byte for byte; a journal written before this change still loads (`docs/daemon-api.md:83-85`).
- The names `rule:`, `decider:` and `router:` stay the daemon's: no token carries a principal of those forms (`docs/daemon-api.md`, the label rule).

## Acceptance

Part one: the design artifact exists, names the recommended option of each question above, and carries an approval from the operator recorded against it; the story is not moved while it does not.

Part two, after that: with a token file of two tokens, `person:a` and `agent:b`, an answer sent with the first token is recorded with the principal `person:a` in the `DecisionAnswered` event, the view `decisions` and the journal, and an answer to another request sent with the second token with `agent:b`; a request with a token the file does not hold is `401`; the label `answered_by` is unchanged; a daemon started with one token and `--principal` records that principal as before; a restart over the same state directory shows both principals in the old answers.

## Scope

`runbook/src/api/auth.rs`, `runbook/src/api/mod.rs`, `runbook/src/commands.rs`, `runbook/src/serve.rs`, `runbook/src/config.rs`, `ess/domains/daemon.yaml`, `ess/generated`, `docs/daemon-api.md`, `docs/mcp.md`, `runbook/tests/daemon`.

## Depends on

- `story:daemon-runs-take-a-gateway`: the order is behind the gateway release.
- Informed by `story:unattended-run-records-no-human-actor` (who a record names as actor) and `story:amendment-keeps-person-decisions`.
- Order: sixth and last: it needs the operator's answer on its design first, and the five before it do not wait for it.
