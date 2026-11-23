---
format: aep.planning-md/3
id: story:reply-coverage-work-follows-the-workspace-of-a-bound-program
kind: story
status: draft
title: sdc-reply coverage in a bound program reads its own workspace, not any --work
summary: The option --work is read whatever LOOM_CASE holds. A pre-existing note of adversary pass 1 of the Loom pin unit c9b4c177.
relations:
- informed_by: story:loom-pin-c9b4c177
revision: 1
---
## Outcome

In a bound program (`LOOM_CASE` set), `sdc-reply coverage` reads the workspace that the program was started in, and a `--work` that names another directory is refused or ignored by name.

## Source

Adversary pass 1 of `story:loom-pin-c9b4c177` measured that `--work <dir>` of `sdc-reply coverage` is read whatever `LOOM_CASE` holds (`driver/src/reply/coverage.rs`, `work_arg`). The reasoning of the unit's commit, that the workspace is not a directory a model can create, holds for the rule of `case_work` and not for this option.

## Scope

- `driver/src/reply/coverage.rs` (`work_arg`), `driver/src/reply/records.rs`.
- `driver/tests/`: one case for the bound and one for the hand-run form.

## Acceptance

- With `LOOM_CASE` set, `sdc-reply coverage --work <another directory>` reads the program's own workspace or exits non-zero naming the option, and a case in `driver/tests/` shows it. Without `LOOM_CASE`, `--work` reads the directory it names, as before.

## Origin

Pre-existing: `work_arg` is on the default branch. Held by the review record of the adversary pass 1 of `story:loom-pin-c9b4c177` (note N3); fix round 2 left it as it was.
