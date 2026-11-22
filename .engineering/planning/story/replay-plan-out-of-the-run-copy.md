---
format: aep.planning-md/3
id: story:replay-plan-out-of-the-run-copy
kind: story
status: draft
title: The plan and the live count of a recorded run are out of reach of the programs it starts
summary: The replay adapter reads its plan and its call log from the run directory on every call; under local confinement a bound program can rewrite both and start a live call past the cap.
tags:
- from-finding
relations:
- informed_by: review-result:security-line2-units-3-4-pass-1
revision: 2
---
## Outcome

The policy, the cap and the live program of a recorded run come from the runner and cannot be
changed by a program the run starts; the count of live calls is held where such a program cannot
reset it.

## Finding

`review-result:security-line2-units-3-4-pass-1`, finding B (origin pre-existing, verdict
NEEDS-CHANGE): `driver/src/bin/sdc-replay.rs:1063` (`ctx_at`), `:1076`, `:1225`. The replay
adapter reads `<run>/replay/plan.json` on every call (the policy, the cap, `live.bin`) and counts
live calls from `<run>/replay/calls.jsonl`. Under `--confinement local` a bound program can
write both: the reviewer's case rewrites the plan and truncates the log, after which one live
call starts where the plan with cap 0 had stopped the run (exit 75 before, exit 0 after). Under
`--confinement substrate` no `<run>/replay` is visible to a bound program. The read is the same
on `main` at `17c00ab57f`. Also named by `review-result:adversary-line2-fix-rounds-1-2-pass-2`
(finding 5).

## Acceptance

- A decision is written down first: the runner hands the policy, the cap and the live program
  to the adapter in the adapter's own environment or arguments, or it keeps the plan and the
  count in its private directory (the one under `$XDG_RUNTIME_DIR/runbook/<run>/`, mode 700),
  or the count is held by the runner process. The choice names how a resumed run finds its count.
- The reviewer's case is turned around and passes: a plan and a call log rewritten after the run
  began do not start a live call past the cap, and do not make another program the live adapter.
- The live program is one of a fixed list or the path the runner resolved at the start.
- Without a readable cap no live call starts (holds today; a test keeps it).
- `docs/loom-run.md` says where the plan and the count are kept.

## Scope

- `driver/src/bin/sdc-replay.rs`, `driver/src/bin/sdc-node/setup.rs`,
  `runbook/src/loom_run/replay.rs`, `runbook/src/loom_run/decider.rs` (`replay_plan`).
- `runbook/tests/loom_run/replay.rs`, `replay_boundaries.rs`, `docs/loom-run.md`.

## Not in this story

Counting a live call before it starts: fixed in wave `2026-11-22-w1` (finding C of the same
review).
