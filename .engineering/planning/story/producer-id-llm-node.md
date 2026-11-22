---
format: aep.planning-md/3
id: story:producer-id-llm-node
kind: story
status: draft
title: The producer id llm-node no longer names a deleted program
summary: 'Pre-existing finding of the adversary on unit B of wave 2026-11-22-w1 (F7): six producer ids carry the name of the deleted model-node script.'
tags:
- from-finding
relations:
- informed_by: review-result:adversary-unit-b-pass-1
revision: 1
---
## Outcome

No producer id of the bindings carries the name of a deleted program: the id `llm-node` is
either renamed after the program that produces the record, or a document says that it is a
label and what it stands for.

## Finding

`review-result:adversary-unit-b-pass-1`, finding F7 (origin pre-existing, verdict CONFIRMED,
severity note): six lines `producer: llm-node` in `bindings/*.yaml` and their JSON twins name
`llm-node.sh`, which wave `2026-11-22-w1` deletes. The acceptance grep of
`story:driver-runtime-removed` searches `llm-node\.sh` and does not see the id. The id is the
same at `985e8ee`.

## Acceptance

- A decision is written down first: rename (to which id) or keep as a label. The independence
  rules of the protocols compare producer ids (`llm-node` against `llm-review`); a rename keeps
  every pair that must differ different, and `runbook canon check` of the nine protocols prints
  the same results.
- After a rename: `driver/tests/producer_ids.rs` holds the new id; `sdc-protocol-site check`
  says current; the three cycles `stub-cycle`, `scope-answer`, `review-reject-once` end as
  expected; recorded runs under `ui/test/recorded/` keep their old ids and still display.

## Scope

- `bindings/*.yaml`, `driver/examples/sdc/*.bindings.json`, `website/data/protocols/`.
- `driver/tests/producer_ids.rs`, `docs/driver.md`.

## Not in this story

The ids `runbook` and `runbook.<x>`: done in wave `2026-11-22-w1`.
