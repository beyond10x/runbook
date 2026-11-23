---
format: aep.planning-md/3
id: story:loom-run-records-its-claims
kind: story
status: active
title: A run on Loom records the claims of each evaluation
summary: 'Found while wave 2026-11-22-w1 ran: the inspector prints claims not recorded for a run directory written by runbook run.'
tags:
- backlog-2026-11-22-w1
relations:
- informed_by: story:driver-runtime-removed
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T03:08:21Z", actor: "human:operator", revision: 3, executor: "agent:runbook", correlation: "operator-2026-11-23-consumer-gaps"}
- {from: "proposed", to: "active", at: "2026-11-23T03:08:21Z", actor: "human:operator", revision: 4, executor: "agent:runbook", correlation: "operator-2026-11-23-consumer-gaps"}
---
## Outcome

A run on Loom records what each evaluation decided about the claims of the case, and the inspector
shows it per step and for the case: which claims an action settled, with their evidence, and on a
stuck or open case which claim or precondition is unmet and what evidence it waits for. Today the
inspector prints "claims not recorded for this run".

## Source

- The consumer session of the operator's own runbooks, 2026-11-23, on the operator's order that its
  gaps go first. What each step should show under "claims", in its words:
  "1. Each claim the action settled: claim id, holds / fails / open, the revision it was settled
  at, and its evidence records (kind, subject, result, producing action, revision). 2. For the
  case: which claims led to the outcome, and on a stuck or open case, which precondition or claim
  is unmet and what evidence it waits for. I did not need the handoff checks for preflight."
- Branch `fix/inspector-any-runbook` (`9189b26`), `ui/test/loom-run-record.spec.mjs`: the
  inspector prints "claims not recorded for this run" for a run directory written by `runbook run`.
- The inspector reads the count from three places a run of the deleted driver wrote:
  `store/<root>/evaluations/<NNNN>.json` (`canon-decision/1`, member `claims`), the claims map of
  `ir.json`, and an `evaluated` event.
- Loom's run log has the line `Evaluated` with `case_revision`, `frontier` and `decision_digest`
  only: no claims.

## Acceptance

- After every evaluation the runner writes one record of the decision into the run directory
  (`claims/<revision>.json`): every claim of the case with its id, its state (holds, fails, open),
  the revision at which that state was reached, and its evidence records (kind, subject, result,
  producing action, revision). The source is Loom's decision for that revision, read through the
  library; where the library gives the runner no decision, the unit reports it and the request
  goes to the Loom session (no second evaluation in the runner).
- `outcome.json` names, for the case, the claims that led to the outcome; for a stuck or open case
  it names each unmet claim or precondition and the evidence kind it waits for.
- The inspector shows, for a step, the claims that changed state at the revision the action
  produced, with their evidence records, and for the case the list of `outcome.json`. A run
  without the records shows "claims not recorded for this run" as today.
- The text of an evidence record is content of a run: it is shown as text, never as markup.
- `docs/loom-run-directory.md` states the members. Tests with sh programs and a hand-written
  protocol; no model call; no test over 60 s.

## Scope

`runbook/src/loom_run/run_dir.rs`, `runbook/src/loom_run/run_loop.rs`, `runbook/src/engine/`,
`runbook/tests/`, `ui/`, `docs/loom-run-directory.md`, `CHANGELOG.md`.

## Not in this story

- The checks of a handoff.
