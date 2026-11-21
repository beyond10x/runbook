---
format: aep.planning-md/3
id: story:ess-ratchet-gate
kind: story
status: proposed
title: Ratchet ESS gate for an adopted spec
tags:
- orchestration
relations:
- decomposes: epic:brownfield-entry
scope:
- confidence: inferred
  path: cases/15-spec-ratchet
- confidence: inferred
  path: driver/src/bin/sdc-ess-ratchet.rs
- confidence: inferred
  path: protocol/compiled/sdc-spec.ir.json
- confidence: inferred
  path: protocol/sdc-spec.protocol.yaml
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:35Z", actor: "human:timo", revision: 5}
---
# Story: ess-ratchet-gate

## Outcome

Brownfield stories can start while an adopted spec still has synthesis refusals, and the spec
only moves forward (decision 3, `docs/orchestration.md` § 7).

## Context

Decider: 45 refusals, gate needs 0 (§ 4.3). Nothing is needed from ESS: the structured refusal
list exists in ess 0.96.0 (ESS session, 2026-11-21). Mechanics: `docs/orchestration.md` § 4.5.

- `ess verify conform synthesize --path spec --suite-format 5 --out suite.json`.
- Count: `.coverage.counts.refused` (decider `{"authored":0,"generated":41,"outside":0,"refused":45}`);
  `.coverage.knowledge` must be `"complete_inventory"`; records in `.coverage.refused`, identical
  records keep their multiplicity.
- Per record: `.code`, `.subject` = `{kind, name}` (`entity`, `outcome`, `type`, `transition`),
  `.scenario`. Domain = qualified name without its last segment; for an outcome from
  `.subject.name.command`, for a transition from `.subject.name.entity`.
- A story's own domains: the declarations it touches, mapped to domains the same way.
- Cautions: `--suite-format 5` writes `ess-conformance/59` for decider (not `/58`); never mix the
  two files' digests. `--format json` with an ordinary suite prints refusals on neither stream, so
  no refusal lines there is not zero refusals.

## Acceptance

In `cases/15-spec-ratchet`, with a recorded adoption baseline of 45 refusals, a spec revision with
44 `.coverage.counts.refused` and 0 records in the story's domains passes the gate, and a revision
with 46, or with 1 record in the story's domain, or with `.coverage.knowledge` other than
`complete_inventory`, fails it naming the count, the domain or the knowledge value; and while
`ess specify validate --strict-requires` fails, the gate refuses to take a baseline, naming the
pin (decider: requires 0.74, installed 0.96).

## Scope

`sdc.spec` protocol (baseline evidence), its IR, a new program `sdc-ess-ratchet` (reading
`suite.json`), one fixture case.

## Ambiguities

- How a story's own domain is determined: inferable, `docs/orchestration.md` § 4.5 (domains of
  the declarations the story touches).
- Version pin: inferable, ruling of 2026-11-21 in `docs/orchestration.md` § 4.5. A failing pin is
  not ratcheted; strict validation at the installed ess is a precondition of adoption, and fixing
  the pin is the first adoption step, its own story action, before the baseline is recorded.

## Tooling note

- New tool: the Rust binary `sdc-ess-ratchet` (`driver/src/bin/sdc-ess-ratchet.rs`), in place of `driver/examples/sdc/tools/ess-ratchet.sh` named before.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)
