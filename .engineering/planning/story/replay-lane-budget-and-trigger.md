---
format: aep.planning-md/3
id: story:replay-lane-budget-and-trigger
kind: story
status: draft
title: The replay lane ends within 120 s and runs when its sources change
summary: 'Found in wave 2026-11-22-w1: the lane was red at the head unseen, and its setup runs 131 s.'
tags:
- backlog-2026-11-22-w1
relations:
- informed_by: review-result:adversary-line2-units-4-6-pass-1
revision: 1
---
## Outcome

The replay lane (`task test:replay`) ends within the 120 s budget of a test step, and it runs
whenever a source of its result changed, so that it cannot be red at a head without anyone
seeing it.

## Source

- `review-result:adversary-line2-units-4-6-pass-1` (wave `2026-11-22-w1`): the lane was red 5 of
  9 at the head of line 2, because the committed recording was packed at ESS 0.97.3 and the line
  pinned 0.97.4. No gate had run the lane after the pin moved.
- Fix round 2 of that wave (`fix/line2-review-2`, `c413209`): 9 of 9 green; the lane's setup
  (`replay-runs`) ran 131.2 s at a load average of 44 to 60, over the 120 s budget (141 s at the
  first measurement).
- The implementor's list of what makes the lane's result stale: `driver/src/bin/sdc-replay.rs`,
  `driver/src/bin/sdc-node.rs`, `driver/src/bin/sdc-node/**`, `driver/src/reply/**`,
  `driver/examples/sdc/fixtures/recorded/stub/**`, `driver/examples/sdc/fixtures/idea.yaml`,
  `driver/ess-pin.json`, `bindings/**`, `instructions/**`, `protocol/**`, `rust-toolchain.toml`,
  `runbook/src/loom_run/{replay.rs,bindings_copy*,decider*,run_loop*}`,
  `runbook/tests/loom_run/replay.rs`, `runbook/tests/replay-runs.sh`,
  `runbook/.config/nextest.toml`.

## Acceptance

- The lane's setup builds its recorded runs once per digest of those sources (a cache under
  `target/`, keyed by the digest) and the step ends within 120 s with a warm cache; the cold
  setup has its own named time and runs in the `slow` lane.
- A check that costs under 5 s compares the digest of those sources with the one the last green
  lane run recorded, and fails with the command to run when they differ. The gate runs that
  check; CI runs the lane when the check would fail.
- The mask of the ESS version label in `normalise` (`driver/src/bin/sdc-replay.rs`) does not
  carry a typed version: a test fails when `driver/ess-pin.json` moves and the recording was not
  packed again, or the recording no longer depends on the label.
- No test of the lane runs over 60 s.

## Scope

- `Taskfile.yml`, `runbook/tests/replay-runs.sh`, `runbook/tests/loom_run/replay.rs`,
  `runbook/.config/nextest.toml`.
- `driver/src/bin/sdc-replay.rs` (`normalise`), `driver/examples/sdc/fixtures/recorded/stub/`.
- `AGENTS.md` (the list of lanes), `.github/workflows/`.

## Not in this story

A recording of a real run: recorded model output stays out of git.
