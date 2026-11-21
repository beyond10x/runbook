---
format: aep.planning-md/3
id: approval-record:release-0-1-2
kind: approval-record
status: approved
title: Release 0.1.2
summary: 'Operator''s standing order, 2026-11-21: cut releases after every improvement. Covers the tag 0.1.2 at 72a3eef and its GitHub Release.'
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-21T15:56:39Z", actor: "human:operator", revision: 2, executor: "agent:sdc-merge-queue"}
---
# Approval: release 0.1.2

## Decision

The operator's standing order in the sdc session, 2026-11-21: "Cut releases after every
improvement." Under `docs/releasing.md` § Standing rule the coordinating sdc session (the
orchestrating session) closed batch 10 and ordered the merge queue to cut 0.1.2 at the release
commit once CI on `main` was green there, without asking again. Executed by the merge queue agent
as the bot `b10x-bot[bot]`.

## What it covers

- Release 0.1.2 of this repository from `main` at `72a3eefb20e0edff50dc8ad12fcf561f8e0790e3`
  (batch 10: docs/trial-3-costs `66e5935`, feat/recorded-replay merged again `8a96b50` with the
  unstick fix `f717443`, feat/run-timeline Part A `9e0492f`, the root protocol's description
  `1109656`, fix/decider-principal `20fa990`, the preparation `72a3eef`).
- Gates on `72a3eef`: check-protocols, driver clippy and test, check-bindings (15 route findings,
  experiments ok), aep validate, evaluate-cases, the finding cycles (98 expected, 98 as expected,
  0 failed; 2 pending), the autopilot stub trial, `release:check VERSION=0.1.2`, `b10x-gates check`
  and `verify`, all exit 0; every commit since 0.1.1 authored and committed by `b10x-bot[bot]`; the
  CI check `Shared source gates` green on `main` (run 44921731118).
- The annotated tag `0.1.2` by `b10x-bot[bot]` (tag object `7a6428461cdc68108017778ed1b09a871e68656f`,
  peeled `72a3eef`), green on CI (run 44921788276).
- The GitHub Release `0.1.2` with the notes `docs/releases/0.1.2.md`
  (https://github.com/beyond10x/sdc/releases/tag/0.1.2).

## What it does not cover

- Any later release, tag or version number; each is its own order.
