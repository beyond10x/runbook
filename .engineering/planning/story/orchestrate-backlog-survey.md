---
format: aep.planning-md/3
id: story:orchestrate-backlog-survey
kind: story
status: proposed
title: Survey the backlog and end a bounded case done
tags:
- orchestration
relations:
- decomposes: epic:orchestrate-area-case
- depends_on: story:orchestrate-protocol-adopt
scope:
- confidence: inferred
  path: cases/12-orchestrate-bounded-done
- confidence: inferred
  path: driver/src/bin/sdc-backlog-survey.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:36Z", actor: "human:operator", revision: 3}
---
# Story: orchestrate-backlog-survey

## Outcome

A bounded area case ends `done` when every arrival was routed and nothing accepted is
undelivered; "no bugs left" is not the condition (`docs/orchestration.md` § 3).

## Context

`backlog_state` about `portfolio`: `drained` / `pending` (§ 2.2). Outcome `done` needs
`intake.triaged` and `backlog.drained` at the current revisions (§ 3 table).

## Acceptance

In `cases/12-orchestrate-bounded-done`, the variant with every entry triaged and no active
undelivered story ends `done`, and the variant with one active undelivered story records
`backlog_state` `pending` and has no legitimate outcome.

## Scope

A new program `sdc-backlog-survey`, one fixture case.

## Tooling note

- New tool: the Rust binary `sdc-backlog-survey` (`driver/src/bin/sdc-backlog-survey.rs`), in place of `driver/examples/sdc/tools/orchestrate-backlog-survey.sh` named before.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)
