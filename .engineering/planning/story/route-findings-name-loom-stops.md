---
format: aep.planning-md/3
id: story:route-findings-name-loom-stops
kind: story
status: draft
title: The routes check names stops of a run on Loom
summary: Five findings of sdc-check-routes name rework_spent and decision_not_takeable, stops of the deleted driver; they say what a run on Loom does instead.
tags:
- from-finding
relations:
- informed_by: review-result:adversary-unit-b-pass-2
revision: 1
---
## Outcome

Every finding the routes check (`sdc-check-routes`) prints names a stop that a run on Loom can
end in. No finding names a stop of the deleted driver.

## Finding

`review-result:adversary-unit-b-pass-2`, finding N3 (origin pre-existing, verdict NEEDS-CHANGE,
severity warning): `driver/src/bin/sdc-check-routes.rs:218` and `:267`. Five findings of the
check at `802e9f5` name stops that nothing in `runbook/src` or `driver/src` writes:

- four lines `rework-limit …` that end in the stop `rework_spent`;
- one line `no-reject-route amend.upstream …` that names `decision_not_takeable`.

The `runtime` block of `driver/examples/sdc/rule-ends.json` says what holds in a run on Loom
(wave `2026-11-22-w1`, unit B), and the change log of that unit says `rework_limit` is not a
count on Loom. The five lines are the same at `985e8ee`.

## Acceptance

- A decision is written down first, per stop: what a run on Loom does when a rule's rework limit
  is reached, and what it does when a finding decision cannot be taken (the stop's name in the
  run's records, or "no stop: the run goes on until the cap of the action", with the Loom
  `file:line` at the pinned commit).
- The five findings name that stop or that behaviour; `grep -n 'rework_spent\|decision_not_takeable'`
  over the output of the routes check prints nothing unless a file under `runbook/src` or
  `driver/src` writes the name.
- A test of the check holds each of the two texts.
- The "Check output" block of `docs/reject-routes.md` equals the live output.
- What Loom does not provide is sent to the Loom session as a gap, with the finding's line.

## Scope

- `driver/src/bin/sdc-check-routes.rs`, `driver/tests/`.
- `driver/examples/sdc/rule-ends.json` (block `runtime`), `docs/reject-routes.md`.

## Not in this story

Reading `bindings/*.yaml` instead of the JSON copies: `story:one-copy-of-the-bindings`.
