---
format: aep.planning-md/3
id: epic:area-board
kind: epic
status: proposed
title: Area board in the UI
summary: 'The UI shows an area as a portfolio board: header state, intake and admission strips, one lane per child case, the orchestrator''s panel; today''s view becomes a lane''s drill-down.'
tags:
- orchestration
relations:
- decomposes: initiative:roadmap
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:34Z", actor: "human:operator", revision: 2}
---
# Epic: area-board

## Status note

Accepted by the operator, 2026-11-21: "I do accept the orchestration and parallelism design and upcoming epics and stories. So write them, scope them, implement it, but when it's time for it. So we're probably focusing first on the non-orchestration bits." Drafted 2026-11-21 under that acceptance; implementation waits until the non-orchestration work (AGENTS.md, "Work order") is done.

## Outcome

A reader of the live view sees, for one area, what arrived, what was admitted or held and why,
every running child as a lane, the orchestrator's ranking beside the program's verdict, and
whether the area is quiescent and what would wake it (`docs/orchestration.md` § 5). Today: one
store, one tree, one graph (`ui/README.md:26-52`).

## Why Now

Concurrent lanes make the single-tree view unreadable. Design accepted 2026-11-21 (quote above);
scheduled after the non-orchestration work.

## Scope

Header, lanes, drill-down, timeline; intake strip; admission strip; orchestrator panel and
`wave.approve` in the Decisions drawer; a store watcher for many concurrent writers (§ 5).

## Out of Scope

- The programme view listing areas (§ 5 mentions it; the programme level is *inferred*, § 2.6).
- Flow KPIs on the board: `story:area-board-flow-kpis` in `epic:flow-limits`.

## Risks

- `ui/web/index.html` and `ui/server.mjs` are shared surfaces; stories touching them serialise.

## Ambiguities

- None beyond the programme view, left out above.

## Done When

The board renders a recorded area run with at least two concurrent lanes, and a Playwright spec
covers each strip.
