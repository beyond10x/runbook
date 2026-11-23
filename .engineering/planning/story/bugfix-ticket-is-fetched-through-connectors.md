---
format: aep.planning-md/3
id: story:bugfix-ticket-is-fetched-through-connectors
kind: story
status: draft
title: The ticket of a bug-fix run is fetched from a tracker through the connectors command
summary: runbook run --bugfix-ticket takes a ticket key; one tool of this repository reads it through the connectors command into sdc.ticket/1. No second tracker client.
relations:
- decomposes: epic:ticket-to-merge-request-on-a-real-repository
revision: 1
---
## What is known

`runbook run --bugfix-ticket` reads a ticket file in the form `sdc.ticket/1` that somebody wrote.
`docs/bugfix.md` calls the fetcher a later story. A consumer wrote the file by hand from a tracker search
(2026-11-23).

## Outcome

One tool of this repository reads a ticket by its key through the `connectors` command (the governed way
to an integration on a host) and writes `sdc.ticket/1`: summary, description, the fields the form names,
and the constraints a ticket states. `runbook run --bugfix-ticket <key>` uses it when the argument is a key
and not a file. The tool has no tracker client of its own and holds no token: what `connectors` admits is
what it can read.

## What a guard must hold

- The ticket's text reaches the run directory and the model's intake only; never a console, a log line or
  an event of the runner (the standing rule for a bug-fix run).
- A read that `connectors` refuses ends the run before a run directory exists, by the name of the operation.
- The tool only reads: it has no operation that writes to the tracker.

## Acceptance

A case with a stand-in `connectors` on its own `PATH` shows the key becoming an `sdc.ticket/1` file with
invented text, the refusal path, and that no line of the ticket is in the runner's output.

## Origin

A gap named by a consumer on 2026-11-23; announced in `docs/bugfix.md` as a later story.
