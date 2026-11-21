---
format: aep.planning-md/3
id: story:stop-message-prints-request
kind: story
status: proposed
title: The trial stop message prints the decision request
relations:
- decomposes: epic:decisions-a-person-can-take-cold
- depends_on: story:decision-request-from-question
scope:
- confidence: inferred
  path: scripts/run-trial.sh
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:38Z", actor: "human:timo", revision: 3}
---
# Story: stop-message-prints-request

## Outcome

The command line shows a person the whole request when a trial stops for them, not only the command
that approves it.

## Acceptance

`scripts/run-trial.sh` on a run that stops for scope answers (`SDC_DECIDER=person`, exit 3) prints,
before the `then run again` line, one rendered request per open scope-changing question: the context
line, the question, each option with its consequence and cost, the recommendation with its grounds,
and what happens next; a dry run's output shows it for the stub idea.

## Scope

The `waiting()` function (`scripts/run-trial.sh:156`) and the person path of
`scripts/run-trial.sh`, calling `sdc-decision-request`. Bound to `scripts/`: waits for the
trial in progress to end.

## Tooling note

- New tool: the Rust binary `sdc-decision-request` (`driver/src/bin/sdc-decision-request.rs`), in place of `tools/decision-request.sh` named before.
- Changes the existing bash tool `scripts/run-trial.sh`: allowed, a fix to prototype glue; no new bash tool.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)
