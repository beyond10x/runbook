---
format: aep.planning-md/3
id: story:requester-decider
kind: story
status: proposed
title: Requester agent as a decider configuration
summary: Requester-side classes go to decider:requester with goals in its inputs; delivering-side classes keep their decider.
tags:
- requester
relations:
- decomposes: epic:requester-side-and-goals-context
- depends_on: story:goals-root-input
- depends_on: story:decision-contract-goals-grounds
- depends_on: story:decision-request-from-question
scope:
- confidence: inferred
  path: driver/examples/sdc/tools/decider.sh
- confidence: inferred
  path: scripts/run-trial.sh
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:38Z", actor: "human:timo", revision: 3}
---
# Story: requester-decider

## Outcome

The requester-side decision classes (scope-changing answers, context, vision, roadmap, release,
close, budget overrun) go to a requester agent, principal `decider:requester`, whose inputs are the
request, its subject, the files its grounds cite, idea, dissection, goals and its earlier decisions
only; delivering-side classes keep their own decider. Without a ground it escalates under D→P and
takes the default marked `no_ground_in_goals` under D*.

## Acceptance

On the trial-2 fixture at A3 with `SDC_REQUESTER_DECIDER=haiku` and the recorded requester reply
fixture, the scope-changing questions are decided by `decider:requester`, whose record's `in/` holds
exactly the request, subject and cited files, `idea.yaml`, `dissection.yaml`, `goals.yaml` and
`decisions.json`, whose reply's grounds pass `tools/decision-grounds.sh`, and the same run's
code-review and merge requests are routed to the delivering-side decider.

## Scope

`driver/examples/sdc/tools/decider.sh` (a requester configuration: inputs, prompt, reply with
`no_ground_in_goals`), routing per class `side` in `scripts/run-trial.sh`. Takes the request built
by `story:decision-request-from-question`. Bound to `driver/` and `scripts/`: waits for the running
trial to end.

## Tooling note

- Changes the existing bash tool `driver/examples/sdc/tools/decider.sh`: allowed, a fix to prototype glue; no new bash tool.
- Changes the existing bash tool `scripts/run-trial.sh`: allowed, a fix to prototype glue; no new bash tool.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)
