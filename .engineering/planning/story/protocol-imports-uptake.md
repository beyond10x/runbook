---
format: aep.planning-md/3
id: story:protocol-imports-uptake
kind: story
status: proposed
title: Share protocol declarations through --import
tags:
- blocked-upstream
- canon
relations:
- decomposes: epic:kernel-and-runtime-uptake
- depends_on: story:canon-slice-bound-uptake
scope:
- confidence: inferred
  path: protocol/compiled
- confidence: inferred
  path: protocol/sdc-direction.protocol.yaml
- confidence: inferred
  path: protocol/sdc-epic.protocol.yaml
- confidence: inferred
  path: protocol/sdc-plan.protocol.yaml
- confidence: inferred
  path: protocol/sdc-release.protocol.yaml
- confidence: inferred
  path: protocol/sdc-spec.protocol.yaml
- confidence: inferred
  path: protocol/sdc-story.protocol.yaml
- confidence: inferred
  path: protocol/sdc.protocol.yaml
- confidence: inferred
  path: protocol/shared
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:37Z", actor: "human:timo", revision: 3}
---
# Story: protocol-imports-uptake

## Outcome

A declaration shared by several SDC protocols is written once and imported, so the copies can no
longer drift apart.

## Context

Canon `story:protocol-imports` (wave 2, not released): `--import` on compile, validate and check,
pinned by revision and sha256 of the compiled IR. Copies today:

- evidence kind `roadmap_approval` in five protocols (`protocol/sdc.protocol.yaml:22`,
  `sdc-direction.protocol.yaml:55`, `sdc-epic.protocol.yaml:15`, `sdc-spec.protocol.yaml:22`,
  `sdc-plan.protocol.yaml:21`);
- evidence kind `ess_gate` in four (`sdc-epic.protocol.yaml:17`, `sdc-plan.protocol.yaml:23`,
  `sdc-release.protocol.yaml:21`, `sdc-story.protocol.yaml:32`), whose descriptions already differ
  (`sdc-epic.protocol.yaml:18-19` against `sdc-plan.protocol.yaml:24-25`);
- artifacts `roadmap` and `spec` declared in several protocols (e.g. `sdc-epic.protocol.yaml:8`, `:12`).

## Acceptance

With the first Canon release after 0.9.0 pinned, `roadmap_approval` and `ess_gate` are each
declared in exactly one imported file, and `scripts/check-protocols.sh`,
`scripts/check-bindings.sh` and `scripts/evaluate-cases.sh` exit 0.

## Scope

The eight `protocol/*.protocol.yaml`, their compiled IR, a new shared declarations file under
`protocol/`.
