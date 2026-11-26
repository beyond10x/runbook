---
format: aep.planning-md/3
id: story:composer-opens-independent-children-in-parallel
kind: story
status: draft
title: A composer opens independent children in parallel
relations:
- informed_by: story:loom-concurrent-children
revision: 1
---
## Outcome

A composer opens child cases whose openers have no ordering edge concurrently, bounded by a stated limit, so a round
takes about its longest stage rather than the sum of its stages.

## Source

A consumer, 2026-11-25 (relayed by the host operator). Loom opens child cases one at a time, so "gates in parallel" of
a protocol does not hold. Low priority: it changes round time, no outcome.

## Acceptance

- A fixture with 3 independent children of 2 s each (sleep stubs) finishes in under 4 s wall; the order of records is
  deterministic by child id. No model.

## Related

- `story:loom-concurrent-children` (proposed) and `story:concurrency-limit` (proposed).

## Status

Draft. Concurrent child execution is a change of the engine's scheduling (Loom) and of how a run's records are
ordered: it waits for the operator's decision.
