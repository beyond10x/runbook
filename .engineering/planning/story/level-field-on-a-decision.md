---
format: aep.planning-md/3
id: story:level-field-on-a-decision
kind: story
status: draft
title: A decision declares a level that says who answers it
relations:
- informed_by: story:a-run-names-its-classes-document-and-level-table
revision: 1
---
## Outcome

A decision carries a level field, declared by the protocol: who answers it (a person, a model decider, a program), so
a consumer can route a decision by it.

## Source

A consumer (2026-11-25) keeps its own table of 15 decision kinds with their levels in a file and generates its routing
from it, because a decision has no such field. No refusal was recorded.

## Acceptance

- A decision declares a level, and the level reaches the decision request a consumer reads.
- A decision without a level is refused at check time, and the refusal names the decision.

## Related

`story:a-run-names-its-classes-document-and-level-table` (the run names its classes document and level table).
Whether the field is Runbook's or the protocol language's (an upstream change) is decided when the story is scoped.
