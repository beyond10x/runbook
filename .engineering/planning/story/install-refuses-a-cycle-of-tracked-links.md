---
format: aep.planning-md/3
id: story:install-refuses-a-cycle-of-tracked-links
kind: story
status: draft
title: The install script refuses a tracked link that resolves into a cycle, before any write
summary: A cycle of two tracked links passes the link check and is installed. From adversary pass 3 of the install unit, left open by fix round 3.
relations:
- informed_by: story:install-script-lays-out-a-prefix
revision: 1
---
## Outcome

A tracked link in a copied directory that resolves into a cycle of links is refused by name before anything is written to the prefix.

## Source

Adversary pass 3 of `story:install-script-lays-out-a-prefix` measured that a cycle of two tracked links passes the link check and is installed (`scripts/install.sh:361`). The check follows a link to the first place it points at and does not follow the chain. The checks of round 3 moved into one function unchanged (`check_link`); the pass-4 review did not re-run this one.

## Scope

- `scripts/install.sh` (`check_link` and the chain it follows).
- `runbook/tests/install/what_is_copied.rs`.

## Acceptance

- An install of a checkout that tracks a link cycle in a copied directory ends with exit 1, names the link, and writes nothing. A case in `runbook/tests/install/` shows it. A link that resolves to a file inside the copied directories still installs as before.

## Origin

Introduced by the install unit. Held by the review record of its adversary pass 3 (finding N12).
