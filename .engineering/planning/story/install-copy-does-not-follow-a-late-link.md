---
format: aep.planning-md/3
id: story:install-copy-does-not-follow-a-late-link
kind: story
status: draft
title: The install script does not copy through a link that replaces a file after its check
summary: A file that becomes a link between the check of its group and the copy must not bring bytes from outside the checkout into the prefix. From security pass 4 of the install unit.
relations:
- informed_by: story:install-script-lays-out-a-prefix
revision: 1
---
## Outcome

Every file that `scripts/install.sh` copies into the prefix is the regular file of the checkout that was checked before the copy. A link that takes its place between the check of a group of files and the copy of the group is not followed, and bytes from outside the checkout never reach the prefix.

## Source

Security pass 4 of `story:install-script-lays-out-a-prefix` measured that `mkdir` and `chmod` run between the look at a group of files and the `cp -f` of the group (`scripts/install.sh:705` to `:660`). A file that has become a link to a place outside the checkout in that gap is copied through the link: exit 0, no marker, the outside bytes installed.

## Scope

- `scripts/install.sh` (the copy of each group; `cp` options and the check after the copy).
- `runbook/tests/install/`: one case per way the check is placed.

## Acceptance

- When a file of a group has become a link before its copy, the install either copies the link itself and stops by name, or stops after the copy because what landed is not the regular file that was checked. Exit is not 0, the marker stays, and no file of the prefix holds bytes from outside the checkout. A case in `runbook/tests/install/` shows it, and a mutant that copies through links fails it.
- A checkout in which nothing changes installs as before.

## Origin

Residual of the finding the unit fixed in round 3; the same window exists in the version before the fix. Introduced by the install unit. Held by the review record of its security pass 4.
