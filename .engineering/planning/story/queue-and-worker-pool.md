---
format: aep.planning-md/3
id: story:queue-and-worker-pool
kind: story
status: draft
title: Cases wait in a queue worked by N workers under a work-in-progress limit
revision: 1
---
## Outcome

Cases wait in a queue and N workers take them, with a work-in-progress limit (`docs/orchestration.md` § 2.4a); the
consumer sets N.

## Source

A consumer (2026-11-25). Today a case is one submitted run; no command was recorded.

## Acceptance

- With N = 2, two queued cases start within one minute of each other and the third waits until one ends.
- N is a setting of the consumer, not a constant.

## Open

Whether the queue is the daemon's (submits beyond N wait) or a separate component is architecture and goes to the
operator before code.
