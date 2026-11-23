---
format: aep.planning-md/3
id: story:experiment-passes-efforts
kind: story
status: draft
title: sdc-experiment passes the effort per node class to a run on Loom
summary: 'Found while wave 2026-11-22-w1 ran: sdc-experiment refuses efforts in a loom variant although the runner has the settings key.'
tags:
- backlog-2026-11-22-w1
relations:
- informed_by: review-result:adversary-line2-units-4-6-pass-1
- depends_on: story:effort-levels-per-class
revision: 1
---
## Outcome

A registered experiment can set the effort per node class for a variant that runs on Loom:
`sdc-experiment` passes `efforts.<class>.effort` of a `loom` variant to `runbook run` instead of
refusing the record.

## Source

- `driver/src/bin/sdc-experiment.rs:455` (line 2 of wave `2026-11-22-w1`): `efforts.*` in a
  `loom` variant is refused.
- Unit 6 of that wave gave the runner the settings key `efforts.<role>.effort` and `sdc-node` the
  option `--effort` (`docs/loom-run-settings.md`).
- `review-result:adversary-line2-units-4-6-pass-1`: the refusal named a wrong reason; fix round 2
  corrected the text and did not build the pass-through.

## Acceptance

- `sdc-experiment check` accepts a record whose `loom` variant sets `efforts.<class>.effort`, and
  refuses a class that `bindings/tools/node-classes.json` does not name.
- `sdc-experiment run` writes the value into the settings file it hands to `runbook run`; a test
  with a stand-in adapter reads the effort each model node received. No model call.
- The effort is part of the run's manifest, so that two variants that differ only in it are told
  apart in the ledger.

## Scope

- `driver/src/bin/sdc-experiment.rs`, `driver/tests/`.
- `docs/eval-design.md` (the record's members).

## Not in this story

A measured run with changed efforts: it needs a registered hypothesis and the operator's word on
its cost.
