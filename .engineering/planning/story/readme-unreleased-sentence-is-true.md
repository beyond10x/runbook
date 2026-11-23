---
format: aep.planning-md/3
id: story:readme-unreleased-sentence-is-true
kind: story
status: draft
title: No sentence of README.md says nothing is unreleased while the changelog lists unreleased entries
summary: The sentence in the list of releases is false at the unit's head and on the default branch. From security pass 4 of the install unit.
relations:
- informed_by: story:install-script-lays-out-a-prefix
revision: 1
---
## Outcome

No sentence of `README.md` says that nothing is unreleased while `CHANGELOG.md` holds entries under its Unreleased heading.

## Source

Security pass 4 of `story:install-script-lays-out-a-prefix` measured that the sentence "(nothing is unreleased)" in the list of releases of `README.md` is false: `CHANGELOG.md` has entries under Unreleased. The sentence was written for the release it follows and is not true after the next change; it stands the same on the default branch.

## Scope

- `README.md` (the paragraphs on each release).
- A case of the docs check that reads `README.md` and `CHANGELOG.md`.

## Acceptance

- `README.md` has no sentence about what is unreleased that disagrees with `CHANGELOG.md`: either the sentences are removed, or a check compares them to the Unreleased section and fails on a difference. The check is run by the gate, and a case shows it failing on a README that says "nothing is unreleased" beside a non-empty Unreleased section.

## Origin

Pre-existing: the sentence is the same on the default branch at the commit the unit's last merge took. Not the unit's sentence. Held by the review record of the security pass 4 of `story:install-script-lays-out-a-prefix`.
