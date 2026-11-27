---
format: aep.planning-md/3
id: story:a-run-whose-node-used-its-last-call-and-wrote-its-result-is-delivered
kind: story
status: draft
title: A run whose node used its last call and wrote its result is delivered
summary: The daemon holds the call cap before it reads the run's end, so a node that spends every call left ends as a cap stop.
relations:
- informed_by: story:the-free-runs-turns-follow-the-runs-call-cap
revision: 1
---
## Outcome

A run whose agent node spends the last call the run has left and writes its result ends `delivered`, not
`stopped: llm calls`.

## Source

- The adversary pass on the free run's turns (branch `impl/free-run-turns-follow-call-cap`), 2026-11-27: with a
  call cap of 30, a node that uses all 30 turns and writes its result ends the run as a cap stop.
- On `origin/main` `6fd1d7cf`: the daemon's step loop holds the caps at the top of each round
  (`runbook/src/engine/mod.rs:553`, `gate.admit()`) before the next `run_tree` call
  (`runbook/src/engine/mod.rs:561`) evaluates the evidence the step recorded; `Gate::admit`
  (`runbook/src/engine/mod.rs:598-609`) answers `Cap::Calls` once the calls used reach `max_llm_calls`, so the
  run ends at `gate.reached` (`runbook/src/engine/mod.rs:554`) before the root case's terminal outcome is read
  (`runbook/src/engine/mod.rs:577-581`).

## Acceptance

- A daemon case with a call cap of N and a stand-in node that makes N calls and writes its result ends
  `delivered`; red on the base with `stopped: llm calls`.
- A case whose node makes N calls and writes no result still ends as a cap stop.
- A case whose node needs a call over N is still refused that call.
