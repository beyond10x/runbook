---
format: aep.planning-md/3
id: story:intake-classifier-entry
kind: story
status: draft
title: The default intake entry classifies a ticket into a case from the consumer's table
relations:
- informed_by: story:orchestrate-intake-triage
revision: 1
---
## Outcome

The default intake entry classifies a request (a ticket key) into a case from data the consumer supplies: a table
from component to repository and from ticket type to protocol.

## Source

A consumer's plan (2026-11-25), against `docs/intake.md` and `docs/orchestration.md`. No command or refusal was
recorded; this is a planned need.

## Acceptance

- A ticket whose component is in the consumer's table yields a bug-fix case that names the repository.
- A ticket whose component is not in the table yields a case outcome that names the component.
- The table is the consumer's file; Runbook ships no consumer's values.

## Open

Where the table lives (a runbook input, a bindings file) and its format are to be decided when the story is scoped.
