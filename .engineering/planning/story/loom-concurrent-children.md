---
format: aep.planning-md/3
id: story:loom-concurrent-children
kind: story
status: proposed
title: Run area trees on Loom and retire the driver copies
tags:
- blocked-upstream
- loom
- orchestration
relations:
- decomposes: epic:concurrent-child-cases
- depends_on: story:concurrent-sibling-drive
- depends_on: story:standing-wakeup
scope:
- confidence: inferred
  path: driver/src/main.rs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:36Z", actor: "human:timo", revision: 3}
---
# Story: loom-concurrent-children

## Outcome

The area case and its children run on Loom, and the TEMPORARY runtime code in `sdc-drive` is
deleted (`docs/driver.md:43-45`; `docs/layers.md:51`).

## Context

Blocked upstream, Loom. Loom has no child cases today (pins Canon 0.8.0; `case_open` arrived in
0.9.0), and its planned `tree run` is sequential (`docs/orchestration.md` § 1.1). See the
`upstream-blocker` artifacts that block this story.

## Acceptance

A wave of two stories of a fixture area runs on Loom with both children in flight at once, and
`grep -c TEMPORARY driver/src/main.rs` prints 0.

## Scope

`driver/src/main.rs` (deletion). Capability lands in Loom, requested from the Loom session.
