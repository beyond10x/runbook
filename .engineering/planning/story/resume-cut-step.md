---
format: aep.planning-md/3
id: story:resume-cut-step
kind: story
status: proposed
title: Detect a cut step and resume or re-run it
tags:
- conductor-succession
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: inferred
  path: driver/src/main.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:25:44Z", actor: "human:timo", revision: 4, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: resume-cut-step

## Outcome

A step whose driver died is noticed and handled on the next run, with the handling on the record.

## Context

`docs/driver.md` gap 15 (no resume; an `action_started` without `action_finished` is re-run,
*inferred*); observation records `observed.jsonl`, `driver.json`, `status.json` (`docs/driver.md`
"Observation records"); research: `docs/canon-research/01-event-stream-and-resume.md`.

## Acceptance

After a driver is killed during an LLM node, the next `sdc-drive run` records `step_resumed` (call
record `status.json` says `finished`) or `step_discarded` (it says `running` with a dead pid, or
`failed`) naming the action, and the run ends in the same state digest as an uncut run of the same
fixture.

## Scope

`driver/src/main.rs`. TEMPORARY copy of a runtime capability (`docs/driver.md:43-45`).

## Source to port

- takt replay and resume: `crates/takt/src/replay.rs` and the resume in `crates/takt/src/case.rs`.
  Move: port the idea. To assess: not read in detail (`docs/takt-comparison.md` § 5 answer 2, resume row). Paths are in the takt repository at `origin/main` 25c62df (tag 0.36.0), as cited there.
