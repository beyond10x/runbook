---
format: aep.planning-md/3
id: story:submitrun-outcome-order
kind: story
status: active
title: SubmitRun declares no-session above below-floor
summary: 'Notice of the ESS session, 2026-11-22: the next ESS release refuses the declaration order of two outcomes of SubmitRun; the pinned 0.97.4 accepts it.'
tags:
- backlog-2026-11-22-w1
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: docs/daemon-api.md
- confidence: cited
  path: ess/domains/daemon.yaml
- confidence: cited
  path: ess/generated
- confidence: cited
  path: ess/scenarios
- confidence: cited
  path: runbook/src/commands.rs
- confidence: cited
  path: runbook/tests/daemon
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T02:38:32Z", actor: "human:operator", revision: 2, executor: "agent:runbook", correlation: "host-operator-2026-11-23-ess-0-98-0"}
- {from: "proposed", to: "active", at: "2026-11-23T02:38:32Z", actor: "human:operator", revision: 3, executor: "agent:runbook", correlation: "host-operator-2026-11-23-ess-0-98-0"}
---
## Outcome

The daemon's specification validates on the next ESS release: in the command `SubmitRun`, the
outcome `no-session` is declared above `below-floor`, the order in which the precedence answers.
The daemon's handler answers in the declared order.

## Source

- Notice of the ESS session, 2026-11-22: its unreleased build (the next ordinary release, 0.98.0,
  not tagged) refuses the specification with `[conflicting_declaration]
  command.sdc.daemon.SubmitRun.outcomes.no-session: no-session answers at step 4 before
  below-floor, declared above it, wherever both hold; the declaration order and the precedence
  order disagree` (ESS-COMMAND-004, `domains/daemon.yaml:480`). ESS 0.97.4, the pinned version,
  accepts the file.
- The ESS session validated a copy with the reorder on its unreleased build. It did not run the
  reordered copy on 0.97.4.
- Found in the unit, 2026-11-23: the handler checked the floor before the session, the reverse of
  the declared order. The handler follows the declaration in this unit (a request below the floor
  that names a missing session is now refused `no-session`; no request that was refused is
  accepted).

## Acceptance

- `no-session` stands above `below-floor` in `SubmitRun` of `ess/domains/daemon.yaml`.
- The handler refuses a missing session before the floor; a test reads the order of the outcomes
  from the domain file and holds the handler and the table of outcomes in the code to it, order
  included. A request below the floor is refused whatever its session, its idempotency key or its
  other members are, and writes nothing.
- An authored scenario holds the order. On the pinned ESS: `task check:ess-generated` exits 0
  after the suites and `DIGEST` are regenerated; the conformance run prints its counts with the
  one added authored scenario (13 authored; both sets of numbers in the change's message).
- `docs/daemon-api.md` and `CHANGELOG.md` state the order and the counts the head prints.
- The sha and the counts are sent to the ESS session.

## Scope

- `ess/domains/daemon.yaml`, `ess/scenarios/`, the generated suites and `DIGEST` under `ess/`.
- `runbook/src/commands.rs`, `runbook/tests/daemon/`, `docs/daemon-api.md`, `CHANGELOG.md`.

## Not in this story

The pin of ESS 0.98.0: it follows the release, on the operator's order.
