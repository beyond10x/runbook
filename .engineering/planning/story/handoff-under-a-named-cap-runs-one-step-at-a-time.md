---
format: aep.planning-md/3
id: story:handoff-under-a-named-cap-runs-one-step-at-a-time
kind: story
status: proposed
title: With a cap named, a handoff of a run with a model node takes one step per call
summary: The runner runs the printed tree run of a handoff as printed, so several steps spend from one reservation before a named cap is looked at again; with a cap named every such call takes one step.
relations:
- informed_by: story:caps-are-optional
scope:
- confidence: cited
  path: docs/loom-run.md
- confidence: cited
  path: runbook/src/loom_run/run_loop.rs
- confidence: cited
  path: runbook/tests/loom_run/answers.rs
- confidence: cited
  path: runbook/tests/loom_run/caps_optional_review.rs
- confidence: cited
  path: runbook/tests/loom_run/decisions.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T14:57:35Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
---
## Defect

The operator's rule for caps: a cap a user names stops the run when it is reached. The runner looks at a
named cap between two calls of Loom. At a handoff Loom prints a `tree run` command, and the runner runs it
as printed. That command carries no bound on its steps, so every step it takes spends from the one
reservation made before the call, and the cap is looked at again only when the call returns.

The unit `caps-are-optional` bounded the printed command to one step per call for a run opened without a
model node. For a run opened with a model node it did not: two committed cases
(`decisions::a_handoff_runs_the_printed_tree_run_with_the_confinement_loom_printed`,
`answers::a_finding_that_needs_a_person_is_answered_at_its_handoff_and_the_run_goes_on`) assert the printed
command unchanged, and such a run needs more than one step behind a handoff. The ignored measurement
`the_printed_tree_run_of_a_handoff_is_bounded_to_one_step_when_a_cap_is_named` in
`runbook/tests/loom_run/caps_optional_review.rs` states what is wanted and fails today.

## Outcome

1. With a spend cap or a call cap named, every `tree run` the runner makes for a handoff takes one step,
   for a run opened with a model node as for one opened without. The runner makes the next call itself and
   looks at the cap between two calls.
2. With no cap named the printed command runs as printed.
3. The level and the store of the printed command are the run's own, as the story
   `handoff-call-holds-the-runs-own-level-and-store` says.
4. The two committed cases keep what they hold about the level, the store and the answer at the handoff.
   Where one of them asserts the argv byte for byte, the change of that assertion is named in the unit's
   report with the sentence it held before and after.

## What a guard must hold

- A named cap that is reached at a handoff stops the run before the next step starts, with the outcome and
  the exit code a reached cap has elsewhere.
- A printed command that is no plain `tree run` is not run while a cap is named.
- A bound in the printed command is never widened: a printed bound of one step stays one step.

## Acceptance

The ignored measurement runs without `#[ignore]` and passes. A fixture run with a model node, a handoff and
a cap that the second step behind the handoff passes ends at the cap after that step and starts no third.
The same fixture with no cap named makes the calls it makes today. No test over 60 s.

## Origin

Finding S4 of the security pass on the unit `caps-are-optional`, 2026-11-23; fixed there for runs opened
without a model node.
