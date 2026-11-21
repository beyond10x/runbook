---
format: aep.planning-md/3
id: approval-record:release-0-1-3
kind: approval-record
status: approved
title: Release 0.1.3
summary: 'The operator''s "publish it" and standing order, 2026-11-21: cut releases after every improvement. Covers the tag 0.1.3 at 99390b9, its GitHub Release and the site''s publication.'
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-21T17:01:33Z", actor: "human:operator", revision: 2, executor: "agent:sdc-merge-queue"}
---
# Approval: release 0.1.3

## Decision

The operator approved the documentation site, "publish it", and the standing order in the sdc
session, 2026-11-21: "Cut releases after every improvement." Under `docs/releasing.md` § Standing
rule the coordinating sdc session (the orchestrating session) closed batch 11 and ordered the merge
queue to push `main`, which publishes the site, and to cut 0.1.3 at that commit without asking
again. Executed by the merge queue agent as the bot `b10x-bot[bot]`.

## What it covers

- Release 0.1.3 of this repository from `main` at `99390b9c61929a15a0c5206cba1e43891c498c23`
  (batch 11: exp/improve-1 at `f619fd4`, merged as `0dd42eb`; docs/site at `dd02c33`, merged as
  `925addd`; the preparation `99390b9`).
- Gates on `99390b9`: driver build, clippy and test, check-bindings (15 route findings, experiments
  ok, site data current), `sdc-experiment check --all`, aep validate, `task docs:build` and
  `task docs:test`, evaluate-cases, the finding cycles (98 expected, 98 as expected, 0 failed;
  2 pending), the autopilot stub trial, `release:check VERSION=0.1.3`, `b10x-gates check` and
  `verify`, all exit 0; every commit since 0.1.2 authored and committed by `b10x-bot[bot]`; on
  `main` the CI checks `Shared source gates` (run 44924859148) and `Documentation validation`
  (run 44924858945) green.
- The publication of https://beyond10x.github.io/sdc/ by `Documentation site` (run 44926528232).
- The annotated tag `0.1.3` by `b10x-bot[bot]` (tag object `3e32ff085c9563be5ff3902f65716d406c07d94e`,
  peeled `99390b9`), green on CI (run 44927503604).
- The GitHub Release `0.1.3` with the notes `docs/releases/0.1.3.md`
  (https://github.com/beyond10x/sdc/releases/tag/0.1.3).

## What it does not cover

- Any later release, tag or version number; each is its own order.
