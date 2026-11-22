---
format: aep.planning-md/3
id: story:submitrun-outcome-order
kind: story
status: draft
title: SubmitRun declares no-session above below-floor
summary: 'Notice of the ESS session, 2026-11-22: the next ESS release refuses the declaration order of two outcomes of SubmitRun; the pinned 0.97.4 accepts it.'
tags:
- backlog-2026-11-22-w1
revision: 1
---
## Outcome

The daemon's specification validates on the next ESS release: in the command `SubmitRun`, the
outcome `no-session` is declared above `below-floor`, the order in which the precedence answers.

## Source

- Notice of the ESS session, 2026-11-22: its unreleased build (the next ordinary release, 0.98.0,
  not tagged) refuses the specification with `[conflicting_declaration]
  command.sdc.daemon.SubmitRun.outcomes.no-session: no-session answers at step 4 before
  below-floor, declared above it, wherever both hold; the declaration order and the precedence
  order disagree` (ESS-COMMAND-004, `domains/daemon.yaml:480`). ESS 0.97.4, the pinned version,
  accepts the file.
- The ESS session validated a copy with the reorder on its unreleased build. It did not run the
  reordered copy on 0.97.4.

## Acceptance

- `no-session` stands above `below-floor` in `SubmitRun` of `ess/domains/daemon.yaml`.
- On the pinned ESS: `task check:ess-generated` exits 0 after the suites and `DIGEST` are
  regenerated; the conformance run prints the same scenario counts as before the reorder (both
  numbers in the change's message).
- The sha and the two counts are sent to the ESS session.

## Scope

- `ess/domains/daemon.yaml`, the generated suites and `DIGEST` under `ess/`.

## Not in this story

The pin of ESS 0.98.0: it follows the release, on the operator's order.
