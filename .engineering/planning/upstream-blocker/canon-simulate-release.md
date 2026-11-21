---
format: aep.planning-md/3
id: upstream-blocker:canon-simulate-release
kind: upstream-blocker
status: open
title: 'Canon: release carrying canon simulate and revises/author/decided_by'
tags:
- canon
relations:
- blocks: story:simulate-as-a-protocol-gate
- blocks: story:protocols-declare-revises-and-authors
revision: 2
---
Waiting on Canon: a release carrying `canon simulate` and the `protocol/1` fields `revises`,
`author` and `decided_by` (Canon branch `wave/2026-11-20` at `49ff2ce`, unreleased on
2026-11-21). Accepted by Canon; reported by the Canon session, 2026-11-21.

## SDC state, 2026-11-21

The gate no longer waits for `canon simulate` itself: it runs on the pinned commit `a1a937ec`
(`protocol/simulate/CANON_REV`, decision D151) with pinned reports in `protocol/simulate/`. What
this blocker still holds for `story:simulate-as-a-protocol-gate`: the release that replaces the
pin by the released version, and the re-pin of the reports from it; zero findings is a later
target, reached when Canon's waves 6-8 are released.
