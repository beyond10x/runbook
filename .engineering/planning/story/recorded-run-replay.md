---
format: aep.planning-md/3
id: story:recorded-run-replay
kind: story
status: proposed
title: Re-drive a recorded run under changed conditions
summary: A recorded run driven again with its recorded model replies, changed conditions and injected faults, divergence handled by policy
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: docs/driver.md
- confidence: cited
  path: driver/src/bin/sdc-replay.rs
- confidence: cited
  path: scripts/run-trial.sh
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T11:10:39Z", actor: "human:operator", revision: 3, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: recorded-run-replay

## Outcome

The operator, 2026-11-21: "If the LLM nodes are successful for a certain eval, we could also just
record it and then replay it, very quickly and fast, and simulate: given the LLM would respond in
that way at that place, change conditions, timeouts, retries, stuff like that, and see how the
system behaves and whether it would produce better metrics or not."

A recorded run can be driven again in seconds to minutes with no paid model call: the real driver
and programs run, every model and decider call gets the reply the source recorded at that place,
and changed conditions and injected faults show how the system behaves.

## Context

`scripts/run-trial.sh <run> --autopilot --recorded <source>` with `driver/src/bin/sdc-replay.rs` as
the adapter of every work node and of the decider (`docs/driver.md`, "Recorded replay"). Each call
is keyed by case, node or decision, arrival and try, and its inputs are checked against the
recorded call by normalised digests. A divergence is handled by a policy (`stop`, `nearest`,
`live`), never hidden. Conditions (`--set`) and faults (`--inject`) are run settings.

## Acceptance

- Re-driving a recorded run unchanged on the tree that recorded it reproduces its end and each
  case's sequence of actions, with zero divergence and no paid call (`replay-unchanged` on the
  committed recording `driver/examples/sdc/fixtures/recorded/stub`; `trial-3-strong-2` as a local
  check).
- `reject@merge.approve#1` runs the rework route, and the divergence policy decides the rest
  (`replay-reject-stop`, `replay-reject-nearest`); a timeout injected at one writer runs its retry
  path (`replay-timeout`); `live` with the `dry` adapter is counted and capped (`replay-live-dry`).
- Every call record says `recorded`, `reused` or `live`; the run's status carries the counts,
  `prompt_changed` and both tree commits; the model's time is on a virtual clock, in the fields
  real runs use, marked `clock: virtual`.

## Scope

`driver/src/bin/sdc-replay.rs`, `driver/src/main.rs` (`bindings_of`, virtual timeout),
`scripts/run-trial.sh` (`--recorded`), `driver/examples/sdc/tools/llm-node.sh` (divergence exit),
`driver/examples/sdc/fixtures/recorded/`, `scripts/run-finding-cycles.sh`, `docs/driver.md`.
