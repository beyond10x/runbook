---
format: aep.planning-md/3
id: approval-record:release-0-2-1
kind: approval-record
status: approved
title: Release 0.2.1
summary: 'Operator, 2026-11-21 about 20:29Z: "So I really want to see a new cut version". Covers the tag 0.2.1 at 99f5617 and its GitHub Release.'
revision: 3
transitions:
- {from: "draft", to: "approved", at: "2026-11-21T21:41:25Z", actor: "human:operator", revision: 3, executor: "agent:sdc-merge-queue"}
---
# Approval: release 0.2.1

## Decision

On 2026-11-21 at about 20:29Z the operator asked in the coordinating session for a release that
can be cut with confidence: "So I really want to see a new cut version" and "When do we have a
stable version we can confidently cut?". Tag 0.2.0 (`6cde564`) carried two defects that the
documentation checks see; 0.2.1 is 0.2.0 with their fix. The coordinating session took the
operator's words as the order to cut 0.2.1, prepared on `main` as `99f5617`, and tagged and
published it as the bot `b10x-bot[bot]` at 20:51Z, after CI was green on `99f5617`.

## What it covers

- Release 0.2.1 of this repository from `main` at `99f56179428ee272acb894535f852bca93275d56`:
  - the fix `e46a237`: the evaluation smoke test admits releases without runs after 0.1.4, and
    `website/data/protocols/index.json` is current;
  - the record of 0.2.0 (`60b8d3e`);
  - the preparation `99f5617`: version 0.2.1 in the crates, lock files and `ui/package.json`,
    `CHANGELOG.md` `[0.2.1]`, `docs/releases/0.2.1.md`, the ledger entry and the regenerated
    progress and site data.
- Gates:
  - The checks on `e46a237` and on `99f5617` that `docs/releases/0.2.1.md` § Checks lists.
  - On `main` at `99f5617`: `Shared source gates` (run 44947963603) and `Documentation validation`
    (run 44947963427) green; `Documentation site` published (run 44948308821).
- The annotated tag `0.2.1` by `b10x-bot[bot]` (tag object
  `cdf2c1076b21833fc45ab64357e55a81f64e8ebf`, peeled `99f5617`, tagged 20:51:14Z). `Shared source
  gates` on the tag is green (run 44948472571).
- The GitHub Release `Runbook 0.2.1` with the notes `docs/releases/0.2.1.md`, published 20:51:25Z
  (https://github.com/beyond10x/runbook/releases/tag/0.2.1).

## What it does not cover

- Any later release, tag or version number; each is its own order.
- The four cost changes behind run switches and the work merged after `99f5617`.
