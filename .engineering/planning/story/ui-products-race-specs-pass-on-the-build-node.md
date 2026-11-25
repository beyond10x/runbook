---
format: aep.planning-md/3
id: story:ui-products-race-specs-pass-on-the-build-node
kind: story
status: draft
title: The products-race UI specs pass on the build node
revision: 1
---
## Outcome

The two products-race UI specs, `ui/test/security-products-race.spec.mjs:27` and `ui/test/security-products-2-race.spec.mjs:27`, pass on every machine the gate runs on. On the build node they time out at 30 s in every run of 2026-11-25 (all four model-route units), while they pass on the workstation; no unit that saw them changed `ui/`.

## Acceptance

- The cause of the node-only timeout is named with evidence (timing of the spec's waits on the node against the workstation).
- The fix is in the specs (their waits, their clock or their fixture), not in production code.
- Both specs pass 10 times in a row on the node and on the workstation, each under 30 s.
