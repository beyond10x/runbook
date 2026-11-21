---
format: aep.planning-md/3
id: approval-record:release-0-1-4
kind: approval-record
status: approved
title: Release 0.1.4
summary: 'Operator''s standing order, 2026-11-21: cut releases after every improvement. Covers the tag 0.1.4 at 7bbdee6, its GitHub Release and the site''s publication.'
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-21T19:21:33Z", actor: "human:operator", revision: 2, executor: "agent:sdc-merge-queue"}
---
# Approval: release 0.1.4

## Decision

The operator's standing order in the sdc session, 2026-11-21: "Cut releases after every
improvement." Under `docs/releasing.md` § Standing rule the coordinating sdc session (the
orchestrating session) closed batch 12, added docs/quality-0.1.0 to it so that the cost comparison
and the quality evaluation are published in the same release, ordered the conformance observation
off by default in this release, and ordered the merge queue to push `main` and cut 0.1.4 at that
commit without asking again. Executed by the merge queue agent as the bot `b10x-bot[bot]`.

## What it covers

- Release 0.1.4 of this repository from `main` at `7bbdee60c4595bf2beee8406a3f45b984def5dba`
  (batch 12: exp/improve-1 at `110ab2e`, merged as `7c9e34b`; docs/conformance-chart at `38859a8`,
  merged as `9195563`, with feat/conformance-observe `e8d1df7` and feat/run-timeline `0b65626`;
  the conformance default off `e1dd70a`; the preparation `d1b9b0e`; docs/quality-0.1.0 at
  `6c67a55`, merged as `eb4c6e1`; the notes `7bbdee6`).
- Gates on `d1b9b0e`: driver build, clippy and test (177, the conformance tests with the pinned
  ess among them), check-bindings, `sdc-experiment check --all`, aep validate, `task docs:build`
  and `task docs:test`, evaluate-cases, the finding cycles (99 expected, 99 as expected, 0 failed;
  2 pending), the autopilot stub trial, all exit 0. From `d1b9b0e` to `7bbdee6` nothing under
  driver/, scripts/, protocol/, cases/ or Taskfile.yml changed; on `7bbdee6` check-bindings,
  `sdc-experiment check --all`, aep validate, docs:build and docs:test again, the scan of the added
  data files and screenshots, `release:check VERSION=0.1.4`, `b10x-gates check` and `verify`, all
  exit 0. Every commit since 0.1.3 authored and committed by `b10x-bot[bot]`. On `main` the CI
  checks `Shared source gates` (run 44938542019) and `Documentation validation` (run 44938541851)
  green.
- The publication of https://beyond10x.github.io/sdc/ by `Documentation site` (run 44940310778).
- The annotated tag `0.1.4` by `b10x-bot[bot]` (tag object `67e7e38fdbe0b0fe0d6a7c4a529aae4487abeec1`,
  peeled `7bbdee6`), green on CI (run 44940450316).
- The GitHub Release `0.1.4` with the notes `docs/releases/0.1.4.md`
  (https://github.com/beyond10x/sdc/releases/tag/0.1.4).

## What it does not cover

- Any later release, tag or version number; each is its own order.
