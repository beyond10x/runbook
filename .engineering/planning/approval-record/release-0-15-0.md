---
format: aep.planning-md/3
id: approval-record:release-0-15-0
kind: approval-record
status: draft
title: Release 0.15.0
summary: The session's own decision of 2026-11-28 under the operator's standing word of 2026-11-27 (releases of this repository included). Covers the tag 0.15.0 at d1ac5276, its GitHub Release with four archive assets, and its checks.
revision: 1
---
# Approval: release 0.15.0

## Decision

The operator's standing word of 2026-11-27 at about 15:2x CET, relayed by the host's operator session: this
repository's session takes every open decision of its own on its recommended option and reports it as taken,
releases of this repository included. Under that word the session decided on 2026-11-28 to release 0.15.0 from
`main` at `01c9f4c7e57feb4c22133aac086a2a72b45188ea`, after the consumer's gaps G1 to G6 (G2 as its first slice)
were merged, and reported the decision as taken. Runbook 0.15.0 is a minor release of 0.14.0 with three changes
marked breaking (`docs/releases/0.15.0.md`). The bot `b10x-bot[bot]` merged the preparation, tagged `main` at the
merge commit and published the GitHub Release.

## What it covers

- Release 0.15.0 of this repository from `main` at `d1ac5276aac0099237ef1f70f60b32bf61531ed2`, the bot's merge of
  pull request 143 (`release/0.15.0`, head `6a0e6c1afc7681aac7d0461afa0dae7dc6aa411c`) onto `main` at
  `01c9f4c7e57feb4c22133aac086a2a72b45188ea`, made 2026-11-28T01:00:58Z. The four check runs of that head ended
  success before the merge (`Release archives`, `Documentation validation`, `Protocol checks`, `Shared source
  gates`).
- What is in it: `docs/releases/0.15.0.md` and the `[0.15.0]` section of `CHANGELOG.md`.
- The check runs of the merge commit, all completed success: `Protocol checks`, `Shared source gates` (twice, one
  of them on the tag push), `Documentation validation`, `Documentation site` and `Release archives`. The tag was made
  before `Documentation site` ended; it deploys the site and checks no code.
- The annotated tag `0.15.0` by `b10x-bot[bot]` (tag object `ef1d433b370092a0b4deedca6900ca134d6ca2fc`, peeled
  `d1ac5276aac0099237ef1f70f60b32bf61531ed2`, message "SDC 0.15.0"), made 2026-11-28T01:32:36Z.
- The GitHub Release `0.15.0` with the notes `docs/releases/0.15.0.md`, published 2026-11-28T01:33:53Z
  (https://github.com/beyond10x/runbook/releases/tag/0.15.0), and its four assets, uploaded by the release workflow:
  `runbook-0.15.0-x86_64-unknown-linux-gnu.tar.gz` (23069927 bytes), its `.sha256`,
  `runbook-0.15.0-x86_64-unknown-linux-musl.tar.gz` (23487736 bytes) and its `.sha256`. Downloaded, `sha256sum -c`
  reported both archives OK, and the musl archive's `bin/runbook --version` printed `runbook 0.15.0 (d1ac5276aac0)`.

## Gates of the preparation

On the preparation tree (`docs/releasing.md` step 2): `task release:check VERSION=0.15.0` exit 0; `task check`
exit 0 (runbook 2066 passed, driver 987, watch 52, goals 41, UI 687); `scripts/check-protocols.sh` exit 0;
`task check:protocols:slow` exit 0 (9 of 9 protocols); `b10x-gates check --receipt` common checks passed;
`task check:names` 0 refused; `scripts/check-bindings.sh` exit 0.
