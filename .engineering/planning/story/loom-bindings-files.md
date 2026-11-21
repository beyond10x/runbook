---
format: aep.planning-md/3
id: story:loom-bindings-files
kind: story
status: active
title: Runbook's nine bindings files in loom.bindings/1, loaded by Loom's loader
summary: 'Step 1 of the move onto Loom: bindings/<protocol>.yaml beside the JSON files, PENDING.md for what the format lacks, a plain-text CI check and a load check with Loom''s own loader.'
tags:
- conductor-succession
- loom
relations:
- decomposes: epic:conductor-succession
- informed_by: story:bindings-to-loom-bindings-1
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: bindings
- confidence: cited
  path: docs/loom-bindings.md
- confidence: cited
  path: scripts/check-bindings.sh
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T21:36:38Z", actor: "human:operator", revision: 2, executor: "agent:feat-loom-bindings", correlation: "operator-standing-acceptance-2026-11-21"}
- {from: "proposed", to: "active", at: "2026-11-21T21:36:38Z", actor: "human:operator", revision: 3, executor: "agent:feat-loom-bindings", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: loom-bindings-files

## Outcome

Runbook's nine bindings files exist in Loom's format `loom.bindings/1` under `bindings/`, one per
protocol (`sdc`, `sdc-intake`, `sdc-direction`, `sdc-epic`, `sdc-spec`, `sdc-plan`, `sdc-story`,
`sdc-release`, `sdc-bugfix`), beside the JSON files the driver still runs from. Every construct the
format cannot express yet is listed in `bindings/PENDING.md` with the Loom story or wave that
brings it, never written in invented syntax. This is step 1 of the move of Runbook onto Loom (the
operator, 2026-11-21: "runbook runs on loom entirely").

## Context

The map of the differences: the Loom session's comparison of `driver/examples/sdc/*.bindings.json`
with `loom.bindings/1` at Loom commit `8a8fad3b` (39 differences). Decisions taken with Loom: the
default timeout stays 60 s, so every action that runs a build, a test suite, an agent or a decider
states `timeout_seconds`; the program-error and not-called exit lists are each binding's own and
wait for Loom story G1; a bound program gets `LOOM_CASE`, its arguments on standard input and
`PATH` alone; retry feedback arrives as `LOOM_RETRY_FEEDBACK`; agent actions are bound to stub
programs until Loom's `agent` executor (wave 4).

## Acceptance

- Each of the nine `bindings/<protocol>.yaml` files is accepted by Loom's own loader
  (`b10x_loom_bindings::decode` and `check` at a named Loom commit) against
  `protocol/<protocol>.protocol.yaml`, run from a program outside the repository.
- `scripts/check-bindings.sh` exits 0 and fails when an action bound in a JSON file is neither in
  the YAML file nor in `bindings/PENDING.md`, or when a YAML file binds an action its protocol does
  not declare.
- `docs/loom-bindings.md` gives the field mapping, the timeouts, what is pending on which Loom wave,
  how to run the load check, and the list of tool changes for Loom's program contract (step 2).
- The JSON files and the driver are unchanged.

## Scope

`bindings/`, `scripts/check-bindings.sh`, `docs/loom-bindings.md`, `AGENTS.md`, `CHANGELOG.md`.
