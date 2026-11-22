---
format: aep.planning-md/3
id: epic:flow-limits
kind: epic
status: proposed
title: Flow limits and flow KPIs
summary: 'Bound unfinished work in multi-item runs: flow KPIs from records, WIP limits enforced at admission, limit changes as events, an eval sweep for the default, the KPIs on the board.'
tags:
- orchestration
relations:
- decomposes: initiative:roadmap
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:34Z", actor: "human:operator", revision: 2}
---
# Epic: flow-limits

## Status note

Accepted by the operator, 2026-11-21: "I do accept the orchestration and parallelism design and upcoming epics and stories. So write them, scope them, implement it, but when it's time for it. So we're probably focusing first on the non-orchestration bits." Drafted 2026-11-21 under that acceptance; implementation waits until the non-orchestration work (AGENTS.md, "Work order") is done.

Addition by the operator, 2026-11-21: "for orchestration or multi-work item protocols there are additional
KPIs: concurrent unfinished work, open work trees, in-progress AEP work items. This should be
constrained within a healthy amount. I've seen agents dispatching work all the time, nothing being
finished, ending up with 100+ work trees, so that to get started at all we have to reconcile all
the state. This should be prevented while still speeding up through parallelism. There is a sweet
spot; we want to tweak that during the run, or run evals and check which setting delivers better."
(shortened by the coordinator.)

## Outcome

A multi-item run never accumulates unfinished work beyond its limits, and the limit that gives the
best throughput at zero orphans is chosen by measurement. Design: `docs/orchestration.md` § 2.4a,
KPIs F1-F17 in `docs/kpis.md` § 5.

## Why Now

Parallelism without a WIP bound produced 100+ worktrees and a reconcile before every start (the operator,
above). Scheduled with the orchestration epics, after the non-orchestration work.

## Scope

- Flow KPIs F1-F16 computed from case events, the planning store, `git worktree list` and the
  fs-change record.
- Limits `limits.wip.max_in_progress`, `max_open_worktrees`, `max_unmerged`, `max_item_age`,
  enforced by `wave.admit` (finish before start), never by an LLM.
- `limits_changed` event (who, old, new), read at the next admission, never retroactive.
- Eval sweep of the WIP limit (1, 2, 4, 8), default per level (F17).
- The KPIs and limits on the area board.

## Out of Scope

- Who may change a limit beyond "a person or a rule, never an LLM" (`docs/autonomy-levels.md:271-272`).

## Risks

- F14 and F15 need a sequential baseline run of the same suite on a shared host (noisy wall time).

## Ambiguities

- Default values of the four limits before the sweep: requires-stakeholder-input (the operator), or the
  sweep's result.

## Done When

Admission refuses a wave over a limit naming the limit and the items, the sweep report exists for
the standard suite, and the board shows F1, F3-F6 against their limits.
