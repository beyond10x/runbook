---
format: aep.planning-md/3
id: story:spec-gate-interpreted-conformance
kind: story
status: proposed
title: The spec gate runs the interpreted conformance suite
summary: A spec its own interpreter cannot carry out is refused at the spec stage and goes back to spec.author (fix/spec-gate-interpreted-conformance)
tags:
- eval
- improvement-loop
relations:
- decomposes: epic:improvement-loop
scope:
- confidence: cited
  path: driver/examples/sdc/fixtures
- confidence: cited
  path: driver/examples/sdc/tools
- confidence: cited
  path: scripts
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T18:54:09Z", actor: "human:operator", revision: 5, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: spec-gate-interpreted-conformance

## Outcome

The spec stage refuses a specification that ESS's own interpreter cannot carry out. `spec.synthesize`
and `spec.gate`'s `ess-gate` (`sdc-tool.sh spec_conform`) run the synthesized suite with
`ess verify conform run --target interpreted --strict`. They pass only when every scenario passes. An
unsupported, failed or errored scenario refuses the specification, and the gate's output ends with
the scenario ids and ESS's reason. `spec.author` gets that list in `in/feedback.md` through the
existing route for a refused `spec_synthesis` (branch `fix/spec-gate-interpreted-conformance`).

## Context

`release-0.1.2.mixed.1` stopped `writer_spent` at `change.implement` of IDEA-1.sharedlist.list.
The story's `tests.run` runs the same interpreted suite. 12 scenarios passed and 8 were unsupported
("binding command has an unmet outcome obligation"), all in other stories' domains. The failure
went to the implementing writer, which cannot change the specification. Two rework sessions
changed nothing, and the run stopped. 0.1.0 would have done the same
(`experiments/release-0.1.2/notes.md`, branch `exp/improve-1-results`). Follow-up: the story's
`tests.run` still runs the interpreted suite, which now repeats a check the spec stage passed.

## Hypothesis and metric

- Hypothesis: a specification defect of this kind is repaired by `spec.author` at the spec stage
  and no longer stops a run at a story.
- Primary metric: runs stopped `writer_spent` at `change.implement` after a failed `tests.run`
  whose interpreted conformance had a scenario that did not pass, direction decrease, to 0.
- With it: `spec.author` reruns sent back by `spec_synthesis` (expected to rise), and the time of
  `spec.synthesize`, about 0.04 s more on a baseline-size spec.
- Guards: the clean-pass count; `run.delivered`.

## Acceptance

The cycle `spec-interp-unsupported` passes. The gate refuses a specification that validates,
compiles and synthesizes with no refusal but has an unsupported scenario, `spec.author` is sent
back with the scenario id and ESS's reason, and the clean specification passes. An experiment
record with this hypothesis and at least 3 runs per variant reaches a verdict by `sdc-experiment
report`.
