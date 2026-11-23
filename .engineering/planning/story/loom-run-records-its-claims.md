---
format: aep.planning-md/3
id: story:loom-run-records-its-claims
kind: story
status: draft
title: A run on Loom records the claims of each evaluation
summary: 'Found while wave 2026-11-22-w1 ran: the inspector prints claims not recorded for a run directory written by runbook run.'
tags:
- backlog-2026-11-22-w1
relations:
- informed_by: story:driver-runtime-removed
revision: 1
---
## Outcome

A run on Loom records what each evaluation decided about the claims of the case, so that the
inspector shows the count of satisfied and open claims of a Loom run. Today it shows "claims not
recorded for this run".

## Source

- Branch `fix/inspector-any-runbook` (`9189b26`), `ui/test/loom-run-record.spec.mjs`: the
  inspector prints "claims not recorded for this run" for a run directory written by `runbook run`.
- The inspector reads the count from three places a run of the deleted driver wrote:
  `store/<root>/evaluations/<NNNN>.json` (`canon-decision/1`, member `claims`), the claims map of
  `ir.json`, and an `evaluated` event.
- Loom's run log has the line `Evaluated` with `case_revision`, `frontier` and `decision_digest`
  only: no claims.
- Found while wave `2026-11-22-w1` ran; not a unit of it.

## Acceptance

- After each evaluation of a run, the runner writes the decision with its `claims` where the
  inspector reads it, or the inspector reads it from the place Loom's store keeps it. One of the
  two, named in `docs/driver.md`.
- The runner's projection of the run log carries an `evaluated` event per `Evaluated` line.
- A UI test with a hand-written run record (protocol `example@1`, invented text) shows the
  count; the test of "claims not recorded for this run" stays for a record without evaluations.
- No recorded model output enters a fixture.

## Scope

- `runbook/src/loom_run/` (the projection of the run log, the run directory).
- `ui/runfacts.mjs`, `ui/web/`, `ui/test/`.
- `docs/driver.md`.

## Not in this story

A change of Loom's run log format: asked of the Loom session if the decision cannot be read
from Loom's store.
