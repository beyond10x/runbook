---
format: aep.planning-md/3
id: story:settings-reader-bound-and-older-refusals
kind: story
status: draft
title: The settings reader has a size bound and its older refusals print no value
summary: 'Findings of the two security passes of run-settings-are-toml in code older than that unit: the file is read whole, four older refusals name the file''s value, and a member name is printed without a bound.'
relations:
- informed_by: story:run-settings-are-toml
revision: 1
---
## Outcome

The reader of the settings file of `runbook run` reads at most a stated number of bytes, and no refusal of
any of its checks prints text of the file.

## Source

Security passes 1 and 2 of `story:run-settings-are-toml` (2026-11-23,
`review-result:security-run-settings-are-toml-pass-1`, `-pass-2`), findings in code older than that unit or
outside its acceptance:

- The settings file is read whole, whatever its size.
- Four checks of `normalise` that are older than the unit name the file's value of `format`, `run`,
  `confinement` and `decider.principal` in their refusal. The rule the unit holds for its own refusals is
  that no refusal prints a line, a value or a part of a value of the file, because a line of the file can
  hold the path of the login file.
- A refusal that names a member prints the member's name without a bound on its length.

## Scope

- `runbook/src/loom_run/settings.rs` (`read`, `normalise`), `runbook/src/loom_run/decider.rs`
  (`from_settings`).
- `runbook/tests/loom_run/settings_refusals_print_no_value.rs`: rows for the four checks, the size bound and
  a long member name.

## Acceptance

- A settings file over the stated bound is refused by name with the bound, exit 2, nothing written, before
  it is parsed.
- The four refusals name the member and the option it must equal, and not the file's value; the committed
  matrix of refusals covers them.
- A member name in a refusal is cut to a stated length.

## Not in this story

- Defaults from a `runbook.toml` (`story:runbook-toml-carries-defaults`).
