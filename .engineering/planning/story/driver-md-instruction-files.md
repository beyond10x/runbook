---
format: aep.planning-md/3
id: story:driver-md-instruction-files
kind: story
status: draft
title: docs/driver.md says where the bindings name an instruction file
summary: 'Pre-existing finding of the adversary on unit C of wave 2026-11-22-w1: one false sentence in the section Instructions.'
tags:
- from-finding
relations:
- informed_by: review-result:adversary-unit-c-pass-1
revision: 1
---
## Outcome

`docs/driver.md`, section "Instructions", says truthfully where the bindings name an instruction
file.

## Finding

`review-result:adversary-unit-c-pass-1`, row 6 (origin pre-existing, verdict CONFIRMED, severity
note): `docs/driver.md:1108` says "`bindings/*.yaml` names no instruction file", while
`bindings/*.yaml` carries `--instructions` on 14 lines, at the head of `impl/line3-docs`
(`9fa5d1e`) and at its base (`4677c2b`).

## Acceptance

- The sentence at `docs/driver.md:1108` (or where it stands then) agrees with
  `grep -c -- '--instructions' bindings/*.yaml`: it names the option and what it points at.
- No other sentence of the section contradicts `bindings/*.yaml`.

## Scope

- `docs/driver.md` (section "Instructions").

## Not in this story

The other 24 findings of the pass: they went back to the unit's implementor in wave
`2026-11-22-w1`.
