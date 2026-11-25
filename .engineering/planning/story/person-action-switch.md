---
format: aep.planning-md/3
id: story:person-action-switch
kind: story
status: draft
title: A run switches a protocol's person-action steps on or off, and the run setting wins
revision: 3
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

## Design (taken by the operator, 2026-11-25, reversible before release)

- Canon protocol/1 as it is (no Canon change): a protocol declares one case input per switchable step (`CaseInput`,
  values `[required, none]`, id `person_action.<action>`) and gates the person action with
  `precondition: {input: person_action.<action>, in: [required]}`, with the path taken without it. `runs_only_when`
  is not the gate (it only asserts, and the pinned Canon has no such key). `canon check` takes every declared value,
  so both paths are proven. The protocol's default is the first listed value.
- D814: a child case inherits its parent's inputs by id, so the root protocol declares every switch input of its
  children.
- D815: the daemon's own config names which switches a submit may set to `none` (empty by default); `runbook run`
  reads the same list from runbook.toml; `person_actions` is refused inside a run's own tree. Turning an approval
  off is the daemon operator's choice, never the client's.
- Runbook maps `person_actions.<protocol>.<action>` and `person_actions.default` onto the declared inputs when the
  case opens (keys matched against the declared protocol and action pairs), refuses an unknown key or an unlisted
  value before the run starts, and records the source of each value in an own event `runbook.person-actions`.
- Guarded steps (an external ticket system, a cloud account, a post a customer reads) declare no input.
- A skipped step records no evidence. `runbook run --loom` (the child binary) refuses the switch.
- Order: Loom change (`OpenRequest.inputs`, `tree open --input`, children inherit by input id), Loom tag, Runbook
  pin bump, then this story.
