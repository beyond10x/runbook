---
format: aep.planning-md/3
id: story:install-accepts-a-checkout-path-with-a-quote-or-backslash
kind: story
status: draft
title: The install script installs from a checkout path with a quote or backslash, or names the character
summary: Such a path ends the script with the message about the target triple, which names the wrong cause. From adversary pass 3 of the install unit, left open by fix round 3.
relations:
- informed_by: story:install-script-lays-out-a-prefix
revision: 1
---
## Outcome

`scripts/install.sh` installs from a checkout whose path holds a double quote or a backslash, or it refuses that path by naming the character. It never ends with the message about the target triple for such a path.

## Source

Adversary pass 3 of `story:install-script-lays-out-a-prefix` measured that cargo escapes those characters in its JSON output, the script's search for the binary's path does not find the escaped form, and the script stops with the target-triple message (`scripts/install.sh:412`). The message names the wrong cause. Fix round 3 left it open.

## Scope

- `scripts/install.sh` (the search for the built binary).
- `runbook/tests/install/`: one case for each character.

## Acceptance

- For a checkout path with a double quote, and for one with a backslash, the install ends with exit 0 and an installed binary that runs, or with a refusal that names the character and writes nothing. A case for each shows the outcome chosen. A path without those characters installs as before.

## Origin

Introduced by the install unit. Held by the review record of its adversary pass 3 (finding N11).
