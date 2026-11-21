---
format: aep.planning-md/3
id: approval-record:release-0-1-1
kind: approval-record
status: approved
title: Release 0.1.1
summary: 'Operator''s standing order, 2026-11-21: cut releases after every improvement. Covers the tag 0.1.1 at d79975d and its GitHub Release.'
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-21T14:41:51Z", actor: "human:operator", revision: 2, executor: "agent:sdc-merge-queue"}
---
# Approval: release 0.1.1

## Decision

The operator's standing order in the sdc session, 2026-11-21: "Cut releases after every
improvement." Under `docs/releasing.md` § Standing rule the coordinating sdc session (the
orchestrating session) ordered the merge queue to cut 0.1.1 at the commit after the data-only
baseline merge, once `main` was green there. Executed by the merge queue agent as the bot
`b10x-bot[bot]`.

## What it covers

- Release 0.1.1 of this repository from `main` at `d79975dd70dcbdca1b8820b2bdb2774b28e76028`
  (batch 9: audit, fix/attempt5-close, fix/review-reply, impl/bugfix-cycle, feat/simulate-gate,
  feat/run-metrics, feat/progress-ledger, ui/run-picker, fix/decider-cost, the unstick effort
  `0f7eb8f`, the preparation `50cb76f`, the revert of feat/recorded-replay `7c5dd5e`, the baseline
  of 0.1.0 `c798d1a` and the notes `d79975d`).
- Gates: the cycles and checks on `7c5dd5e` (89 expected, 89 as expected), check-protocols on
  `50cb76f`; on `d79975d` driver test, check-bindings, `sdc-experiment check --all`, aep validate,
  `release:check VERSION=0.1.1`, `b10x-gates check` and `verify`, all exit 0; the CI check
  `Shared source gates` green on `main` (run 44915100444).
- The annotated tag `0.1.1` by `b10x-bot[bot]` (tag object `b015cfe0f29e2b8047fcfdbb4735fbdd90527faa`,
  peeled `d79975d`), green on CI (run 44915180149).
- The GitHub Release `0.1.1` with the notes `docs/releases/0.1.1.md`
  (https://github.com/beyond10x/sdc/releases/tag/0.1.1).

## What it does not cover

- Any later release, tag or version number; each is its own order.
