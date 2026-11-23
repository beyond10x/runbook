---
format: aep.planning-md/3
id: story:suite-entry-changes-state-only-on-runs-inside-the-bound
kind: story
status: draft
title: A suite entry loses pending only after runs that ended inside Loom's 60 s bound
summary: Three entries lost pending on one quiet run each; at the load of the build host the same cycles end on the bound. From adversary pass 1 of the Loom pin unit db2726ff.
relations:
- informed_by: story:loom-pin-db2726ff
revision: 1
---
## Outcome

An entry of `cycles/tree-suite.yaml` loses its `pending` mark, or is called expected in `docs/cycles-suite.md`, only after runs of the cycle that ended inside Loom's 60 s bound at a load average that the page states.

## Source

Adversary pass 1 of `story:loom-pin-db2726ff` measured that three entries (`run-stop`, `decider-rework`, `replay-sibling`) lost `pending` on one run each at a quiet host. At a load average of 72 to 87 one ended as expected in 15.8 s, one in 53.6 s, and one failed on Loom's 60 s bound at 61.1 s. The page names the sensitivity to load for other entries. Later passes show the same: at the next pin no entry was observed to change state, but runs at a load of 76 to 139 mostly ended on the bound, so "no entry changes from expected" was not shown either way.

## Scope

- `cycles/tree-suite.yaml`, `docs/cycles-suite.md` (the paragraphs for each pin).
- The check of `runbook check` that compares the two files.

## Acceptance

- For each of the three entries, three runs alone are listed in `docs/cycles-suite.md`, each with seconds and the load average at the start, each ended inside the bound and as expected. An entry without them is marked `pending` again. `runbook check` fails when the page names fewer runs than the rule asks for an entry that is not pending.

## Origin

Introduced by the pin unit `story:loom-pin-db2726ff`; the rule of one run per entry carried into the units that followed. Held by the review record of its adversary pass 1 (finding 6), not taken by fix round 1.
