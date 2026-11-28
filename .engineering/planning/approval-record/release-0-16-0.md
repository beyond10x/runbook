---
format: aep.planning-md/3
id: approval-record:release-0-16-0
kind: approval-record
status: draft
title: Release 0.16.0
summary: The session's own decision of 2026-11-28 under the operator's standing word of 2026-11-27 (releases of this repository included). Covers the tag 0.16.0 at cdfb14df, its GitHub Release with four archive assets, and its checks.
revision: 1
---
# Approval: release 0.16.0

## Decision

The operator's standing word of 2026-11-27 at about 15:2x CET, relayed by the host's operator session: this
repository's session takes every open decision of its own on its recommended option and reports it as taken,
releases of this repository included. Under that word the session decided on 2026-11-28 to release 0.16.0 from
`main` at `465a7f5e355a6aa34f744980889e6a4ef7afb1e4`, after the four consumer gaps found on 0.15.0 were merged, and
reported the decision as taken. Runbook 0.16.0 is a minor release of 0.15.0 with no change marked breaking
(`docs/releases/0.16.0.md`). The bot `b10x-bot[bot]` merged the preparation, tagged `main` at the merge commit and
published the GitHub Release.

## What it covers

- Release 0.16.0 of this repository from `main` at `cdfb14dfda6743288254a12e1dd29f0d7b49739b`, the bot's merge of
  pull request 147 (`release/0.16.0`, head `62df5c924f7705c75fc49b670ea588200d82fa46`) onto `main` at
  `465a7f5e355a6aa34f744980889e6a4ef7afb1e4`, made 2026-11-28T05:02:49Z. The four check runs of that head ended
  success before the merge (`Protocol checks`, `Documentation validation`, `Release archives`, `Shared source gates`).
- What is in it: `docs/releases/0.16.0.md` and the `[0.16.0]` section of `CHANGELOG.md`.
- The check runs of the merge commit, all completed success: `Shared source gates` (twice, one of them on the tag
  push), `Protocol checks`, `Documentation validation`, `Documentation site` and `Release archives`. The tag was made
  after `Shared source gates` on the merge commit ended success.
- The annotated tag `0.16.0` by `b10x-bot[bot]` (tag object `e39fd71897dd0d0bc8c1d4e00559ffb6ae6731d8`, peeled
  `cdfb14dfda6743288254a12e1dd29f0d7b49739b`, message "SDC 0.16.0"), made 2026-11-28T05:07:10Z.
- The GitHub Release `0.16.0` with the notes `docs/releases/0.16.0.md`, published 2026-11-28T05:07:58Z
  (https://github.com/beyond10x/runbook/releases/tag/0.16.0), and its four assets, uploaded by the release workflow:
  `runbook-0.16.0-x86_64-unknown-linux-gnu.tar.gz` (23070863 bytes), its `.sha256`,
  `runbook-0.16.0-x86_64-unknown-linux-musl.tar.gz` (23487013 bytes) and its `.sha256`. Downloaded, `sha256sum -c`
  reported both archives OK, and the musl archive's `bin/runbook --version` printed `runbook 0.16.0 (cdfb14dfda67)`.

## Gates of the preparation

On the preparation tree (`docs/releasing.md` step 2): `task release:check VERSION=0.16.0` exit 0; `task check` ran
the driver (987 passed), runbook (2081 passed), `watch/` (52) and `goals/` (41) test steps green, and its UI step
stopped at its 120 s bound at a load average near 37 with 683 passed and none failed; the UI step alone then passed
687 in 98.7 s, and the three commands after it (`scripts/check-protocols.sh`, `scripts/check-bindings.sh`,
`scripts/evaluate-cases.sh`) each exited 0 when run alone. `task check:protocols:slow` exit 0 (9 of 9 protocols);
`b10x-gates check --receipt` common checks passed; `task check:names` 0 refused. One case found racing under load was
changed in the second preparation commit and passed 20 of 20 stress runs.
