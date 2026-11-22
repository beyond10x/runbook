---
format: aep.planning-md/3
id: story:unattended-run-records-no-human-actor
kind: story
status: proposed
title: An unattended run's planning writes record no human actor
summary: planning moves in a run's store carry the acting agent or decider principal, never human:<user>
tags:
- trial-3
relations:
- decomposes: epic:conductor-succession
- informed_by: upstream-blocker:aep-actor-option
scope:
- confidence: inferred
  path: driver/examples/sdc/tools/llm-node.sh
- confidence: inferred
  path: driver/examples/sdc/tools/sdc-tool.sh
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T06:32:49Z", actor: "human:operator", revision: 4, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: unattended-run-records-no-human-actor

## Outcome

A run's planning writes carry the acting principal (`agent:…` or `decider:…`), never a human.

## Context

Trial 3, run `runs/trial-3-haiku-3`, unattended: the draft-to-active moves in the run's own planning
store record `actor: "human:operator"` although no person acted
(`store/IDEA-1/work/aep/story/sync.md:16`, `store/IDEA-1/work/aep/epic/app.md:10`). They come from
`plan.decompose` (`driver/examples/sdc/tools/llm-node.sh:428`, `:431`).

The `aep` CLI (0.80.0) has no actor flag. `aep plan artifact move --help`: "The actor is who the
move is on behalf of (`AEP_ACTOR`, else `human:$USER`); the executor is what ran it", and
`--executor` records what ran the move. So SDC can fix this now by setting `AEP_ACTOR` (and
`--executor`) on every `aep` call a node makes. The explicit per-command option is filed upstream:
`upstream-blocker:aep-actor-option`.

## Acceptance

After a stub unattended run, no `transitions` entry in the run's `work/aep/**/*.md` has an actor
starting `human:`, and each names the node's principal.

## Scope

- `driver/examples/sdc/tools/llm-node.sh`
- `driver/examples/sdc/tools/sdc-tool.sh`

## Ambiguities

- Which principal a `plan.decompose` move is on behalf of: the writer agent, or the decider that
  approved the plan (with the agent as executor).
