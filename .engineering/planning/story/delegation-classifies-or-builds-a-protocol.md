---
format: aep.planning-md/3
id: story:delegation-classifies-or-builds-a-protocol
kind: story
status: draft
title: A delegation classifies its request or builds a protocol for it
revision: 1
---
## Outcome

Without a named protocol, one classifier call picks a runbook at or above the threshold; else one builder call writes a protocol from catalog parts only, checked by the loader; the free run is the fallback. The run records path, protocol, confidence and the generated protocol's digest.

## Acceptance

- A stand-in classifier answer of 0.9 runs that runbook; 0.5 builds.
- A built protocol the loader accepts runs; one it refuses ends failed: protocol-refused with the loader's line.
- read_run shows path, protocol and confidence.

Design: `docs/model-routes.md`, section 9. Tests use a stand-in gateway on loopback: no model call, no pod, no network beyond loopback.
