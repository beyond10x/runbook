---
format: aep.planning-md/3
id: story:install-reads-head-again-after-the-copy
kind: story
status: draft
title: The install script reads HEAD and the tracked state again after the last copy
summary: A change of HEAD or of a tracked file while the copy runs must not give a prefix that names the first commit. Residual of the second HEAD read, from security pass 4 of the install unit.
relations:
- informed_by: story:install-script-lays-out-a-prefix
revision: 1
---
## Outcome

The prefix that `scripts/install.sh` leaves names the commit whose bytes it holds. HEAD and the state of the tracked files are read again after the last copy and before the marker of an unfinished install is removed; a difference ends the install.

## Source

Security pass 4 of `story:install-script-lays-out-a-prefix` measured that HEAD and the tracked state are read once, before the copy starts. A commit or an edit made while the copy runs gives exit 0, a `VERSION` that names the first commit without ` dirty`, and the later bytes in the prefix. The comment at `scripts/install.sh:266` says the files are of one commit; that holds only after a read at the end.

## Scope

- `scripts/install.sh` (the reads at the start of the copy, the end of the copy, the removal of the marker).
- `runbook/tests/install/`: one case, and the mutant without the second read.

## Acceptance

- A change of HEAD or of a tracked file made while the copy runs ends the install with exit 1, names both states, and leaves the marker of an unfinished install. The prefix never states a commit other than the one whose bytes it holds. A case in `runbook/tests/install/` shows it, and a mutant of the script without the second read fails that case.
- An install with no such change ends as before: exit 0, no marker, `VERSION` as before.

## Origin

Residual of the finding the unit fixed in round 3 (the window that covered the whole build is closed; the window of the copy is not). Introduced by the install unit. Held by the review record of its security pass 4.
