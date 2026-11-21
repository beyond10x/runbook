---
format: aep.planning-md/3
id: story:per-case-lock
kind: story
status: proposed
title: Lock per case instead of per tree
tags:
- orchestration
relations:
- decomposes: epic:concurrent-child-cases
scope:
- confidence: inferred
  path: driver/src/main.rs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:36Z", actor: "human:timo", revision: 3}
---
# Story: per-case-lock

## Outcome

Siblings can be written at the same time; one case still has exactly one writer.

## Context

Today one `.sdc-drive.lock` covers the whole tree (`driver/src/main.rs:108-138`).

## Acceptance

Two `sdc-drive` processes on two sibling cases of one tree both make progress, and a second
`sdc-drive` on the same case is refused naming the holder's pid.

## Scope

`driver/src/main.rs`. TEMPORARY copy of a runtime capability (`docs/driver.md:43-45`).
