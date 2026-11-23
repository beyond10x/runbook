---
format: aep.planning-md/3
id: approval-record:release-0-6-0
kind: approval-record
status: approved
title: Release 0.6.0
summary: 'Operator''s pick of 2026-11-22 23:57:02 CET, relayed by the host''s operator session: release 0.6.0 now; a minor. Covers the tag 0.6.0 at 3c40ef2, its GitHub Release and its checks.'
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-23T09:14:45Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
---
# Approval: release 0.6.0

## Decision

On 2026-11-22 the coordinator put the question "release 0.6.0 from `main` now?" to the host's
operator session as a quick selection for the operator, with no default, because a tag is his. At
23:57:02 CET (22:57:02Z) that session relayed his pick: "A: release 0.6.0 now (Recommended)". Option
A read: "version commit as a bot pull request, tag after green CI on `main`, release notes". The
rules of the earlier order still held: "Minor if a user sees a change, patch for fixes only; no
major version." and "Release from a green default branch only. Work on a branch or an open PR does
not go in; do not merge a branch to make a release." Runbook 0.6.0 is a minor release: a user sees
the change (`runbook canon`, the read-only inspector, routing by autonomy level,
`runbook run --recorded`). The bot `b10x-bot[bot]` tagged `main` at the merge commit and published
the GitHub Release on that pick.

## What it covers

- Release 0.6.0 of this repository from `main` at `3c40ef21e1bb7ba8e77e3b89e889543a5d149875`, the
  merge of pull request 34 (`release/0.6.0`, head `700a45c065ecdd24da4f708c01fa5bb1490838bc`), merged
  2026-11-23T00:17:11Z. Pull request 33 had merged the release preparation at 2026-11-22T23:38:22Z;
  pull request 34 set the date of the release to the day of its tag.
- What is in it: `docs/releases/0.6.0.md` and the `[0.6.0]` section of `CHANGELOG.md`. What is not
  in it, because it was on a branch: the deletion of the old driver and the fix for the stopped runs.
- Checks before the tag:
  - On `main` at `3c40ef21e1`: the check runs of that commit were all completed and success, read at
    2026-11-23T00:22:20Z (`common / Security and privacy`, `Build documentation`,
    `Find the built site`, `publish / publish`).
  - `task release:check VERSION=0.6.0` exit 0 on the merge commit.
- The annotated tag `0.6.0` by `b10x-bot[bot]` (tag object
  `db014169f29e51fdac9ce6e10b0720acec77c334`, peeled
  `3c40ef21e1bb7ba8e77e3b89e889543a5d149875`, message "Runbook 0.6.0"), made 2026-11-23T00:22:34Z.
- The GitHub Release `0.6.0` with the notes `docs/releases/0.6.0.md`, published
  2026-11-23T00:23:22Z (https://github.com/beyond10x/runbook/releases/tag/0.6.0). It has no binary
  asset.

## What was not run

A full local `task check` was not run on the release commit; the release rests on the forge's runs
above. `docs/releases/0.6.0.md` § Limits states the rest: the three runs of
`experiments/parity-loom-strong-4.yaml` delivered nothing, the old driver is still in the tree, and
of the 130 cycle entries 37 are converted to the tree suite.

## Standing rule

This release is not one under the standing rule "a release after every improvement" of
`docs/releasing.md`: it claims no measured change. It rests on the operator's pick above. Its ledger
entry is in `experiments/releases.yaml`.

## What it does not cover

- Any later release, tag or version number.
- A claim about clean passes, cost, wall time or quality of the 0.6.0 tree beyond what
  `experiments/` records.
- Every branch or open pull request at the time of the tag.
