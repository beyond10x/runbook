---
format: aep.planning-md/3
id: story:start-form-keeps-the-key-of-each-content-it-started
kind: story
status: draft
title: The start form starts content it already started at most once, also after other content
summary: Content A, then B, then A again gets a new key and a second run of A. A note of security pass 2 of the page-start unit that its own rule leaves open.
relations:
- informed_by: story:daemon-page-starts-a-run
revision: 1
---
## Outcome

A filled form of the daemon's page that has been started once is started at most once, whatever other content was started between two presses of it in the same page load.

## Source

Security pass 2 of `story:daemon-page-starts-a-run` measured that content A pressed, then content B pressed, then content A pressed again gets a new key and starts a second run of A (`ui/test/security-start-key.spec.mjs:132`, two runs, the note of the form on screen). The rule of the unit is that the key changes when the content changes, and a committed case asserts that rule, so the unit did not take it. It matters most when the answer of the first press was not read.

## Scope

- `ui/daemon/start.js` (the key per content).
- `ui/test/`: the case at `security-start-key.spec.mjs:132` turns from a measurement into a guard.
- `docs/daemon-api.md` (the sentence on the key).

## Acceptance

- In one page load, content A started (answer read or not), content B started, content A pressed again: the second press of A carries the key of the first press of A and starts no second run. A case in `ui/test/` shows it, and the key rule of the unit holds unchanged for content that was not started before.

## Origin

Introduced by the page-start unit; a note by design of the unit's own rule. It fits with `story:submit-run-key-is-bound-to-its-content`, which binds a key to its content in the daemon. Held by the review record of the unit's security pass 2 (note P4), still open at the unit's head.
