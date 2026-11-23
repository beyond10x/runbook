---
format: aep.planning-md/3
id: story:daemon-runs-take-autonomy-and-a-decider
kind: story
status: active
title: A run started through the daemon takes an autonomy level and a decider
summary: The daemon has no autonomy and no decider, so every decision of a run waits for answer_decision; a submit or the daemon's runbook.toml names both and the daemon answers through the decider code of runbook run.
relations:
- depends_on: story:runbook-toml-carries-defaults
- informed_by: story:daemon-default-confinement-is-stated
scope:
- confidence: cited
  path: docs/engine.md
- confidence: cited
  path: docs/mcp.md
- confidence: cited
  path: ess/domains/daemon.yaml
- confidence: cited
  path: runbook/src/engine
- confidence: cited
  path: runbook/src/loom_run/decider.rs
- confidence: cited
  path: runbook/src/mcp/schema.rs
- confidence: cited
  path: runbook/src/serve.rs
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T14:15:30Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
- {from: "proposed", to: "active", at: "2026-11-23T14:15:30Z", actor: "human:operator", revision: 3, executor: "agent:runbook"}
---
## What is missing

The operator, 2026-11-23, relayed by the operator's host session: a consumer's release review runs fully
automatic, with no approval step. A consumer starts that runbook through the daemon every day.

Read on the default branch at `914dffc2`:

- `runbook run` has the switch. With `autonomy = "A5"` and a decider of kind `rule` or `model` in the run
  settings the decider takes every decision and a rule takes the close of a case
  (`runbook/src/loom_run/decider.rs:22-25`, `docs/loom-run-settings.md:103`). At A1 to A4 the decider takes
  what the holder table gives it; with no level every decision is a person's.
- The daemon has neither. Its engine names no autonomy and no decider (`runbook/src/engine/`), and
  `docs/engine.md` lists "the spend ledger and the decider" under "Not built". Every wait of the protocol
  (a capability approval, a person step, the close of each case) becomes an open request until a client
  calls `answer_decision` (`docs/engine.md:33`, `docs/mcp.md:48-49`).

So a runbook started through the daemon cannot run without a client that answers each request.

## Outcome

1. A run started through the daemon takes an autonomy level and a decider: named by the submit, or by the
   `runbook.toml` the daemon reads (`story:runbook-toml-carries-defaults`); the submit wins.
2. The daemon answers through the same decider and routing code as `runbook run`: one implementation, the
   same holder table, the same record of each answer.
3. A decision the level leaves to a person is an open request as today, and `answer_decision` answers it.
4. With no level named, every decision waits, as today.

## What a guard must hold

- An answer of a rule or a model is recorded as `rule:<name>` or `decider:<kind>`, never under a person's
  name and never under the label a client typed. The daemon's own principal stays the one Loom sees for an
  answer a client gave.
- A level below A5 never widens what the holder table gives the decider; a missing or unreadable table
  fails closed: every decision waits.
- The level and the decider of a run are fixed at its start and written in its record; no later call
  changes them.
- A submit cannot name a decider program by path: the kinds are the ones the settings reader knows, and a
  model decider starts only what the daemon's operator configured.
- The daemon's floor for the confinement, its token check and its one guard layer are unchanged. A submit
  selects among the decider kinds the daemon's operator configured, in an option or the file, and never
  adds one: a model decider that the operator did not configure is refused.
- The start form of the daemon's page presets no level and no decider.

## Acceptance

With stand-ins and no model call: a submit with `A5` and the rule decider `approve` runs a fixture runbook
with a person step, a capability approval and a close to its end with no open request, and every answer in
the record names the rule; the same submit without a level stops at the first decision as today; a level
of A3 leaves the decisions of a person open and answers the others; a submit that names a model decider
against a daemon whose operator configured none is refused before anything is written. A security
review pass covers the change before it merges.

## Origin

The operator's word of 2026-11-23 on an automatic release review, and the consumer's milestone that
starts a daily round through the daemon.
