---
format: aep.planning-md/3
id: approval-record:release-0-3-0
kind: approval-record
status: approved
title: Release 0.3.0
summary: 'Operator''s pick, relayed 2026-11-22 06:47 CET: "A: tag 0.3.0 now". Covers the tag 0.3.0 at ae7a697 and its GitHub Release.'
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-22T05:52:30Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
---
# Approval: release 0.3.0

## Decision

On 2026-11-22 at 06:47 CET (05:47Z) the host's operator session relayed the operator's pick "A: tag
0.3.0 now". It answered a question that put three options (A: tag now on the gate as composed; B:
one single full cycle run first; C: no tag) and said that the gate's count is made of three runs.
The bot `b10x-bot[bot]` tagged `main` at the release commit and published the GitHub Release on
that word.

## What it covers

- Release 0.3.0 of this repository from `main` at `ae7a697616857a2bf2744c85a33e58b0b5927dc8`, the
  merge of pull request 2 (`release/0.3.0`, head `59f6841`).
- Gates:
  - The checks that `docs/releases/0.3.0.md` § Checks lists: the local checks on the release
    preparation, the finding cycles on the frozen tree of `5d9e679`, and CI on `705171f`. This
    record states none of their numbers beyond that section.
  - `b10x-gates check --receipt` exit 0 on `59f6841`.
  - `task release:check VERSION=0.3.0` exit 0 on `main` before the tag.
  - On `main` at `ae7a697`: `Shared source gates` (run 44986946006) and `Documentation validation`
    (run 44986945866) green; `Documentation site` (run 44987075826) green.
- The annotated tag `0.3.0` by `b10x-bot[bot]` (tag object
  `a979b6dfc56ff97c274ca3f66d0fe56e3182a95c`, peeled
  `ae7a697616857a2bf2744c85a33e58b0b5927dc8`, message "Runbook 0.3.0"). `Shared source gates` on
  the tag (run 44991003645) read `completed`, conclusion `success`, at the time of this record.
- The GitHub Release `0.3.0` with the notes `docs/releases/0.3.0.md`, published
  2026-11-22T05:48:51Z (https://github.com/beyond10x/runbook/releases/tag/0.3.0).

## What the operator knew

The release notes (`docs/releases/0.3.0.md` § Checks) state the cycle count of the gate: 124
expected cycles, 124 as expected, and 2 pending cycles that still fail as recorded. The count is made
of three runs on one tree (`5d9e679`) and one set of binaries: a full run stopped after 109 cycles
when the shared build machine was overloaded, then 10 and 5 cycles by name. No single full run ended
with all 124. The operator picked A with that stated, and not B, the one single full run.

## What it does not cover

- Any later release, tag or version number; each is its own order.
- A claim about clean passes, cost, wall time or quality of the 0.3.0 tree: no run has used it
  (`docs/releases/0.3.0.md` § Runs and Limits), and the four switches are not measured.
