---
format: aep.planning-md/3
id: story:reply-case-dir-branch-has-a-caller-or-goes
kind: story
status: draft
title: The case_work branch that reads the case directory has a bound caller or is removed
summary: No bound flow of the release reaches the branch; only a hand-run helper and tests do. A note of adversary pass 1 of the Loom pin unit c9b4c177.
relations:
- informed_by: story:loom-pin-c9b4c177
revision: 1
---
## Outcome

The workspace of a bound program has one source in `driver/src/reply/records.rs`. The branch of `case_work` that reads `<case_dir>/work` has a caller that a bound flow reaches, or it is removed with its cases.

## Source

Adversary pass 1 of `story:loom-pin-c9b4c177` measured that no bound flow of the current release reaches the `<case_dir>/work` branch: only a hand-run `sdc-reply` and the driver's own tests do (callers: `Facts::driver` in `runbook/src/loom_run/decider/validate.rs` and tests). A later unit may remove it; fix round 2 of the pin left it as it was.

## Scope

- `driver/src/reply/records.rs` (`case_work`, `Facts::open`), `runbook/src/loom_run/decider/validate.rs`.
- `driver/tests/`: the cases of the branch.

## Acceptance

- After the change, a search for callers of the branch finds either a case that a bound flow reaches or no branch. The driver's test step passes, and the cases of the workspace rule (`workspace_layout.rs`, `sdc_node.rs`) pass unchanged.

## Origin

Introduced by the pin unit `story:loom-pin-c9b4c177` (the branch was left in place when the workspace rule moved). Held by the review record of its adversary pass 1 (note N1).
