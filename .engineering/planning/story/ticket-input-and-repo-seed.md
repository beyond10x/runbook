---
format: aep.planning-md/3
id: story:ticket-input-and-repo-seed
kind: story
status: active
title: Bug-fix runbook started from a ticket document on an existing repository
summary: sdc-bugfix seed --repo --ticket takes an sdc.ticket/1 document and a clean git worktree; run-trial.sh --bugfix-ticket starts the run.
tags:
- conductor-succession
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: docs/bugfix.md
- confidence: cited
  path: driver/examples/sdc/fixtures
- confidence: cited
  path: driver/examples/sdc/tools/llm-node.sh
- confidence: cited
  path: driver/src/bin/sdc-bugfix.rs
- confidence: cited
  path: scripts/run-finding-cycles.sh
- confidence: cited
  path: scripts/run-trial.sh
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T19:36:49Z", actor: "human:operator", revision: 2, executor: "agent:ticket-input-and-repo-seed", correlation: "operator-standing-acceptance-2026-11-21"}
- {from: "proposed", to: "active", at: "2026-11-21T19:36:53Z", actor: "human:operator", revision: 4, executor: "agent:ticket-input-and-repo-seed", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: ticket-input-and-repo-seed

## Outcome

The bug-fix runbook `sdc-bugfix` can be started on an existing git repository from a ticket
document (`sdc.ticket/1`), instead of on a hand-made seed directory with a typed bug report. A
controller outside (a later story) writes the ticket document; this story defines the document and
makes the runbook take it.

## Context

The operator, 2026-11-21: "then I can actually point it to say Jira ticket and then it will just do
it. Maybe we can have external outside controller session, which is maybe first fetching the ticket
context, and the repo context and such, and then starting the runbook with a certain protocol."

## Scope

- `sdc.ticket/1`: YAML, strict validation, its JSON schema printed by `sdc-bugfix schema ticket`.
- `sdc-bugfix seed --repo <worktree> --ticket <file>`: refuses a dirty worktree, a HEAD other than
  `repository.base.commit`, a repository without `Cargo.toml` at its root, and a tree above the
  size cap; copies exactly what `git ls-files` lists; writes `bug.yaml` from the ticket.
- `llm-node.sh`: a ticket run drops "Standard library only" and adds the ticket's constraints; a
  run without a ticket keeps today's prompt byte for byte.
- `scripts/run-trial.sh --bugfix-ticket <file>`.
- A section in `docs/bugfix.md`.

Not in scope: a hand-back branch, the `runbook` crate, use of `check.argv`, a feature protocol, a
fetch from any ticket system.

## Acceptance

- Unit tests of `sdc-bugfix`: every validation refusal, the `git ls-files` copy (an ignored and an
  untracked file are not copied), dirty and wrong-HEAD refusals, the size cap, the `bug.yaml`
  content.
- The cycle `bugfix-from-ticket` of `scripts/run-finding-cycles.sh` (stub adapter, autopilot,
  through `scripts/run-trial.sh --bugfix-ticket`) closes `fixed`; the existing bug-fix cycles pass
  unchanged.
- `task check` exits 0.

## Privacy

Ticket text reaches prompts and local run records only: never a commit message, a branch name, a
line printed at default verbosity, or a committed file. Tests use invented tickets.
