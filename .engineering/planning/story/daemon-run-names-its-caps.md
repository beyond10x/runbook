---
format: aep.planning-md/3
id: story:daemon-run-names-its-caps
kind: story
status: proposed
title: A run the daemon starts can name a wall time, a call cap, a spend cap and a ledger
summary: 'submit_run takes optional bounds for a run on the Loom runtime: wall time first, then call cap, spend cap and ledger. None named means no bound; a named one stops the run.'
relations:
- depends_on: story:caps-are-optional
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T10:20:12Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
---
## Outcome

A run the daemon starts on the Loom runtime can name its bounds: a wall time first, then a call cap, a spend
cap and a spend ledger. Each is optional; one that is named stops the run when reached.

## Source

The consumer's third gap after its first run through the daemon (2026-11-23): the daemon's Loom runtime has
no spend ledger, no wall cap and no model-call cap, and its runbook gets model steps next. The wall cap per
run is the one it asked for first. The rule for money caps is `story:caps-are-optional`: none named means
no limit, a named one stops.

## Scope

- `ess/`: the daemon's specification: the start of a run takes the optional bounds.
- `runbook/src/engine/calls.rs` (`RunSettings`), `runbook/src/mcp/schema.rs` (`submit_run`),
  `runbook/src/serve.rs`: the bounds reach the Loom run.
- `runbook/tests/`, `docs/engine.md`, `docs/mcp.md`, `CHANGELOG.md`.

## Acceptance

- `submit_run` with `max_wall_seconds: N` ends a run that is still running after N seconds, with the outcome
  of a reached wall time; the journal states it. The case uses the daemon's test clock and runs under 5 s.
- `submit_run` with `max_llm_calls`, `cap_micro_usd` and a ledger behaves as `runbook run` with the same
  options: a named cap stops the run when reached.
- A submit that names none of them starts and runs without a bound, as today.
- The daemon's start options can state a bound for every run it starts; a submit may name a lower one and
  never a higher one.

## Not in this story

- A decider in the daemon's Loom runtime, and resume of an orphaned run.
