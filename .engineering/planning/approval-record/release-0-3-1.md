---
format: aep.planning-md/3
id: approval-record:release-0-3-1
kind: approval-record
status: approved
title: Release 0.3.1
summary: 'Operator''s pick, relayed 2026-11-22 10:15 CET: "A: tag 0.3.1 after PR 5". Covers the tag 0.3.1 at ebea58d, its GitHub Release and the nine registration runs under a cap of 30 USD.'
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-22T11:35:01Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
---
# Approval: release 0.3.1

## Decision

On 2026-11-22 at 10:15 CET (09:15Z) the host's operator session relayed the operator's pick "A: tag
0.3.1 after PR 5". The decision reads: "Tag and release 0.3.1 on main after PR 5 merges (bot route,
receipt check), notes list what it holds; then the 9 runs on 0.3.1 under a new record, cap 30 USD,
when the queue is short." The bot `b10x-bot[bot]` tagged `main` at the merge commit and published the
GitHub Release on that word.

## What it covers

- Release 0.3.1 of this repository from `main` at `ebea58dee2d1b4c9326a07d4e70128fa922bd01b`, the
  merge of pull request 5 (`wave/rb2`, head `42412cd93c0f885cdeb83c962ea569cc094b2ef0`), merged
  2026-11-22T11:30:35Z.
- Gates:
  - The checks that `docs/releases/0.3.1.md` § Checks lists. This record states none of their
    numbers beyond that section.
  - `b10x-gates check --receipt` valid on `42412cd`.
  - On `main` at `ebea58dee`: `Documentation validation` (run 45017666579), `Protocol checks`
    (run 45017666593), `Shared source gates` (run 45017667072) and `Documentation site`
    (run 45017822210), all success.
- The annotated tag `0.3.1` by `b10x-bot[bot]` (tag object
  `55c1c898046a4ec2caa068b35d5a04c7213402f0`, peeled
  `ebea58dee2d1b4c9326a07d4e70128fa922bd01b`, message "Runbook 0.3.1").
- The GitHub Release `0.3.1` with the notes `docs/releases/0.3.1.md`, published
  2026-11-22T11:33:29Z (https://github.com/beyond10x/runbook/releases/tag/0.3.1).
- The nine registration runs on this tree, under a new experiment record `release-0.3.1` with a cap of
  30 USD; the record is its own change and the runs start only when the queue is short.

## What the operator knew

The release notes (`docs/releases/0.3.1.md` § Checks) state that the finding cycles ran twice in
full with 126 of 127 as expected. The one failure, `pool-wait`, has its cause fixed in `4133330ac`,
the three pool cycles passed alone after the fix, and the full set was not run again. The operator
picked A with that stated, and not a further full cycle run first.

## What it does not cover

- Any later release, tag or version number; each is its own order.
- A claim about clean passes, cost, wall time or quality of the 0.3.1 tree: no run has used it when
  this record is written.
- A spend above 30 USD for the nine runs.
