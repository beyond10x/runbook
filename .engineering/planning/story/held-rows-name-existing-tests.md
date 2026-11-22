---
format: aep.planning-md/3
id: story:held-rows-name-existing-tests
kind: story
status: draft
title: runbook check refuses a held row whose named test does not exist
summary: 'Pre-existing finding of the adversary on unit A of wave 2026-11-22-w1 (pass 2): the check does not read the test a row of docs/cycles-suite.md names.'
tags:
- from-finding
relations:
- informed_by: review-result:adversary-unit-a-pass-2
revision: 1
---
## Outcome

`runbook check` refuses a row of `docs/cycles-suite.md` whose state is `held by a Rust test`
when the test the row names does not exist.

## Finding

`review-result:adversary-unit-a-pass-2` (origin pre-existing, verdict NEEDS-CHANGE, severity
warning): `docs/cycles-suite.md:197`, `runbook/src/check.rs:546-566`. A row names the test that
holds its cycle, and the check does not read the name: a row naming a test nobody wrote passes
with exit 0. At the base of line 3 (`4677c2b`) the table was not read at all. At `985e8ee` every
named test exists (the reviewer's script), so nothing is wrong today.

## Acceptance

- For each row in the state `held by a Rust test`, `runbook check` finds a `fn` of the named test
  under `runbook/tests/`, `runbook/src/`, `driver/tests/` or `driver/src/`, and names the row and
  the test when there is none. It reads the sources; it runs no test.
- The reviewer's case (a row naming a test that does not exist exits 1) is a test of the check.
- `task check:cycles-suite` stays within 5 s.

## Scope

- `runbook/src/check.rs`, `runbook/tests/cli.rs`.
- `docs/cycles-suite.md` (the sentence that says what the check holds).

## Not in this story

Whether the named test covers what the old cycle covered: a person's review.
