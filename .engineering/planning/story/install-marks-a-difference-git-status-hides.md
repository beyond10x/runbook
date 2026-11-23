---
format: aep.planning-md/3
id: story:install-marks-a-difference-git-status-hides
kind: story
status: draft
title: The dirty mark of VERSION covers differences git status does not print, or says it does not
summary: The sentence that VERSION is marked when a tracked file differs from HEAD is false for flagged files, converted line endings and hidden submodule changes. From security pass 4 of the install unit.
relations:
- informed_by: story:install-script-lays-out-a-prefix
revision: 1
---
## Outcome

The statement that `VERSION` carries ` dirty` when a tracked file of the checkout differs from HEAD is true for every difference between the bytes installed and HEAD, or it says what it measures.

## Source

Security pass 4 of `story:install-script-lays-out-a-prefix` measured that the mark follows what `git status` prints. A tracked file that git has been told to treat as unchanged or to leave out of the work tree, a file whose line endings the checkout converts, and a submodule change that a git setting hides are all installed with the work-tree bytes and no mark. The sentence "any tracked file of the checkout differs from HEAD" stands in `scripts/install.sh:56`, `README.md:89`, `docs/releasing.md:22` and `CHANGELOG.md:12`.

## Scope

- `scripts/install.sh` (the mark, and the usage text).
- `README.md`, `docs/releasing.md`, `CHANGELOG.md`.
- `runbook/tests/install/version_file.rs`: one case per class named above.

## Acceptance

- Either the install marks or refuses for each class named above, or the four sentences say that the mark follows what `git status` reports. One of the two is chosen, and a case per class holds it.
- A case reads the four places and fails when one of them states more than the script does.

## Origin

Behaviour as at the round before the fix (measured at both heads); the sentence is from the round-3 change of the unit. Introduced by the install unit. Held by the review record of its security pass 4.
