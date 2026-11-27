---
format: aep.planning-md/3
id: approval-record:release-0-14-0
kind: approval-record
status: draft
title: Release 0.14.0
summary: 'Operator''s word of 2026-11-27 about 15:10 CET through the host''s operator session: "A: 0.14.0 now", with the go for the preparation pull request 130 after 129. Covers the tag 0.14.0 at 2fb0e19a, its GitHub Release with four archive assets, and its checks.'
revision: 1
---
# Approval: release 0.14.0

## Decision

On 2026-11-27 at about 15:10 CET (14:10Z) the operator gave his word for this release through the host's operator
session: "A: 0.14.0 now", a release from `main` at `73948079526ee954b1bc2715d80bf5dda3151fdd`. With it the operator
gave, through the same session, the go for the release preparation pull request (pull request 130,
`release/0.14.0`, head `f965e62b`) "after #129; if #129 moves main under it, update and merge on the new green head
without asking again". Pull request 129 moved `main` to `831740662cc4d060a73a2b4faf235b0e0d234f3b` before the
preparation was merged, so `release/0.14.0` merged that `main` (head `5049e505ee83b2296a9587b42b3ee88e51fe00bb`) and
was merged on that head without a second word; the check runs of that head ended success by 2026-11-27T15:28:07Z
(`Upload the archives to the release` skipped, as on every pull request). Runbook 0.14.0 is a minor release of 0.13.0 with one change marked
breaking (`docs/releases/0.14.0.md`). The bot `b10x-bot[bot]` merged the preparation, tagged `main` at the merge
commit and published the GitHub Release on that word.

## What it covers

- Release 0.14.0 of this repository from `main` at `2fb0e19ace21d74b9904920ad1c42d90f3119147`, the bot's merge of
  pull request 130 (`release/0.14.0`, head `5049e505ee83b2296a9587b42b3ee88e51fe00bb`) onto `main` at
  `831740662cc4d060a73a2b4faf235b0e0d234f3b` (pull request 129). The merge commit was made 2026-11-27T15:31:39Z and
  pushed to `main` at 2026-11-27T15:32:48Z (the merge time GitHub gives pull request 130).
- What is in it: `docs/releases/0.14.0.md` and the `[0.14.0]` section of `CHANGELOG.md`.
- The check runs of the merge commit, read at 2026-11-27T16:08:45Z, all completed: eight success (`common / Security
  and privacy` twice, `Protocol checks, slow lane`, `Build documentation`, `Archive x86_64-unknown-linux-gnu`,
  `Archive x86_64-unknown-linux-musl`, `Upload the archives to the release`, `Find the built site`) and one failure,
  `publish / publish` of the workflow `Documentation site` (run 46028219939), which deploys the site and checks no
  code. It waited queued from 15:48:55Z and failed at 16:08:18Z in its step `Verify caller authority`, which requires
  `main` to be the commit it publishes; `main` had moved to `2b1940d4`, pushed at about 15:57Z. That this check is the one that
  failed is inferred: the step prints no reason.
- The annotated tag `0.14.0` by `b10x-bot[bot]` (tag object `ae57621d8bcd44358226994f4dd7bbfa51061d6d`, peeled
  `2fb0e19ace21d74b9904920ad1c42d90f3119147`, message "SDC 0.14.0"), made 2026-11-27T15:33:02Z.
- The GitHub Release `0.14.0` with the notes `docs/releases/0.14.0.md`, published 2026-11-27T15:33:59Z
  (https://github.com/beyond10x/runbook/releases/tag/0.14.0), and its four assets, uploaded by the release workflow:
  `runbook-0.14.0-x86_64-unknown-linux-gnu.tar.gz` and `runbook-0.14.0-x86_64-unknown-linux-musl.tar.gz`, each with
  its `.sha256` file. `sha256sum -c` of both `.sha256` files said OK, and the musl archive's `bin/runbook --version`
  printed "runbook 0.14.0 (2fb0e19ace21)".

## How it was recorded

This record was written after the release, as step 5 of `docs/releasing.md`. The instant and words of the operator's
word are those the host's operator session gave with the order to record it; the commit, pull request, tag, release,
asset and check facts above were read from the repository and from GitHub when the record was written.

## Standing rule

This release is not one under the standing rule "a release after every improvement" of `docs/releasing.md`: it rests
on the operator's word above.

## What it does not cover

- Any later release, tag or version number.
- A claim about clean passes, cost, wall time or quality of the 0.14.0 tree beyond what `docs/releases/0.14.0.md`
  states in its Limits.
- Every branch or open pull request at the time of the tag.
