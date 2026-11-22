---
format: aep.planning-md/3
id: approval-record:release-0-4-0
kind: approval-record
status: approved
title: Release 0.4.0
summary: 'Operator''s pick, relayed 2026-11-22 13:18 CET: "A: release 0.4.0 now". Covers the tag 0.4.0 at 5955f82, its GitHub Release, its checks and what the operator knew.'
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-22T12:34:04Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
---
# Approval: release 0.4.0

## Decision

On 2026-11-22 at 13:18 CET (12:18Z) the host's operator session relayed the operator's pick "A:
release 0.4.0 now" to the question "tag and release Runbook 0.4.0 now?". The decision reads: "Notes
and version on a branch, bot-route merge with the receipt check, tag and release 0.4.0; notes say the
full finding-cycle set was not rerun and that the engine on Loom is not in it. The 9 runs on 0.3.1
stay untouched." The bot `b10x-bot[bot]` tagged `main` at the merge commit and published the GitHub
Release on that word.

## What it covers

- Release 0.4.0 of this repository from `main` at `5955f827359378701dba0ca900ba19ba6f490073`, the
  merge of pull request 10 (`release/0.4.0`, head `051a287a2b1a8c96a118cd230f3f9bcc297732cd`), merged
  2026-11-22T12:20:14Z.
- Checks before the tag:
  - `b10x-gates check --receipt` valid on `051a287`.
  - On the pull request head: `Shared source gates` (run 45020202951) and `Documentation validation`
    (run 45020202599), both success.
  - `task release:check VERSION=0.4.0` exit 0 on the merge commit.
  - `task check` exit 0 on `051a287`: runbook 434 tests, driver 357, UI 125.
- The annotated tag `0.4.0` by `b10x-bot[bot]` (tag object
  `8c67a97b605075cfbff474614a968ce139489a32`, peeled
  `5955f827359378701dba0ca900ba19ba6f490073`, message "Runbook 0.4.0").
- The GitHub Release `0.4.0` with the notes `docs/releases/0.4.0.md`, published
  2026-11-22T12:20:32Z (https://github.com/beyond10x/runbook/releases/tag/0.4.0).

## What the operator knew

The question put to the operator said: since tag 0.3.1, `main` holds pull request 8 (`runbook
loom-run`) and pull request 6 (the daemon on its stub runtime). The engine that runs a case on Loom
inside the daemon is on a local branch and is not in the release. The full set of finding cycles was
not run again on this tree. `docs/releases/0.4.0.md` § Limits says the same. The operator picked A
with that stated.

## Standing rule

This release is not one under the standing rule "a release after every improvement" of
`docs/releasing.md`: it has no measured run of its own and claims no measured change. It rests on the
operator's word above.

## What it does not cover

- Any later release, tag or version number; each is its own order.
- A claim about clean passes, cost, wall time or quality of the 0.4.0 tree: no run has used it.
- The nine registration runs of `experiments/release-0.3.1.yaml`: they stay on 0.3.1 and are not
  touched by this release.
