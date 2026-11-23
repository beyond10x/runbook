---
format: aep.planning-md/3
id: story:submit-run-key-is-bound-to-its-content
kind: story
status: draft
title: A used key of submit-run with other content is answered by name, not by the first run
summary: The replay rule of submit-run answers already-submitted with the first run for a used key whatever the content. A note of security pass 2 of the start form, in code older than that unit.
relations:
- informed_by: story:daemon-page-starts-a-run
revision: 1
---
## Outcome

The daemon answers a `submit-run` whose key was used before with other content by a named outcome that says
so, and replays the first run only for the same content.

## Source

Security review pass 2 of `story:daemon-page-starts-a-run` (2026-11-23), a note in code older than that unit:
the replay rule of `submit-run` (`runbook/src/commands.rs`) answers `202 already-submitted` with the first
run for a used key whatever the content; another intake or another runbook under the same key is dropped
without a word. The daemon's page never sends that (a committed case holds it:
`ui/test/security-start-idempotency.spec.mjs`); a client of the API or of MCP can.

## Scope

- `ess/domains/daemon.yaml` (the outcome table of `SubmitRun`), `runbook/src/commands.rs`
  (`submit_run_for`), the daemon's state (what is kept per key: a digest of the content).
- `docs/daemon-api.md`, the daemon's conformance suite.

## Acceptance

- A `submit-run` with a used key and the same content (runbook, intake, inputs, confinement) answers
  `already-submitted` with the first run, as today.
- A `submit-run` with a used key and other content starts nothing and answers a named outcome of its own; the
  first run is unchanged. One test per member of the content.
- The rule survives a restart of the daemon over the same state directory.
- The precedence of the outcome is declared in `ess/domains/daemon.yaml` and the conformance suite covers it.

## Not in this story

- The page's own key rule (one key per filled form), which `story:daemon-page-starts-a-run` holds.
