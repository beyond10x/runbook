---
format: aep.planning-md/3
id: story:stage-runs-alone
kind: story
status: proposed
title: Every stage of the cycle runs alone from recorded inputs
summary: Each stage protocol opens as a root in autopilot from a fixture input set, one run-trial.sh entry.
tags:
- conductor-succession
- eval
relations:
- decomposes: epic:conductor-succession
- depends_on: story:bugfix-cycle
scope:
- confidence: inferred
  path: docs/driver.md
- confidence: inferred
  path: driver/examples/sdc/fixtures/stages
- confidence: inferred
  path: scripts/run-trial.sh
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T10:34:41Z", actor: "human:timo", revision: 3, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: stage-runs-alone

## Outcome

Every stage of the cycle runs alone from recorded inputs: each stage protocol (intake, direction,
epic, spec, plan, story, release) can be opened and driven by itself in autopilot from a fixture
input set (the artifacts its parent would hand it), with one entry of `scripts/run-trial.sh`, so a
stage is trialled with a real model in minutes and a defect in release does not need a full run to
reach.

## Context

The operator, 2026-11-21: "Break it down, make it composable. That's the right way." Earlier the
same day: the trial cycles are too big. Trial 3 needed five full attempts, each stopping at a later
stage (`docs/trial-3.md` will carry them; until then the runs `runs/trial-3-*`).

What exists (scoping of 2026-11-21 for `story:bugfix-cycle`):

- `sdc-drive init --protocol <p> --id <id> --bindings <b>` opens any protocol as a root
  (`driver/src/main.rs:39-51`, `:753-830`); `--artifact name=revision` sets an initial revision. A
  root has no `link.json`, so `close` writes no parent (`driver/src/main.rs:911`).
- A child's `work/` is a link to the parent's (`driver/src/main.rs:767-771`): a stage's inputs are
  files in the shared `work/`, so a seeded `work/` stands in for the parent.
- `eval/run-adversaries.sh:97-103` already opens `sdc.story` as root `S-1` with only `work/spec`.
- `scripts/run-trial.sh` opens only `IDEA-1` on `protocol/sdc.protocol.yaml`
  (`scripts/run-trial.sh:105`, `:238-246`) and prints the idea (`:289`); the rest of its loop works
  on whatever case directory `sdc-drive run` names.
- Tools read the epic and story names from the case id (`driver/examples/sdc/tools/llm-node.sh:205-206`,
  `driver/examples/sdc/tools/sdc-tool.sh:205`, `:262`), so a root must take an id of the shape
  `IDEA.<epic>.<story>`.
- Revisions handed in by a parent (`child.revisions`, for example `epic: "{name}"`) must be given
  with `--artifact` when the stage opens as a root.

## Acceptance

Per stage, a stub cycle closes by itself, and one real run in autopilot closes by itself from the
recorded inputs of a real run (`runs/trial-3-haiku-5` has all of them). The full cycle is unchanged
and composes the same protocols.

## Scope

- `scripts/run-trial.sh`: `--stage <protocol> --from <case dir of a recorded run>` (or a fixture
  directory): opens the stage as root with the parent's case id shape, seeds `work/` with the
  artifacts the stage's bindings read, and passes the parent's revisions with `--artifact`.
- `driver/examples/sdc/fixtures/stages/<stage>/`: one input set per stage.
- `docs/driver.md`: the stage entry.
