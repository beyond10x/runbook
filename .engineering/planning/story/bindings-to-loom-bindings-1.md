---
format: aep.planning-md/3
id: story:bindings-to-loom-bindings-1
kind: story
status: proposed
title: Convert the SDC bindings to loom.bindings/1
tags:
- blocked-upstream
- loom
relations:
- decomposes: epic:kernel-and-runtime-uptake
scope:
- confidence: inferred
  path: driver/examples/sdc
- confidence: inferred
  path: driver/src/main.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:35Z", actor: "human:timo", revision: 3}
---
# Story: bindings-to-loom-bindings-1

## Outcome

The SDC's action-to-tool bindings are Loom bindings, so the protocols can run on Loom without a
second bindings format.

## Context

Loom `story:bindings-document`, in its last adversary pass on Loom branch
`impl/bindings-document` (guide `website/docs/guides/bindings.md` there). Today:
`driver/examples/sdc/<protocol>.bindings.json`, read by `sdc-drive` and checked by
`scripts/check-bindings.sh`.

Checklist, as reported by Loom:

- [ ] `actions` is a list; each entry has an `action` member.
- [ ] `executor` is an object `{kind: program|agent|person|open|copy, …}`.
- [ ] A `copy` executor's members are `evidence` and `from`; a `from` starting with `/` is a JSON
      pointer into the report.
- [ ] A `null` result in a result table is allowed.
- [ ] `timeout_seconds` absent stays absent.
- [ ] `protocol` is a registry registration: lowercase letters, digits, hyphens, `@<major>`
      (`sdc.direction/1` is written `sdc-direction@1`).
- [ ] No YAML anchors, aliases, merge keys or tags; nesting at most 64 deep.
- [ ] A result-table key is kept as written; `0` and `"0"` together are a duplicate.
- [ ] A `retry_when` kind is an evidence kind the protocol declares.
- [ ] `executor.bindings` of an `open` executor is held to the workspace like other paths.
- [ ] A `program`'s evidence may not read `from: effect`; a `person`'s or an `open`'s reads
      `effect` only.

## Acceptance

Every SDC bindings file validates as `loom.bindings/1`, and `scripts/check-bindings.sh` still
exits 0.

## Scope

`driver/examples/sdc/` (the eight bindings files), `driver/src/main.rs` (the reader).

## Ambiguities

- Whether `sdc-drive` reads `loom.bindings/1` directly or the old format is kept beside it until
  `story:loom-concurrent-children`: requires-stakeholder-input (the operator); default: the driver reads
  the new format.
