---
format: aep.planning-md/3
id: story:model-entry-for-bound-programs
kind: story
status: draft
title: A bound program calls a model through one documented entry that books its cost
summary: A program a runbook binds calls a model through the run's gateway by one documented entry; each call is recorded, counted against the call cap and booked against the spend cap.
tags:
- upstream-asks-2026-11-24
refs:
- provider: consumer
  reference: upstream-asks-filed/A1
relations:
- depends_on: story:model-programs-reach-a-gateway
- informed_by: story:caps-are-optional
- informed_by: story:spend-cap-with-nothing-left-starts-nothing
- informed_by: story:daemon-runs-take-a-gateway
scope:
- confidence: cited
  path: bindings/tools
- confidence: cited
  path: docs/loom-run-settings.md
- confidence: cited
  path: docs/loom-run.md
- confidence: inferred
  path: driver/src
- confidence: inferred
  path: runbook/src/loom_run
- confidence: inferred
  path: runbook/tests/loom_run
revision: 3
---
## Outcome

A program that a runbook binds can call a model through one documented entry. The entry reaches the model
through the gateway the run's settings name, writes the call in the form the runner already reads costs
from, and counts the call against `--max-llm-calls` and the spend cap. A runbook author no longer wraps a
model command in a program of their own.

## Source

Ask A1 of a consumer, filed in its story `upstream-asks-filed` (read 2026-11-24). Today the consumer wraps
`claude -p` itself and writes the `.sdc/llm` records by hand, because a program Loom starts gets `PATH` and
`LOOM_*` and nothing else. A call the program makes on its own is not in the ledger and not in the call
count (`story:run-without-model-node-needs-no-caps`, the coordinator's reading in its open question: a call
that does not pass the runner's adapter is neither recorded nor counted).

## What exists, and what it leaves open

- `story:model-programs-reach-a-gateway` (active; its unit is in review on the branch
  `impl/model-programs-reach-a-gateway`, not merged) gives the model nodes, the decider, the reply adapter and
  the conformance observation an address and a token from a `[gateway]` table. It does not give an arbitrary
  bound program a way to call a model, and it books nothing for a program's own calls.
- `story:daemon-runs-take-a-gateway` extends that to runs the daemon starts.
- The cost reader of the runner reads `work/.sdc/llm/NNN-<action>/` and `work/.sdc/llm/calls.jsonl`
  (`docs/loom-run.md`, "The spend ledger"); only `sdc-node` writes them.

## What this story adds

1. A documented entry a bound program calls with a prompt (file or standard input) and a few options (model,
   effort, a per-call budget). It prints the reply and exits with a code the page defines.
2. The entry takes the gateway of the run as `model-programs-reach-a-gateway` defines it, and the login path
   when no gateway is named, the same way model nodes do. It adds no default address and no default token.
3. Each call is one directory and one line in `work/.sdc/llm/`, with the members the cost reader reads
   (`cost_usd`, `record`, `action`, `adapter`, `exit`).
4. Before it starts a model process the entry asks the run for the call: a reached `--max-llm-calls` or a spend
   cap with nothing left ends the call before any process starts, with the exit of a reached cap.
5. Loom's program contract does not change: nothing is passed through Loom's environment. How the entry finds
   the run is the unit's decision, inside that rule (the way `--gateway-file` reaches `sdc-node` is one).

## What a guard must hold

- The token is in no argv, no log line, no event, no record, no file of the run directory (the guard of
  `story:model-programs-reach-a-gateway`, unchanged).
- A program that does not use the entry and calls a model itself is not booked. The page names that as a
  limit of the cap, and says that only the confinement level decides what such a program can reach.
- No path of the host and no value of the settings file is in the entry's output.

## Acceptance

With a stand-in gateway on loopback and a stand-in for the model process (no model call, no network beyond
loopback):

- A bound program of a stand-in runbook calls the entry; the reply is on its output and the stand-in gateway
  saw the configured token once.
- The call is one directory and one line under `work/.sdc/llm/`; the run's outcome states one call more than
  before, and the ledger holds its cost.
- With `--max-llm-calls 1`, the second call of that program starts no model process and the run ends as a
  reached call cap.
- With a spend cap with nothing left, the first call starts no model process.
- No file of the run directory and no event holds the token.
- `docs/loom-run.md` documents the entry, its exit codes and the limit above. No test runs over 60 s.

## Scope

- `bindings/tools/`, `driver/src/`, `runbook/src/loom_run/` (the cost reader, the budget and the private
  directory), `runbook/tests/loom_run/`, `docs/loom-run.md`, `docs/loom-run-settings.md`, `CHANGELOG.md`.

## Not in this story

- A confined run that reaches a gateway (`story:gateway-under-substrate-confinement`).
- A program that calls a model without the entry.
