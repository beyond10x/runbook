---
format: aep.planning-md/3
id: story:child-tool-model-calls-are-booked-and-capped
kind: story
status: draft
title: Model calls a child tool makes itself are booked and capped in the run's ledger
relations:
- informed_by: story:model-entry-for-bound-programs
revision: 1
---
## Outcome

A program step can start a tool with the run's model endpoint and ledger (a local proxy or an environment contract),
and each model call the tool makes itself is a ledger line against the run's caps.

## Source

A consumer, 2026-11-25 (relayed by the host operator). A tool that calls a model itself (an extraction tool) is
invisible to the runner: such a step cannot be a daemon runbook, and its calls skip the run's spend cap and call
limit.

## Acceptance

- A stub tool that makes 3 calls to the provided endpoint (a stub server) shows 3 ledger lines and the run ends
  `stopped: spend cap` when the cap is 2. No real model.

## Related

- `story:model-entry-for-bound-programs` (active): bound programs call a model through one documented entry. This is
  the tool-in-the-middle case; fold it in if that story's scope covers a tool the program starts.
- The tool may need its own endpoint option, decided in its own project.

## Status

Draft. A proxy or contract that books a third-party tool's calls inside the run's confinement is a change of
architecture: it waits for the operator's decision.
