---
format: aep.planning-md/3
id: story:daemon-runs-take-a-gateway
kind: story
status: proposed
title: A daemon whose operator named a gateway gives it to the model programs of its runs
summary: The gateway table of runbook run does not reach runs started through the daemon, sdc-experiment run or sdc-free-agent; the daemon's operator states it and a submit cannot.
relations:
- depends_on: story:daemon-runs-take-autonomy-and-a-decider
- informed_by: story:model-programs-reach-a-gateway
scope:
- confidence: cited
  path: driver/src/bin/sdc-experiment.rs
- confidence: cited
  path: driver/src/bin/sdc-free-agent.rs
- confidence: cited
  path: runbook/src/engine
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T14:15:34Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
---
## What is missing

`story:model-programs-reach-a-gateway` gives `runbook run` a `[gateway]` table in the run settings. Its
unit does not cover a run started through the daemon: the daemon's engine takes flat string settings and
starts no model program yet. It also does not cover the settings writer of `sdc-experiment run` and the
launcher `sdc-free-agent`.

## Outcome

1. A daemon whose operator named a gateway (in the `runbook.toml` the daemon reads, or an option that
   names a token file) gives it to the model programs of its runs in the same way as `runbook run`: a
   file in the run's private directory, the address and the token set for the model process only, no
   login linked or copied.
2. A submit cannot name a gateway address or a token: the daemon's operator states them.
3. `sdc-experiment run` writes the table into the settings file it makes when the record names a gateway
   by token file, and `sdc-free-agent` takes the same file or refuses the table by name.

## What a guard must hold

The guards of `story:model-programs-reach-a-gateway`, unchanged: the token is in no argv, log line, event,
record, run directory or committed file; it lies only in a private directory of mode 700 that is removed
with everything in it; there is no default address and no default token.

## Acceptance

With a stand-in for the model program and a stand-in gateway on loopback: a run started through the daemon
reaches the stand-in gateway with the configured token and holds no login; a submit that names a gateway is
refused by name; the token is in no file of the run directory and in no event of the daemon. A security
review pass covers the change before it merges.

## Depends on

`story:daemon-runs-take-autonomy-and-a-decider` (the daemon starts a model program only with it).
