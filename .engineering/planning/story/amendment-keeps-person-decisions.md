---
format: aep.planning-md/3
id: story:amendment-keeps-person-decisions
kind: story
status: proposed
title: An agent amendment may not empty a person's decision
tags:
- conductor-succession
- trial-2
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: inferred
  path: driver/src/bin/sdc-findings.rs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:36:14Z", actor: "human:timo", revision: 3, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: amendment-keeps-person-decisions

## Outcome

A default taken by the agent never undoes what a person decided.

## Context

Trial 2: f11's default ("The token is issued and held nowhere; no command checks it yet") emptied
the person's f9 decision (a session cookie); the verification checks rules only, not the meaning of
a person's decision (`docs/trial-2.md:472-475`, `:493-494`, R7).

## Acceptance

On a fixture where an agent amendment's default contradicts or empties a decision with
`decided_by: human`, the check after the amendment refuses it, reverts it and sends the finding to
a person or the decider with both sides named, and an amendment consistent with that decision
stands.

## Scope

`sdc-findings` (existing Rust binary, `driver/src/bin/sdc-findings.rs`).
