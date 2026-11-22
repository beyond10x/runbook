---
format: aep.planning-md/3
id: story:daemon-api-specified-in-ess
kind: story
status: active
title: The daemon API specified in ESS
summary: Entities, commands, views and the event stream of the daemon as an ESS domain, with generated OpenAPI and schemas and a conformance suite.
tags:
- one-binary
relations:
- decomposes: epic:runbook-one-binary-and-daemon
scope:
- confidence: cited
  path: ess/domains
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: inferred
  path: ess/generated
- confidence: cited
  path: ess/scenarios
- confidence: cited
  path: ess/system.yaml
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:47:31Z", actor: "human:operator", revision: 2, executor: "agent:plan-onebin", correlation: "operator-requirement-2026-11-22-one-binary"}
- {from: "proposed", to: "active", at: "2026-11-22T01:47:31Z", actor: "human:operator", revision: 3, executor: "agent:plan-onebin", correlation: "operator-requirement-2026-11-22-one-binary"}
---
# Story: daemon-api-specified-in-ess

## Outcome

The daemon's API is specified in ESS before any handler exists: its entities, commands, views and
event stream are a domain under `ess/`, and the OpenAPI document and JSON Schemas are generated
from it, never written by hand.

## Source

The operator, 2026-11-22: "All of this should be ESS specified, of course." and "there should also
be a runbook daemon, runbookd. It has a simple API where we submit something, and then we seek
events over SSE."

## Context

`ess/` holds the specification of the decision contract (domain `sdc.decision`, `ess/system.yaml`,
`ess/ess-inputs.yaml`). The new domain adds the nouns the plan introduces: run, session, decision
request, event, confinement level. Design decisions 4, 8 and 10 of `epic:runbook-one-binary-and-daemon`
fix the content: run states, submit with a confinement member and a session name, list, read,
cancel, list and answer decision requests, sessions, three SSE streams (run, session, daemon) with
ids and `Last-Event-ID` resume, an event that carries the runtime's record unchanged plus run id
and sequence number, bearer-token refusal on every route. Relations between the nouns (a run
belongs to a session, a decision request to a run) are `relations:` entries, and one whose
cardinality is not known is marked `UNMAPPED:`.

## Acceptance

- `ess specify validate --path ess/system.yaml` exits 0 with the new domain listed in `ess/system.yaml`
  and `ess/ess-inputs.yaml`.
- The domain declares the commands `submit-run`, `cancel-run`, `answer-decision`, `open-session`
  and `close-session`, the views `list-runs`, `read-run`, `list-decisions`, `list-sessions`, and the
  event stream with its id and resume rule; `grep -c 'command:' ess/domains/<new domain>.yaml` is
  at least 5.
- The generated OpenAPI document and JSON Schemas are committed under `ess/generated/` (or the
  path the `ess` command writes), and re-running the generation command produces no diff
  (`git diff --exit-code ess/generated` exits 0).
- At least one scenario per command under `ess/scenarios/` and one for the refusal of a level
  below the floor, one for a request without the token on a non-loopback listener, and one for
  resume after a given `Last-Event-ID`; `ess` synthesizes the conformance suite from them and the
  suite document is committed.
- Every `UNMAPPED:` marker is listed in the story's report; none is a guessed cardinality.
- The step that validates and generates ends within 120 s, and the generation is a committed
  file with a digest of its sources, not a call inside each test (`AGENTS.md`, "Tests stay fast").

## Scope

`ess/system.yaml`, `ess/ess-inputs.yaml`, `ess/domains/` (new domain file), `ess/scenarios/`
(new), `ess/generated/` (new), `Taskfile.yml` (a check target). No code in `runbook/`.
