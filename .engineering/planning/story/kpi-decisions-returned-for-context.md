---
format: aep.planning-md/3
id: story:kpi-decisions-returned-for-context
kind: story
status: proposed
title: 'KPI: decisions returned for more context'
relations:
- decomposes: epic:decisions-a-person-can-take-cold
- depends_on: story:decision-request-from-question
scope:
- confidence: inferred
  path: docs/kpis.md
- confidence: inferred
  path: driver/src/bin/sdc-decision-context-request.rs
- confidence: inferred
  path: driver/src/bin/sdc-finding-kpis.rs
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:36Z", actor: "human:operator", revision: 3}
---
# Story: kpi-decisions-returned-for-context

## Outcome

Whether the requests are good enough is measured, not argued: the number of decisions a person
returned with a request for more context, per run, target 0.

## Acceptance

`sdc-finding-kpis <run-dir>` prints H5, the count of `decisions.jsonl` lines with
`decision: needs_context` for the run, which is 0 on a recorded run with none and 1 after
`sdc-decision-context-request <run-dir> <request id> "<what is missing>"` has appended one;
`docs/kpis.md` § 1d lists H5 (direction down, target 0, advisory) with that source.

## Scope

`docs/kpis.md` § 1d (the story adds the H5 row; the doc is not edited by the planning step),
`driver/src/bin/sdc-finding-kpis.rs` (beside H1 to H4), and a new
`sdc-decision-context-request` that appends the line. The record shape
(`decision: needs_context`, the request id, a note) is inferred.

## Tooling note

- New tool: the Rust binary `sdc-decision-context-request` (`driver/src/bin/sdc-decision-context-request.rs`), in place of `tools/decision-context-request.sh` named before.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)

