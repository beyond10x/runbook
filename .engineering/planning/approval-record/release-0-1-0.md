---
format: aep.planning-md/3
id: approval-record:release-0-1-0
kind: approval-record
status: approved
title: Release 0.1.0
summary: 'Operator''s standing order, 2026-11-21: cut releases after every improvement. Covers the tag 0.1.0 at 4468bfd and its GitHub Release.'
revision: 3
transitions:
- {from: "draft", to: "approved", at: "2026-11-21T12:43:58Z", actor: "human:operator", revision: 3, executor: "agent:sdc-merge-queue"}
---
# Approval: release 0.1.0

## Decision

The operator's standing order in the sdc session, 2026-11-21: "Cut releases after every
improvement." Relayed to the merge queue by the coordinating sdc session as the order to cut 0.1.0
at `4468bfd60e9665c17d57d7ffbbc8e7811d2b4a84` once `main` was green there. Executed by the merge
queue agent as the bot `b10x-bot[bot]`.

## What it covers

- Release 0.1.0 of this repository from `main` at `4468bfd` (the release batch: docs/trial-3,
  fix/review-reply, feat/effort-per-class, fix/superseded-answers, the release preparation
  `8b3abd4`, and the test-only commit `4468bfd`).
- Gates on `4468bfd`: `release:check VERSION=0.1.0` exit 0, `b10x-gates check` and `verify` exit 0,
  the CI check `Shared source gates` green on `main` (run 44905171884).
- The annotated tag `0.1.0` by `b10x-bot[bot]` (tag object `cfc0bded52f4d5a7f09de488c6439363b55c8241`,
  peeled `4468bfd`), green on CI (run 44905283238).
- The GitHub Release `0.1.0` with the notes `docs/releases/0.1.0.md`
  (https://github.com/beyond10x/sdc/releases/tag/0.1.0).

## What it does not cover

- Any later release, tag or version number; each is its own order.
