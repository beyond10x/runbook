---
format: aep.planning-md/3
id: epic:kernel-and-runtime-uptake
kind: epic
status: proposed
title: Kernel and runtime uptake
summary: Take up released Canon and Loom capabilities and take back the SDC's workarounds for their absence.
relations:
- decomposes: initiative:roadmap
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:35Z", actor: "human:operator", revision: 2}
---
# Epic: kernel-and-runtime-uptake

## Outcome

The SDC takes up Canon and Loom capabilities as they are released, and takes back the workarounds
it made while they were missing: shapes dropped only to fit Canon's state bound, definitions copied
between protocols, and its own bindings format beside Loom's.

## Why Now

Canon wave 2 (slice-wise bound, protocol imports) is closed on its branch, and Loom's
`story:bindings-document` is in its last adversary pass (coordinator, 2026-11-21). Each story here
waits for the upstream release; nothing starts before it.

## Scope

- Restore what was dropped to fit the 65,536 bound and re-check (`story:canon-slice-bound-uptake`).
- Share definitions between the eight protocols through `--import` (`story:protocol-imports-uptake`).
- Convert the bindings to `loom.bindings/1` (`story:bindings-to-loom-bindings-1`).

## Out of Scope

- Running the SDC on Loom (`story:loom-concurrent-children`).
- Building or requesting the capabilities: they are Canon's and Loom's, already accepted there.

## Risks

- A restored shape may still exceed the bound in one slice, as `story-with-four-gate-kinds` does.

## Done When

The three stories are implemented, each against a released upstream version.
