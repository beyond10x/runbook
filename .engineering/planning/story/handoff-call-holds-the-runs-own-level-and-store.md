---
format: aep.planning-md/3
id: story:handoff-call-holds-the-runs-own-level-and-store
kind: story
status: draft
title: The runner's handoff call holds the run's own level and store, whatever Loom printed
summary: The command a handoff prints is run as printed, with its level, store and a bound after the separator. No reach with the pinned Loom. From security pass 3 of the zero-caps unit.
relations:
- informed_by: story:caps-are-optional
revision: 1
---
## Outcome

The runner's call to Loom for a handoff names the level and the store of the run itself, whatever words the previous Loom call printed. The runner runs no word of a printed command as printed.

## Source

Security pass 3 of `story:run-without-model-node-needs-no-caps` measured, with an ignored case, that the command a handoff prints is run as printed: a printed confinement level, a printed store and a bound placed after `--` all reach the call (`runbook/src/loom_run/child_host.rs:101`). It has no reach today: the pinned Loom prints its own level and store, the library way makes no handoff call, and the child way starts no step under `substrate`.

## Scope

- `runbook/src/loom_run/child_host.rs` (the handoff call).
- `runbook/tests/loom_run/`: the ignored measurement becomes a case of the default step, with a double of Loom that prints another level and store.

## Acceptance

- With a Loom double that prints a handoff command naming another level and another store, the call the runner makes carries the run's own level and store, and a case in `runbook/tests/loom_run/` shows it. Where the runner adds a bound of its own, it stands before any `--`.

## Origin

Introduced by the zero-caps unit. The part that concerns a bound for steps may be removed by `story:caps-are-optional`; the level and the store are not part of that change. Held by the review record of the unit's security pass 3 (finding F4).
