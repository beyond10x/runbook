---
format: aep.planning-md/3
id: story:router-picks-a-runbook-for-an-input
kind: story
status: active
title: With no runbook named, a router picks one for the input
summary: A rule first, a model call second, maps an input to one available runbook, prints the choice and its reason, and starts nothing when it cannot tell.
tags:
- backlog-2026-11-23
relations:
- informed_by: story:orchestrate-intake-triage
- depends_on: story:runbooks-load-from-configured-paths
scope:
- confidence: cited
  path: protocol
- confidence: cited
  path: runbook/src/client
- confidence: cited
  path: runbook/src/list.rs
- confidence: cited
  path: runbook/src/router.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T01:45:49Z", actor: "human:operator", revision: 2, executor: "agent:runbook", correlation: "operator-2026-11-23-run-defaults"}
- {from: "proposed", to: "active", at: "2026-11-23T01:45:50Z", actor: "human:operator", revision: 3, executor: "agent:runbook", correlation: "operator-2026-11-23-run-defaults"}
---
## Outcome

`runbook run --intake <file>` with no runbook named chooses one: a router reads the input and maps
it to one of the available runbooks, says which one and why, and starts it. When it cannot tell, it
asks the person and starts nothing.

## Source

- The operator, 2026-11-23, typed in the runbook session: "if we would not specify it, there would
  be sort of a default, and the default would just be where we basically take the input and we
  classify it, or we basically map it to available protocols. It's basically like a classifying
  router or something like that. Yeah, look at TACT. There's already some work in there for this."
- Read on 2026-11-23: the takt repository has no classifier. Its choice is a rule in code (a story
  of the store opens `story.delivery/1`, `--intent` opens `software.change/1`;
  `crates/takt-cli/src/main.rs:560-565`, story `cli-protocol-defaults`, implemented) and a registry
  of built-in protocols (`crates/takt/src/protocol.rs:33-49`, `takt protocols`). This repository
  has no router either; `docs/orchestration.md:103,140` describes `intake.route` and
  `story:orchestrate-intake-triage` (proposed) plans it for arrivals of an area.

## Acceptance

- Each runbook states in its protocol or beside it what kind of input it takes (a few lines and
  examples). `runbook list` shows that text.
- With no runbook named, the router picks among the runbooks of `story:runbooks-load-from-configured-paths`.
  A rule decides first where a rule can (a ticket id form, a flag such as `--bugfix`, one runbook
  only); a model call decides the rest, through the adapter and the caps of the run, counted in
  the spend ledger and written to the run's records as a decision of `router:<kind>`, never under
  a person's name.
- The choice is printed before the run starts: the runbook, the reason, the runner-up. Below a
  stated confidence, or with an input that fits none, nothing starts and the exit code says so;
  `--runbook <name>` or the positional name always wins and makes no call.
- The input text is data: an instruction inside it changes no choice of confinement, cap or
  autonomy level (a test with a hostile input holds this).
- Tests with a stub adapter and recorded answers written by hand; no model call in the test step.
  No test over 60 s.

## Scope

A new `runbook/src/router.rs`, `runbook/src/client/`, `runbook/src/list.rs`, `protocol/`,
`runbook/tests/`, `docs/`, `CHANGELOG.md`.

## Not in this story

- Routing of arrivals inside an area case (`story:orchestrate-intake-triage`).
