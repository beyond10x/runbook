---
format: aep.planning-md/3
id: story:a-webhook-receiver-starts-a-run
kind: story
status: draft
title: A webhook receiver starts a run, with the webhook's delivery id as the idempotency key
summary: A receiver beside the daemon checks a forge webhook's signature and submits a run, so a redelivery reaches the same run.
tags:
- forge-gitlab
relations:
- informed_by: story:ci-component-runs-a-runbook-in-a-pipeline-job
scope:
- confidence: cited
  path: docs/daemon-api.md
- confidence: inferred
  path: docs/forge.md
- confidence: inferred
  path: runbook/src/client
- confidence: inferred
  path: runbook/src/main.rs
- confidence: inferred
  path: runbook/tests/daemon
revision: 6
---
## What is missing

A team on a GitLab instance wants an event of the forge (a merge request opened, a pipeline ended, a release published) to start a Runbook run. Nothing receives such an event, and the daemon's own command is not a webhook endpoint.

Read on `main` at `81079535`:

- `submit-run` takes the runbook, the intake as text (at most 1,048,576 bytes, else `IntakeTooLarge`), settings, confinement, autonomy, decider and an `idempotency_key` (`docs/daemon-api.md:48-69`, `docs/mcp.md:44`).
- A key is at most 200 characters and unique per daemon; a second submit with a used key creates no run and answers the first run, `already-submitted`, status 202; a key over the limit is refused `400` before every outcome (`docs/daemon-api.md:62-69`).
- The daemon's routes answer `401` without the bearer token, and it holds no webhook secret (`docs/daemon-api.md:268-276`).

The claims below about the forge are from its public documentation and are to be checked against the documentation of the instance's version before building:

- a webhook can carry a signing token and then sends the headers `webhook-id`, `webhook-timestamp` and `webhook-signature` (`v1,<hmac>`), in the Standard Webhooks scheme; generally available from 19.1 (read);
- the `webhook-id` stays the same across retries of one delivery (read); that it can serve as the idempotency key is an inference;
- a receiver must answer quickly; after four consecutive failures a hook is disabled for a while and after forty for good (read);
- events of jobs can arrive out of order (read).

## Outcome

1. A receiver, a small program beside the daemon and not part of it, accepts a webhook, checks its signature and timestamp, and answers `2xx` at once.
2. For an event its configuration lists, the receiver calls `submit-run` with the runbook, an intake built from the event by a mapping the operator wrote, the autonomy and the decider the operator configured, and the webhook's delivery id as the `idempotency_key`.
3. A redelivery of the same event reaches the same run: the answer `already-submitted` is a success for the receiver and ends in `2xx` for the forge.
4. An event the configuration does not list, one with a bad signature, one outside the timestamp window and one whose intake would pass 1,048,576 bytes starts no run and is answered with a status that says why.

## What a guard must hold

- The signature is checked before anything of the body is read beyond its length; the comparison takes constant time; a timestamp outside a configured window is refused, so a captured delivery cannot be replayed later.
- The body is bounded before it is read whole, and the intake is bounded below the daemon's limit.
- The intake is untrusted text from the forge: the mapping names exactly the members it copies and nothing else; autonomy, decider, confinement and runbook come from the receiver's configuration and never from the payload.
- A delivery id longer than the daemon's key limit is refused by the receiver before the submit; the daemon's token and the webhook secret are in files of mode 600 and in no log.
- The receiver answers `2xx` before the run ends and holds no run open; it does not queue on a disk it cannot bound.

## Open question, with the option recommended

- Whether the receiver is a mode of the `runbook` binary or its own program. Recommended: its own small program, so the daemon and the client stay without the forge's secrets, as in `story:a-runs-end-is-a-commit-status-on-the-forge`.

## Acceptance

Doc claims about the forge first: the note of `story:ci-component-runs-a-runbook-in-a-pipeline-job` lists these claims, checked against the instance's version, including the header names and the delivery id's stability.

Then, against a daemon on the stub runtime:

- a delivery with a valid signature for a listed event starts one run whose idempotency key is the delivery id; the same delivery twice leaves one run and both are answered `2xx`;
- a bad signature, a timestamp outside the window, an unlisted event, a body over the bound and an id over 200 characters each start no run and are answered `4xx` with a named cause;
- the run's autonomy and decider are the receiver's configuration even when the payload names others;
- neither secret is in any output.

## Scope

`docs/daemon-api.md`, `docs/forge.md`, `runbook/src/client`, `runbook/src/main.rs`, `runbook/tests/daemon`.

## Depends on

- Informed by `story:ci-component-runs-a-runbook-in-a-pipeline-job` (the note of forge claims and the way a run carries its commit and project).
