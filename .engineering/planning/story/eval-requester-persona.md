---
format: aep.planning-md/3
id: story:eval-requester-persona
kind: story
status: proposed
title: Eval requester persona with hidden truth
summary: Each eval item carries goals for the run and a hidden truth the requester's answers are graded against.
tags:
- requester
relations:
- decomposes: epic:requester-side-and-goals-context
- depends_on: story:interview-derived-from-goals
scope:
- confidence: inferred
  path: driver/src/bin/sdc-requester-agreement.rs
- confidence: inferred
  path: eval/personas
- confidence: inferred
  path: eval/run.sh
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:35Z", actor: "human:operator", revision: 3}
---
# Story: eval-requester-persona

## Outcome

Each eval item carries a requester persona: `goals.yaml` given to the run and a hidden `truth.yaml`
(per topic: the idea span it is about, accepted answer patterns, the entry it is derivable from)
kept outside the sandbox like the oracle, and the eval grades the requester's answers against it.

## Acceptance

`eval/run.sh` on a persona item passes `goals.yaml` into the run while no record directory of the
run contains `truth.yaml`, and the eval report lists per question `agree`, `disagree` or `unknown`
with covered and uncovered topics apart; a planted persona whose truth contradicts its own goal
entry shows `disagree` on that question.

## Scope

`eval/run.sh`, new `eval/personas/` and a deterministic grader `sdc-requester-agreement`.
Design: `docs/requester.md` § 6; oracle placement `docs/eval-design.md:174-177`.

## Tooling note

- New tool: the Rust binary `sdc-requester-agreement` (`driver/src/bin/sdc-requester-agreement.rs`), in place of `eval/requester-agreement.sh` named before.
- Changes the existing bash tool `eval/run.sh`: allowed, a fix to prototype glue; no new bash tool.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)
