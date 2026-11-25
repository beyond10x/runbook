---
format: aep.planning-md/3
id: story:person-action-switch
kind: story
status: draft
title: A run switches a protocol's person-action steps on or off, and the run setting wins
revision: 1
---
## Outcome

A person-action step can be switched off for a run. A protocol's write step carries `person_action: required` or
`none`; a run setting overrides it, per action (`person_actions.<protocol-id>.<action>`) or for the run
(`person_actions.default`), and the run setting wins. When the effective value is `none`, the runtime does not open
the person action: the step goes on as the protocol declares for that case, and the run records that the person action
was switched off and by which setting.

## Source

A consumer's ask, accepted by the operator on 2026-11-25 (relayed by the host operator).

## Guards that stay outside the switch

Steps guarded for writes to an external ticket system, to a cloud account, or to posts a customer reads keep their
person action whatever the switch says; the switch cannot turn those off. Where that set is declared is decided when
the story is scoped.

## Acceptance

- A protocol step marked `person_action: none` runs without opening a person action; one marked `required` opens it.
- `person_actions.default: none` switches off every switchable step; `person_actions.<protocol>.<action>: required`
  turns one back on; a per-action setting beats the default, and both beat the protocol's value.
- A guarded step opens its person action under any setting, and the run says the setting was not applied to it.
- The run's record names, per skipped step, the setting that skipped it.

## Depends on

The protocol language and the engine: a conditional person-action step in the protocol format (Canon) and in the
engine that runs it (Loom). Filed there as drafts with the same title.

## Open (for scoping)

- Today a step is a person action by its binding (`executor: {kind: person}` in the bindings file), not by the
  protocol. Whether the switch belongs in the protocol (as asked), in the bindings, or only in Runbook's run settings
  over the bindings decides whether the upstream changes are needed at all.
- What the step produces when its person action is skipped (a default result declared by the protocol, or no
  evidence).
