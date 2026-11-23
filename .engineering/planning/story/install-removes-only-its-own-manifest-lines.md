---
format: aep.planning-md/3
id: story:install-removes-only-its-own-manifest-lines
kind: story
status: draft
title: An install removes only files of the forms the script writes, named by an old MANIFEST
summary: Old MANIFEST lines for bin/, libexec/ or share/ names the script never wrote are removed. Only bin/runbook, libexec/runbook/ and share/runbook/ may be.
relations:
- informed_by: story:install-script-lays-out-a-prefix
revision: 1
---
## Outcome

An install over an older install removes only files of the forms the script itself writes: `bin/runbook`, files below `libexec/runbook/`, and files below `share/runbook/`. A line of an old `MANIFEST` in any other form removes nothing and is named.

## Source

Security pass 4 of `story:install-script-lays-out-a-prefix` measured that old `MANIFEST` lines in the right shape but for names the script never writes (another tool under `bin/`, a directory under `libexec/` that is not its own, a note under `share/runbook/`) are removed. The pass-3 fix refuses misspelled lines; it does not limit the names.

## Scope

- `scripts/install.sh` (the removal of files named by the old `MANIFEST`, near line 607).
- `runbook/tests/install/half_install.rs`.

## Acceptance

- With an old `MANIFEST` whose lines name files outside the three forms, the install removes none of them, names each, and finishes. Lines in the three forms still remove the files the script wrote. A case in `runbook/tests/install/` shows both, and a mutant that allows any prefix fails it.

## Origin

Introduced by the install unit; the same behaviour was measured at the head before fix round 3. The `MANIFEST` is a file of the prefix, so the case needs a prefix whose file was edited by hand. Held by the review record of its security pass 4.
