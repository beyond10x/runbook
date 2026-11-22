---
format: aep.planning-md/3
id: approval-record:release-0-4-1
kind: approval-record
status: approved
title: Release 0.4.1
summary: 'Operator''s order, relayed 2026-11-22 about 17:02 CET: cut versions, tag, cut releases; a patch. Covers the tag 0.4.1 at da86a8f, its GitHub Release, its checks and what was not run.'
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-22T16:39:13Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
---
# Approval: release 0.4.1

## Decision

On 2026-11-22 at about 17:02 CET (16:02Z) the host's operator session relayed the operator's order:
"cut versions in the repos we delivered work to default branch and can push, tag those, cut
releases". With it: "Minor if a user sees a change, patch for fixes only; no major version." and
"Release from a green default branch only. Work on a branch or an open PR does not go in; do not
merge a branch to make a release." Runbook 0.4.1 is a patch: one fix, checks and tools, and two
records, with no protocol or behaviour change of the product. The bot `b10x-bot[bot]` tagged `main`
at the merge commit and published the GitHub Release on that order.

## What it covers

- Release 0.4.1 of this repository from `main` at `da86a8f7b81fb5be09c06d19b062e790a6950ada`, the
  merge of pull request 22 (`release/0.4.1`, head `ac4e8c05282bd3114211fe992196bdd59ba4697f`), merged
  2026-11-22T16:32:55Z.
- What is in it: `docs/releases/0.4.1.md` and the `[0.4.1]` section of `CHANGELOG.md`. What is not
  in it: the runner on the Loom engine inside the daemon, which is on a branch.
- Checks before the tag:
  - On the pull request head `ac4e8c052`: `Shared source gates` (run 45043228476) and
    `Documentation validation` (run 45043227775), both success.
  - `task release:check VERSION=0.4.1` exit 0 on the merge commit.
- The annotated tag `0.4.1` by `b10x-bot[bot]` (tag object
  `10fd16048dee105c188186ef097b639e103e7e8f`, peeled
  `da86a8f7b81fb5be09c06d19b062e790a6950ada`, message "Runbook 0.4.1"), pushed 2026-11-22T16:33:33Z.
- Checks after the tag: `Shared source gates` on the tag and on `main` at `da86a8f7b`, both success.
- The GitHub Release `0.4.1` with the notes `docs/releases/0.4.1.md`, published
  2026-11-22T16:33:37Z (https://github.com/beyond10x/runbook/releases/tag/0.4.1).

## What was not run

A full local `task check` was not run on the release commit. The shared host was loaded and the test
steps ran over their 120 s budget there. The named checks of the preparation and the CI runs above
are what the release rests on. `docs/releases/0.4.1.md` § Limits says the same of the test steps and
adds that the full set of finding cycles was not run on this tree and that `task
check:ess-suite:slow` was not run for this release.

## Standing rule

This release is not one under the standing rule "a release after every improvement" of
`docs/releasing.md`: it has no measured run of its own and claims no measured change. It rests on the
operator's order above.

## What it does not cover

- Any later release, tag or version number; each is its own order.
- A claim about clean passes, cost, wall time or quality of the 0.4.1 tree: no run has used it.
- The runner on the Loom engine inside the daemon, and every branch or open pull request.
