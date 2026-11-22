---
format: aep.planning-md/3
id: story:intake-asks-what-stories-will-need
kind: story
status: proposed
title: Intake and spec surface what the implementing node will ask
tags:
- conductor-succession
- trial-2
relations:
- decomposes: epic:conductor-succession
- depends_on: story:interview-derived-from-goals
- depends_on: story:late-findings-per-stage-kpi
scope:
- confidence: inferred
  path: driver/examples/sdc/tools/llm-node.sh
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:36:15Z", actor: "human:operator", revision: 3, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: intake-asks-what-stories-will-need

## Outcome

Decisions the implementing node needs (session handling, storage, read paths) are asked at intake
or at the spec stage, with the goals context, instead of surfacing as story-stage findings.

## Context

Trial 2: the interview asked 4 questions (L1, `docs/trial-2.md:531`), and the run later raised 8
story-stage findings among 14 late ones (`:534`): f9 (session), f13 (storage), f16 (a read path).
Goals context: epic `requester-side-and-goals-context`.

## Acceptance

On a replay of trial 2's idea with the goals context, at most a third of trial 2's 8 story-stage
findings recur (at most 2), counted by `late-findings-per-stage-kpi`.

## Scope

The `questions.write` and `spec.author` prompts in `llm-node.sh` (existing bash, a fix).
