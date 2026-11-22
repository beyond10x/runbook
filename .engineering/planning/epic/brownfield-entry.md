---
format: aep.planning-md/3
id: epic:brownfield-entry
kind: epic
status: proposed
title: Brownfield entry
summary: 'Apply the SDC to an existing repository: reconstructed intake with citations, a ratchet ESS gate, partial plan activation, imported approvals, a roadmap.'
tags:
- orchestration
relations:
- decomposes: initiative:roadmap
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:34Z", actor: "human:operator", revision: 2}
---
# Epic: brownfield-entry

## Status note

Accepted by the operator, 2026-11-21: "I do accept the orchestration and parallelism design and upcoming epics and stories. So write them, scope them, implement it, but when it's time for it. So we're probably focusing first on the non-orchestration bits." Drafted 2026-11-21 under that acceptance; implementation waits until the non-orchestration work (AGENTS.md, "Work order") is done.

## Outcome

An existing repository enters the SDC at the stage its facts make true, instead of being
interviewed from scratch (`docs/orchestration.md` § 4). Today rated 2 of 5 (§ 4.4). Done shows as
the `decider` dry run (§ 4.3) reaching an admitted story without a person rewriting the spec
or the store first.

## Why Now

The decider dry run found six gaps (§ 4.4). Design accepted 2026-11-21 (quote above); scheduled
after the non-orchestration work.

## Scope

- Reconstructed requirements and context, each item citing `path:line` (`aep plan reverse scan`),
  interview only for what cannot be cited (gap 6).
- Decision 3 (§ 7): ratchet ESS gate: refusal count may not rise above the count at adoption, and
  each story's own domain is clean (gap 2).
- `sdc.plan` admits active stories without every story active (gap 3).
- Import of existing approvals and delivery evidence as copied records (gap 5).
- A roadmap for a store that has none (gap 1); AEP 0.80.0 has no roadmap kind, the roadmap is an
  `initiative` (`docs/protocol-notes.md:192-194`).

## Out of Scope

- Gap 4, a conformance target other than the implementation: no decided design (§ 4.4 item 4).
- Writing into `decider` or any adopted repository: the experiment stays read-only.

## Risks

- A reconstructed requirement that cites a line may still misread it; citations make it
  checkable, not right.

## Ambiguities

- How a story's "own domain" is determined (scope path under `spec/domains/`, a tag, or the
  spec's component map): requires-stakeholder-input (the operator).
- Whether the ESS `requires` pin (`--strict-requires` fails on decider, § 4.3) is part of the
  ratchet or must be fixed first: requires-stakeholder-input (the operator).

## Done When

Every story is implemented and a rerun of the § 4.3 dry run starts at `sdc.spec` with the ratchet
gate passing for `story:decider-extract`'s domain.
