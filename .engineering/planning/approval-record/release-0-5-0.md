---
format: aep.planning-md/3
id: approval-record:release-0-5-0
kind: approval-record
status: approved
title: Release 0.5.0
summary: 'Operator''s order, relayed 2026-11-22 about 17:02 CET, and the word of 18:18 CET: tag and release 0.5.0; a minor. Covers the tag 0.5.0 at 44e05a3, its GitHub Release, its checks and what was not run.'
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-22T18:56:27Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
---
# Approval: release 0.5.0

## Decision

On 2026-11-22 at about 17:02 CET (16:02Z) the host's operator session relayed the operator's order:
"cut versions in the repos we delivered work to default branch and can push, tag those, cut
releases". With it: "Minor if a user sees a change, patch for fixes only; no major version." and
"Release from a green default branch only. Work on a branch or an open PR does not go in; do not
merge a branch to make a release." Runbook 0.5.0 is a minor release: a user sees the change
(`runbook run` replaces `runbook loom-run`, `runbook test` is new, and breaking changes are listed in
the notes). On 2026-11-22 at 18:18:32 CET (17:18:32Z), to the question whether to tag and release
0.5.0 with the old driver still in the tree, the operator answered "A: release 0.5.0 now", with "tag
only after green CI on main". The bot `b10x-bot[bot]` tagged `main` at the merge commit and published
the GitHub Release on that order.

## What it covers

- Release 0.5.0 of this repository from `main` at `44e05a37d6c44266849231212b75efabda0bee54`, the
  merge of pull request 25 (`release/0.5.0`, head `2005aae71aa6bd6081eb7caa3cd9c8a538243461`), merged
  2026-11-22T18:04:18Z.
- What is in it: `docs/releases/0.5.0.md` and the `[0.5.0]` section of `CHANGELOG.md`. What is not
  in it, because it is on a branch: `runbook canon`, the inspector that acts on nothing, routing by
  autonomy level, the replay options of `runbook run`, and the Loom pin `70b0e6e0`.
- Checks before the tag:
  - On the pull request head `2005aae71a`: `Shared source gates` (run 45051896955) and
    `Documentation validation` (run 45051896681), both success.
  - On `main` at `44e05a37d6`: four check runs, all success, read at 2026-11-22T18:12:49Z
    (`common / Security and privacy`, `Build documentation`, `Find the built site`,
    `publish / publish`).
  - `task release:check VERSION=0.5.0` exit 0 on the merge commit.
  - Locally on the release branch: the builds, the static checks and `docs:build` exit 0.
- The annotated tag `0.5.0` by `b10x-bot[bot]` (tag object
  `932f9ed97f9f09971b275584eb169124f852e746`, peeled
  `44e05a37d6c44266849231212b75efabda0bee54`, message "Runbook 0.5.0"), pushed 2026-11-22T18:13:37Z.
- The GitHub Release `0.5.0` with the notes `docs/releases/0.5.0.md`, published
  2026-11-22T18:13:42Z (https://github.com/beyond10x/runbook/releases/tag/0.5.0).

## What was not run

A full local `task check` was not run on the release commit. Locally on the release branch the test
step of `runbook/` passed 567 tests with 13 skipped in 181.6 s at a host load average of 122, over
its 120 s budget (29 s on the base); the test step of `driver/` gave no valid local result, because
the command built no examples; and the local `docs:test` ran into its 120 s limit at a host load
average of 86. For the last two the CI runs above are what the release rests on.
`docs/releases/0.5.0.md` § Limits adds that the finding cycles of `cycles/suite.yaml` and
`scripts/run-finding-cycles.sh` were not run on this tree, that `task check:ess-suite:slow` was not
run for this release, and that the old driver (`sdc-drive`, `scripts/run-trial.sh`) is still in the
tree.

## Standing rule

This release is not one under the standing rule "a release after every improvement" of
`docs/releasing.md`: it has no measured run of its own and claims no measured change. It rests on
the operator's word above.

## What it does not cover

- Any later release, tag or version number; each is its own order.
- A claim about clean passes, cost, wall time or quality of the 0.5.0 tree: no run has used it.
- `runbook canon`, the inspector, routing by autonomy level, the replay options and the Loom pin
  `70b0e6e0`, and every branch or open pull request.
