---
format: aep.planning-md/3
id: story:model-step-is-the-lead-of-the-argv
kind: story
status: draft
title: A model step is named by the lead of its argv
summary: 'Pre-existing finding of the adversary on unit B of wave 2026-11-22-w1 (F6): model_step_argv matches sdc-node anywhere in the argv.'
tags:
- from-finding
relations:
- informed_by: review-result:adversary-unit-b-pass-1
revision: 1
---
## Outcome

A binding counts as a model step by its `argv` only when `sdc-node` is the program the `argv`
starts with. A program check that passes `sdc-node` as an argument stays a program check.

## Finding

`review-result:adversary-unit-b-pass-1`, finding F6 (origin pre-existing, verdict INFEASIBLE,
severity note): `driver/src/reply/rules.rs:16-20`, `model_step_argv`. The comment says the
`argv` starts with `sdc-node`; the code matches it as any member. The reviewer found nothing
that reaches it: all 14 agent bindings also carry `llm`, which makes the same decision. Two red
cases are in the reviewer's patch (`only_the_lead_of_the_argv_starts_a_model_step`,
`a_program_check_with_sdc_node_as_an_argument_is_still_a_program_check`).

## Acceptance

- `model_step_argv` reads the first member of the `argv` (its file name, with or without a
  directory in front); the two cases pass.
- A rule that needs a program check and meets a binding with an empty or missing `argv` and no
  `llm` refuses; a test holds it.

## Scope

- `driver/src/reply/rules.rs`, `driver/tests/`.

## Not in this story

The `llm` member of the JSON bindings: `story:one-copy-of-the-bindings`.
