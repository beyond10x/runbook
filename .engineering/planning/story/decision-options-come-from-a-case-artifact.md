---
format: aep.planning-md/3
id: story:decision-options-come-from-a-case-artifact
kind: story
status: draft
title: A person action's question, options, recommendation and grounds come from a case artifact
summary: The engine builds fixed approve and deny options; a person action reads the contract's request from its case so the daemon shows the case's options, recommendation and grounds.
tags:
- consumer-asks-2026-11-25
relations:
- depends_on: story:daemon-runs-take-a-gateway
- informed_by: story:decision-request-from-question
- informed_by: story:decision-contract-goals-grounds
scope:
- confidence: cited
  path: docs/daemon-api.md
- confidence: inferred
  path: docs/engine.md
- confidence: cited
  path: docs/mcp.md
- confidence: cited
  path: ess/domains/daemon.yaml
- confidence: cited
  path: ess/domains/decision.yaml
- confidence: inferred
  path: ess/generated
- confidence: cited
  path: runbook/src/commands.rs
- confidence: cited
  path: runbook/src/engine/decision.rs
- confidence: cited
  path: runbook/src/mcp/tools.rs
- confidence: cited
  path: runbook/src/runtime.rs
- confidence: inferred
  path: runbook/tests/daemon
revision: 6
---
## What is missing

A decision request that a run raises for a person is built by the engine from the structured members of the decision Loom names (`runbook/src/engine/decision.rs:40-61`): the question is the text of `decides` with the subject, revision and path appended, and the options are one of four fixed lists chosen by the decision's kind, `approve` and `deny` with a fixed effect, or the one `approve` of a handoff (`runbook/src/engine/decision.rs:62-76`). The options are JSON objects of the shape `{answer, effect}` (`runbook/src/runtime.rs:147-154`, `options: Vec<Value>`), and the request has no recommendation and no grounds. The specification types the same member as `List<sdc.decision.DecisionOption>` (`ess/domains/daemon.yaml:266`), whose members are `letter`, `does`, `cost` and `consequence` (`ess/domains/decision.yaml:97-105`), and the contract has the recommendation and its grounds (`ess/domains/decision.yaml:221-251`). So a person who answers through the daemon (view `open-decisions`, `docs/daemon-api.md:112`; the MCP tool `read_open_decisions`, `docs/mcp.md:48`) sees an approve or deny question and nothing of what the case knows about the choice, even where the case holds a request built in the contract's shape (`story:decision-request-from-question`, `story:decision-contract-goals-grounds`).

A consumer that puts decisions in front of a person needs the question, the options with what each does and costs, the recommendation and its grounds to come from the case's own artifact, and the answer to name one of those options.

## Outcome

1. A person action whose case holds a decision-request artifact (the contract's shape, `sdc.decision.DecisionRequest`) opens a daemon request whose `question`, `options` (each with `letter`, `does`, `cost`, `consequence`), recommendation and `grounds` are the artifact's, read by the engine and not written by it, and never written by a model.
2. The rows of the views `open-decisions` and `decisions` and the MCP tool `read_open_decisions` carry the recommendation and its grounds beside the options.
3. An answer names an option letter (or the free answer a question request takes, `story:decision-contract-goals-grounds`); the engine hands the runtime the effect of that option. An answer that names no option of the request is refused by name before anything is delivered.
4. A person action whose case holds no such artifact opens the request it opens today, unchanged byte for byte.

Which file of the case is the artifact, and how a binding names it, is settled in the story's first change from `docs/decisions.md`, `docs/loom-run-decisions.md` and `driver/examples/sdc/decision-classes.json`, and written into `docs/daemon-api.md` and `docs/engine.md`.

## What a guard must hold

- The request's content is read from the case artifact and checked: the grounds' citations resolve (`tools/decision-grounds.sh`), and the artifact's subject revision is the revision under decision (`ess/domains/decision.yaml:232`: the decision applies to that revision only). A request whose artifact fails the check is not opened with the artifact's words; the run stops by a named cause. The story tests that cause.
- The engine writes no option, no recommendation and no ground of its own. A model is not on this path.
- The answer's mapping from a letter to an effect is the artifact's, and the daemon's rules stand: the first answer stands, and `answered_by` stays a label (`docs/daemon-api.md:77-85`).
- A request that already opened keeps the options it opened with: a change of the artifact afterwards does not change an open request.

## Acceptance

With a fixture case that holds a decision-request artifact of three options, a recommendation and two grounds, and the stub runtime:

- the view `open-decisions` of the run has one row whose `question` equals the artifact's, whose `options` are the three with their letters, `does`, `cost` and `consequence`, and whose recommendation names the recommended letter and both grounds;
- an answer of the letter `B` is accepted and the runtime receives the effect of option B; an answer `approve` is refused by name and the request stays Open;
- the same case without the artifact gives the row it gave before this change (an existing test holds the bytes);
- `ess specify validate --path ess` passes and `task check:ess-generated` holds the OpenAPI document equal to the specification.

## Scope

`runbook/src/engine/decision.rs`, `runbook/src/runtime.rs`, `runbook/src/commands.rs` (`open_decisions_of_run`, `decisions_of_run`), `ess/domains/daemon.yaml`, `ess/generated`, `runbook/src/mcp/tools.rs`, `docs/daemon-api.md`, `docs/engine.md`, `docs/mcp.md`, `runbook/tests/daemon`.

## Depends on

- `story:daemon-runs-take-a-gateway`: the order is behind the gateway release; there is no code dependency.
- Informed by `story:decision-request-from-question` (builds the request this story reads) and `story:decision-contract-goals-grounds` (the free answer and the goals ground).
- Order: first of the six stories filed together, because it is the one a consumer's first person-facing step needs.
