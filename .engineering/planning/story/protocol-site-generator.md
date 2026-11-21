---
format: aep.planning-md/3
id: story:protocol-site-generator
kind: story
status: proposed
title: Protocol data for the site, generated from the YAML
summary: sdc-protocol-site writes and checks website/data/protocols from the protocols, bindings and committed check outputs
tags:
- docs
relations:
- decomposes: epic:public-docs-site
scope:
- confidence: cited
  path: driver/Cargo.toml
- confidence: cited
  path: driver/src/bin/sdc-protocol-site.rs
- confidence: cited
  path: scripts/check-bindings.sh
- confidence: cited
  path: website/data/protocols
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T15:09:49Z", actor: "human:operator", revision: 2, executor: "agent:sdc-docs-site", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: protocol-site-generator

## Outcome

`sdc-protocol-site write` writes one JSON document per protocol and an index into
`website/data/protocols/` from `protocol/*.protocol.yaml`, the bindings, the node and decision
classes, `docs/check-output.txt`, `protocol/simulate/*.txt` and the release notes; `check` exits
1 when the committed data differs from what `write` would write.

## Context

The site renders the protocols from the YAML itself (the operator, 2026-11-21: "render them just
from the specs themselves, from the YAML itself"). `site-manifest` writes
`.well-known/b10x-site.json`, the declaration the beyond10x project-site workflow checks.

## Acceptance

`cargo test --bin sdc-protocol-site` passes, including a fixture whose change changes the
output and a stale check that fails; `scripts/check-bindings.sh` prints `pass site data`.

## Scope

`driver/src/bin/sdc-protocol-site.rs`, `driver/Cargo.toml`, `website/data/protocols/`,
`scripts/check-bindings.sh`.
