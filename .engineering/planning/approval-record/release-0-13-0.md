---
format: aep.planning-md/3
id: approval-record:release-0-13-0
kind: approval-record
status: draft
title: Release 0.13.0
summary: 'Operator''s word of 2026-11-26 23:00 CET through the host''s operator session: 0.13.0 is a go, and the go for the preparation pull request at 23:25 CET. Covers the tag 0.13.0 at 75ea0758, its GitHub Release and its checks.'
revision: 1
---
# Approval: release 0.13.0

## Decision

On 2026-11-26 at 23:00 CET (22:00Z) the operator gave his word for this release through the host's operator session:
"0.13.0 is a go". At 23:25 CET (22:25Z) the operator gave, through the same session, the go for the release
preparation pull request (pull request 119, `release/0.13.0`). Runbook 0.13.0 is a minor release of 0.12.0 with three
changes marked breaking (`docs/releases/0.13.0.md`, "Breaking changes"). The bot `b10x-bot[bot]` merged the
preparation, tagged `main` at the merge commit and published the GitHub Release on that word.

## What it covers

- Release 0.13.0 of this repository from `main` at `75ea07583e76664b532a2627535a44b2676a0629`, the bot's merge of
  pull request 119 (`release/0.13.0`, head `8d542d6a777a8670af42b73e777afe7bec361d1d`), made
  2026-11-26T22:27:20Z.
- What is in it: `docs/releases/0.13.0.md` and the `[0.13.0]` section of `CHANGELOG.md`.
- The check runs of the merge commit, all completed and success, read at 2026-11-27 (`common / Security and
  privacy`, `Build documentation`, `Find the built site`, `publish / publish`, `Protocol checks, slow lane`).
- The annotated tag `0.13.0` by `b10x-bot[bot]` (tag object `ed66897c5ee704c684aaf85f606b311836b9f681`, peeled
  `75ea07583e76664b532a2627535a44b2676a0629`, message "Runbook 0.13.0"), made 2026-11-26T22:48:44Z.
- The GitHub Release `0.13.0` with the notes `docs/releases/0.13.0.md`, published 2026-11-26T22:49:33Z
  (https://github.com/beyond10x/runbook/releases/tag/0.13.0). It has no binary asset.

## How it was recorded

This record was written after the release, as step 5 of `docs/releasing.md`. The two instants of the operator's word
are those the host's operator session gave with the order to record it; the commit, tag, release and check facts
above were read from the repository and from GitHub when the record was written.

## Standing rule

This release is not one under the standing rule "a release after every improvement" of `docs/releasing.md`: it rests
on the operator's word above.

## What it does not cover

- Any later release, tag or version number.
- A claim about clean passes, cost, wall time or quality of the 0.13.0 tree beyond what `docs/releases/0.13.0.md`
  states in its Limits.
- Every branch or open pull request at the time of the tag.
