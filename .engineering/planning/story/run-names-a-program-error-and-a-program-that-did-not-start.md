---
format: aep.planning-md/3
id: story:run-names-a-program-error-and-a-program-that-did-not-start
kind: story
status: draft
title: A run ended by a program error or a program that did not start ends with a named cause
summary: Only a program ended by a signal is lifted from the run log; the other two end stuck with the generic text. From adversary pass 1 of the Loom pin unit db2726ff.
relations:
- informed_by: story:loom-pin-db2726ff
revision: 1
---
## Outcome

A run that ends because a program of an action ended in an error class of its binding, or because the runner of the program could not start it, ends with a named cause and the reason Loom gave, not with the generic stuck text.

## Source

Adversary pass 1 of `story:loom-pin-db2726ff` measured that only a program ended by a signal is lifted out of the last `ActionFinished` line of a case (`runbook/src/engine/refusal.rs:157`). A program that exits with a code listed under `exits.error`, and a program its runner could not start, still end the run `Stuck` with exit 3, the cause `stuck` and the generic text. The acceptance of that unit asked for named causes "never as a catch-all text" for the causes it added, so this is outside it.

## Scope

- `runbook/src/engine/refusal.rs` (the lifting of a refusal from the run log), `runbook/src/loom_run/` (the outcome and `run-stopped.json`).
- `docs/loom-run.md`, `docs/loom-run-directory.md` (the list of causes and their count).
- `runbook/tests/loom_run/`, `runbook/tests/engine/`: one case per refusal kind.

## Acceptance

- A run whose last action ended in a program error, and one whose program could not be started, end with an outcome whose cause names the kind and whose text holds the reason Loom printed, in `outcome.json` and `run-stopped.json`. A case for each is in the default step, and the two pages list the new causes with the right count.

## Origin

Pre-existing: both kinds ended stuck at the earlier Loom pin too. Held by the review record of the adversary pass 1 of `story:loom-pin-db2726ff` (finding 5); fix round 1 left it out by decision.
