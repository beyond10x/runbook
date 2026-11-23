---
format: aep.planning-md/3
id: story:start-form-names-an-earlier-run-and-selects-nothing
kind: story
status: draft
title: The start form names an earlier run and selects no runbook by itself
summary: 'Two notes of security pass 4 of daemon-page-starts-a-run that the unit did not take: a reopened form says nothing of the run it started, and a list of one runbook is shown as selected.'
relations:
- informed_by: story:daemon-page-starts-a-run
revision: 1
---
## Outcome

The start form of the daemon's page says what it did before and selects nothing a person did not pick.

## Source

Security pass 4 of `story:daemon-page-starts-a-run` (2026-11-23,
`review-result:security-daemon-page-starts-a-run-pass-4`), two notes that the unit did not take:

- A form that is opened again after a run was started from it says nothing of that earlier run; a person
  can fill it again without seeing that a run of the same intake exists.
- When the list of runbooks holds one entry, the form shows it as selected although the person picked
  none. The confinement level is never preset; the runbook should follow the same rule.

## Scope

- `ui/daemon/start.js`, `ui/web/app.js`.
- `ui/test/`: one case per sentence of the acceptance.

## Acceptance

- A form opened after a start from the same page names the run that start made (its id, as text) until the
  page is loaded again.
- No runbook is selected when the form opens, also with one entry in the list; Start stays disabled until
  a runbook and a level are picked.
- The guards of the unit hold unchanged: the token in page memory only, no level preset, one filled form
  starts at most one run, nothing sent before the floor is read.

## Not in this story

- What a step produced on the daemon's page (`story:daemon-page-shows-step-products`, not filed yet).
