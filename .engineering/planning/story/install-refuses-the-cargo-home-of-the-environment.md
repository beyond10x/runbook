---
format: aep.planning-md/3
id: story:install-refuses-the-cargo-home-of-the-environment
kind: story
status: draft
title: The refusal of the cargo directory as a prefix covers CARGO_HOME and an empty HOME
summary: The script reads HOME only, so a CARGO_HOME elsewhere is not refused and an empty HOME skips the check. From adversary pass 3 of the install unit, left open by fix round 3.
relations:
- informed_by: story:install-script-lays-out-a-prefix
revision: 1
---
## Outcome

The refusal of the cargo directory as an install prefix covers the directory that `CARGO_HOME` names, and it holds when `HOME` is empty or unset. The usage text says which directories the script refuses and which it does not.

## Source

Adversary pass 3 of `story:install-script-lays-out-a-prefix` measured that the script reads `HOME` only (`scripts/install.sh:210`): a `CARGO_HOME` elsewhere is not refused, and with `HOME` empty or unset the `.cargo` refusal is skipped. The script has no refusal for the user-level `.local` directory, `/usr` or a directory of `PATH`, and the usage claims none. Fix round 3 left it open.

## Scope

- `scripts/install.sh` (the refusal of prefixes and the usage text).
- `runbook/tests/install/refused_prefixes.rs`.

## Acceptance

- A prefix equal to the directory `CARGO_HOME` names is refused by name with nothing written, also with `HOME` empty and with `HOME` unset. A case in `runbook/tests/install/` shows each. The usage lists the directories that are refused.

## Origin

Introduced by the install unit. Held by the review record of its adversary pass 3 (finding N13).
