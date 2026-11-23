---
format: aep.planning-md/3
id: story:harden-skill-stop-names-the-run
kind: story
status: draft
title: The harden skill stops one run by its name
summary: 'Pre-existing finding of the adversary on unit C of wave 2026-11-22-w1 (pass 2, N4): one command line of plugin/skills/harden/SKILL.md lacks --run.'
tags:
- from-finding
relations:
- informed_by: review-result:adversary-unit-c-pass-2
revision: 1
---
## Outcome

The harden skill tells a reader to stop one run by its name, and says what the command does
without a run name.

## Finding

`review-result:adversary-unit-c-pass-2`, finding N4 (origin pre-existing, verdict CONFIRMED,
severity note): `plugin/skills/harden/SKILL.md:103` lists `sdc-experiment stop <id> --reason
"<why>"` without `--run` and without the sentence that the command then stops every open run and
abandons the record. `AGENTS.md:120` and `docs/workflow-metrics.md:432` give `--run` at the head
of `impl/line3-docs` (`8e8e143`). The line is on the base of line 3 (`4677c2b`).

## Acceptance

- `plugin/skills/harden/SKILL.md` gives the stop command with `--run <run name>` and says what
  it does without it, in the words of `AGENTS.md`.
- `git grep -n 'sdc-experiment stop' -- plugin docs website/docs AGENTS.md README.md`: every hit
  either carries `--run` or says that all open runs stop.

## Scope

- `plugin/skills/harden/SKILL.md`.

## Not in this story

The other findings of the pass: they went back to the unit's implementor in wave `2026-11-22-w1`.
