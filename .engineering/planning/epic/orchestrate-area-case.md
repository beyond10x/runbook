---
format: aep.planning-md/3
id: epic:orchestrate-area-case
kind: epic
status: proposed
title: Area case sdc.orchestrate
summary: One standing or bounded Canon case per area (repository or group with one planning store) that triages intake, proposes, admits, approves, dispatches and integrates waves, and ends done or retired.
tags:
- orchestration
relations:
- decomposes: initiative:roadmap
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:35Z", actor: "human:operator", revision: 2}
---
# Epic: orchestrate-area-case

## Status note

Accepted by the operator, 2026-11-21: "I do accept the orchestration and parallelism design and upcoming epics and stories. So write them, scope them, implement it, but when it's time for it. So we're probably focusing first on the non-orchestration bits." Drafted 2026-11-21 under that acceptance; implementation waits until the non-orchestration work (AGENTS.md, "Work order") is done.

## Outcome

The orchestration the main `sdc` session does by hand today (receive work, route it, rank it,
check it can run at once, ask the owner only on the wave fork, dispatch, merge, decide it is
finished) is one `sdc.orchestrate` case per area, with programs deciding admission and
integration and an LLM only writing artifacts (`docs/orchestration.md` § 2.1-2.3). A reader
knows it happened when a bounded area case over a fixture store runs from intake to outcome
`done` under `sdc-drive`, and a standing one rests quiescent with no outcome.

## Why Now

Design accepted 2026-11-21 (quote above). Not now in the work order: it follows the
non-orchestration work.

## Scope

- Promote `protocol/draft/orchestrate.protocol.yaml` (49,152 states, 0 findings, § 6) into the family
  with bindings, probes and fixture cases.
- Actions: `intake.route`, `intake.check`, `ideas.dispatch`, `wave.propose`, `wave.admit`,
  `wave.approve`, `wave.dispatch`, `wave.integrate`, `backlog.survey`, `health.observe` (§ 2.2).
- Decision 1 (§ 7): story delivery admitted in the area case; `sdc.epic` stops opening stories.
- Decision 2 (§ 7): standing case notices change through `intake` revisions plus `health` with
  `max_age` expired by Canon.
- One `wave_result` summary record per wave (§ 2.6); bounded outcome `done`, standing
  quiescence, `retired` (§ 3); root outcome rename `operating` → `delivered` (§ 3).
- Typed home of the nouns (intake, portfolio, wave, release; their evidence kinds): the Canon
  protocol, `protocol/draft/orchestrate.protocol.yaml:33-131`, not an ESS document.

## Out of Scope

- Driving siblings concurrently, per-story worktrees, merge step: `epic:concurrent-child-cases`.
- Work-in-progress limits and flow KPIs: `epic:flow-limits`.
- A programme above areas (§ 2.6, *inferred*, not drafted).
- The `children` form scoped to a subject revision (Canon request, § 2.6): the aggregate
  `wave_result` is used instead.

## Risks

- State budget: headroom is 16,384 states; one more result kind (×4) does not fit (§ 6).
- `wave_result` and copied records cannot be told from forgeries by Canon (§ 2.6).

## Ambiguities

- Which autonomy levels hold `wave.approve`: requires-stakeholder-input (the operator);
  `docs/autonomy-levels.md:208-213` names the mechanism, not the level.

## Done When

Every story under this epic is implemented, `scripts/check-protocols.sh` covers
`sdc.orchestrate`, and the bounded and standing fixture cases evaluate as stated in their stories.
