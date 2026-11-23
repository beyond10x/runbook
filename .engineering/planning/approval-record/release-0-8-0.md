---
format: aep.planning-md/3
id: approval-record:release-0-8-0
kind: approval-record
status: approved
title: Release 0.8.0
summary: 'Operator''s order of 2026-11-23 03:59 CET, relayed by the host''s operator session: release when a user sees something; a minor. Covers the tag 0.8.0 at 91dafa9, its GitHub Release and its checks.'
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-23T09:14:47Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
---
# Approval: release 0.8.0

## Decision

On 2026-11-23 at 03:57 CET (02:57Z) the operator gave the host's operator session the decisions of
the night: "It will be your responsibility to take decisions for me now, unless they are about
secrets or money or architectural changes." At 03:59 CET (02:59Z) that session relayed his order for
this repository with the line: "Release when you have something a user sees and send me the facts."
The rules of the earlier order still held: "Minor if a user sees a change, patch for fixes only; no
major version." and "Release from a green default branch only. Work on a branch or an open PR does
not go in; do not merge a branch to make a release." Runbook 0.8.0 is a minor release: a user sees
the change (the Products tab of the inspector, the exits of the bindings, `runbook test
--cycle-timeout` and `--list`, and breaking changes are listed in the notes). The bot
`b10x-bot[bot]` tagged `main` at the merge commit and published the GitHub Release on that order;
the facts went to the host's operator session after the publication.

## What it covers

- Release 0.8.0 of this repository from `main` at `91dafa9da447bf30264e26d689ef72382eb18b41`, the
  merge of pull request 42 (`release/0.8.0`, head `fcee453db72b1323569b426951b14b043d35b909`), merged
  2026-11-23T08:21:26Z.
- What is in it: `docs/releases/0.8.0.md` and the `[0.8.0]` section of `CHANGELOG.md`.
- Checks before the tag:
  - On `main` at `91dafa9da4`: the check runs of that commit were all completed and success, read at
    2026-11-23T08:33:17Z (`common / Security and privacy`, `Build documentation`,
    `Find the built site`, `publish / publish`, `Protocol checks, slow lane`).
  - `task release:check VERSION=0.8.0` exit 0 on the merge commit.
- The annotated tag `0.8.0` by `b10x-bot[bot]` (tag object
  `9c498482cd141b94702d87ecec8a233fc2fd8cd3`, peeled
  `91dafa9da447bf30264e26d689ef72382eb18b41`, message "Runbook 0.8.0"), made 2026-11-23T08:33:30Z.
- The GitHub Release `0.8.0` with the notes `docs/releases/0.8.0.md`, published
  2026-11-23T08:34:24Z (https://github.com/beyond10x/runbook/releases/tag/0.8.0). It has no binary
  asset.

## What was not run

A full local `task check` was not run on the release commit; the release rests on the forge's runs
above. No evaluation run was made on this code. The notes claim no count and no time of a test step:
the machine that prepared them did not run the whole test steps within their budget.
`docs/releases/0.8.0.md` § Limits states the rest.

## Standing rule

This release is not one under the standing rule "a release after every improvement" of
`docs/releasing.md`: it has no measured run of its own and claims no measured change. It rests on
the order above.

## What it does not cover

- Any later release, tag or version number.
- A claim about clean passes, cost, wall time or quality of the 0.8.0 tree: no run has used it.
- Every branch or open pull request at the time of the tag.
