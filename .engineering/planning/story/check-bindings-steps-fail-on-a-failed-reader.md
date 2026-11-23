---
format: aep.planning-md/3
id: story:check-bindings-steps-fail-on-a-failed-reader
kind: story
status: draft
title: Every step of check-bindings.sh fails when one of its readers fails
summary: 'Two findings of adversary pass 4 of check-bindings-exits-reads-any-runbook in steps that exist on main: the instruction-file step and the answers-file step go on after a reader failed.'
relations:
- informed_by: story:check-bindings-exits-reads-any-runbook
revision: 1
---
## Outcome

Every step of `scripts/check-bindings.sh` fails the script when one of its readers fails.

## Source

Adversary pass 4 of `story:check-bindings-exits-reads-any-runbook` (2026-11-23), two findings in steps that
exist on `main` and that the unit did not change:

- The step that reads the instruction files of the bindings (`scripts/check-bindings.sh`, near line 232)
  goes on when a reader it starts ends with a failure; the step can print `pass` for a file it did not read.
- The step that reads the answers file (near lines 310 and 317) does the same.

The unit gave the exits step and the registrations step this rule (a reader runs first and its failure
fails the script); these two steps are older.

## Scope

- `scripts/check-bindings.sh` (the instruction-file step, the answers-file step).
- `driver/tests/`: one case per step with a file its reader cannot read.

## Acceptance

- A file that a reader of either step cannot read ends the script with exit 1 and a `FAIL` line that names
  the file; no `pass` line of that step is printed.
- The script's output for this repository's committed files is unchanged.

## Not in this story

- The table of every member with every JSON type (`story:check-bindings-exits-reads-any-runbook`).
