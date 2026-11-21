---
format: aep.planning-md/3
id: approval-record:release-0-2-0
kind: approval-record
status: approved
title: Release 0.2.0
summary: 'Operator''s order, 2026-11-21: one release 0.2.0 with the rename to beyond10x/runbook, then CUT RELEASE ASAP. Covers the tag 0.2.0 at 6cde564 and its GitHub Release.'
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-21T20:36:46Z", actor: "human:operator", revision: 2, executor: "agent:sdc-merge-queue"}
---
# Approval: release 0.2.0

## Decision

The operator ordered on 2026-11-21 that the open work ships as one release, 0.2.0, in place of the
prepared 0.1.5, together with the rename of the repository `beyond10x/sdc` to `beyond10x/runbook`
("merge everything and rename it"). After the rename the public site was broken, and at 20:24Z the
operator ordered "CUT RELEASE ASAP" in the coordinating session. The coordinating session passed
both orders to the merge queue, with the steps of the rename and of the release. Executed by the
merge queue agent as the bot `b10x-bot[bot]`.

## What it covers

- Release 0.2.0 of this repository from `main` at `6cde564f7d0cfbef221e372a6c7bfef79d512c26`:
  - batch 13 (the spec gate fix, the release-0.1.2 results);
  - batch 14: the Runbook rename stages 1 and 3, the landing pitch, the evaluation rework, the
    Context page, the four-entry navbar, the consistency pass `b78d137`, the preparation
    `55b531d`, and the README logo merged as `6cde564`.
- Gates:
  - Batch 13's full round on `08df7a5` exited 0 at every step: check-protocols, driver build,
    clippy and tests, check-bindings, experiment check, aep validate, docs:build, docs:test,
    evaluate-cases, the finding cycles (100 expected, 100 as expected, 0 failed; 2 pending) and
    the autopilot stub trial.
  - On `55b531d` before the push, all exit 0: driver build, the `sdc-protocol-site` tests (10)
    and check, docs:build, and `b10x-gates check` and `verify` under `beyond10x/runbook`.
  - On `6cde564`, all exit 0: `release:check VERSION=0.2.0`, `b10x-gates check` and `verify`.
  - Every commit since 0.1.4 is authored and committed by `b10x-bot[bot]`, and the scan of the
    range found nothing.
- The annotated tag `0.2.0` by `b10x-bot[bot]` (tag object
  `dc2b6cf670556ac8cd373e363e7d958a47dab4ad`, peeled `6cde564`). `Shared source gates` on the tag
  is green (run 44946205720).
- The GitHub Release `0.2.0` with the notes `docs/releases/0.2.0.md`
  (https://github.com/beyond10x/runbook/releases/tag/0.2.0).

## Known defects of the tagged tree

- `Documentation validation` failed on `6cde564` (run 44946173410). `website/tests/smoke.spec.ts`
  expected the list of releases without runs to be exactly 0.1.1, 0.1.3 and 0.1.4, and the 0.2.0
  entry extends it. Separately, `website/data/protocols/index.json` was stale after the README
  merge.
- Both are fixed on `main` in `e46a237`, where `Documentation validation` is green (run
  44946642815). They ship as release 0.2.1. The tag 0.2.0 is not moved.

## What it does not cover

- Release 0.2.1 and any later release, tag or version number; each is its own order.
- Removing `beyond10x/sdc` from the Gates policy, which waits for the orchestrator's word.
