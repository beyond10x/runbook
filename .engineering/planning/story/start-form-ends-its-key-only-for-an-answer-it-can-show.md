---
format: aep.planning-md/3
id: story:start-form-ends-its-key-only-for-an-answer-it-can-show
kind: story
status: draft
title: The start form ends its key only for an answer that names a known outcome and a run
summary: An answer with an outcome the command does not have, or a success without a run, ends the key and allows a second run. No reach with this daemon. From security passes 3 and 4 of the page-start unit.
relations:
- informed_by: story:daemon-page-starts-a-run
revision: 1
---
## Outcome

The start form of the daemon's page ends the key of a filled form only for an answer that names an outcome the command has and, for a success, names the run. Any other answer keeps the key.

## Source

Security pass 3 of `story:daemon-page-starts-a-run` measured that an answer which names an outcome the command does not have, or a success with no run, ends the key while the form stays filled, so the next press is a second run (`ui/daemon/start.js:185`, with `:194`; two runs measured). Pass 4 kept it open by decision. This daemon sends no such answer; only a party that rewrites the bodies of its answers can cause it.

## Scope

- `ui/daemon/start.js` (the handling of the answer).
- `ui/test/security-start-late-answers.spec.mjs` and `security-start-answers.spec.mjs`: the measured kinds turn into guards.

## Acceptance

- For every answer shape that names an outcome unknown to the command, and for a success that names no run, a second press of the same content carries the same key and starts at most one run. Cases in `ui/test/` show each shape. Answers of the kinds the daemon sends behave as before.

## Origin

Introduced by the page-start unit; no reach with the daemon of this repository. Held by the review records of the unit's security passes 3 (finding F2) and 4 (note G7).
