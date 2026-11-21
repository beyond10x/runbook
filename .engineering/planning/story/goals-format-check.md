---
format: aep.planning-md/3
id: story:goals-format-check
kind: story
status: implemented
title: Goals document format and check
summary: sdc.goals/1 entries and layers, checked and resolved by a program.
tags:
- requester
relations:
- decomposes: epic:requester-side-and-goals-context
scope:
- confidence: inferred
  path: goals
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:36Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-21T02:36:21Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"test_result":1}}, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
- {from: "active", to: "implemented", at: "2026-11-21T02:36:21Z", actor: "human:timo", revision: 10, decided_on: {"recorded":{"test_result":1}}, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: goals-format-check

## Outcome

The requester's goals are data a program checks: `sdc.goals/1` entries in company, product and
request layers, each entry with a stable digest a decision can cite.

## Context

Format rules: `docs/requester.md` § 2. A layer file carries a file-level `layer` key (required).
An entry has a `sources` list, `stated`, `derived_from`, and a priority 1 to 5 on every kind except
`constraint`. Entry digest: sha256 over the entry's canonical JSON. An `overrides` that names no
entry of a higher layer is refused as `override_target_missing`.

## Acceptance

`sdc-goals check` exits 0 on the trial-2 example layers and exits 1 naming the entry and the rule
on each bad fixture under `goals/tests/fixtures/` (missing file-level `layer`, duplicate id,
unknown kind, empty `sources`, priority outside 1 to 5 on a non-constraint, a lower layer
overriding a constraint, `override_target_missing`), and `sdc-goals digest` prints the same
per-entry digests on two runs over the same layers.

`sdc-goals resolve --at <instant> <file>...` prints, as JSON, the entries that apply across the
given layers at that instant: an entry outside its `valid_from`/`valid_until` window is left out,
an overridden target is left out and the overriding entry is in, and each entry carries its digest;
two runs with the same instant and files print the same bytes.

## Scope

The Rust crate `goals/` (binary `sdc-goals`, subcommands `check`, `digest` and `resolve`) and its fixtures in
`goals/tests/fixtures/`. No bash, jq or yq tool (AGENTS.md, "Rules").
