---
format: aep.planning-md/3
id: story:docs-llm-not-the-runtime
kind: story
status: proposed
title: 'The LLM is not the runtime: one run step by step'
summary: The principle as Canon states it, one run timeline in lanes, and the comparison with a free-running agent
tags:
- docs
relations:
- decomposes: epic:public-docs-site
scope:
- confidence: cited
  path: website/src/components/RunTimeline.tsx
- confidence: cited
  path: website/src/routes/LlmIsNotTheRuntime.tsx
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T15:09:50Z", actor: "human:operator", revision: 2, executor: "agent:sdc-docs-site", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: docs-llm-not-the-runtime

## Outcome

The page "The LLM is not the runtime" quotes the principle as Canon states it, shows how the SDC
applies it (`docs/driver.md:3-6`), and draws one delivered run from its `sdc.run-timeline/1`
document: lanes for program, model, decider, rule and idle; each model span with model, API
requests, tool calls, tokens and cost; a totals strip; a step list. A free-running agent is drawn
under it on a shared time axis and cost scale with the medians and the recorded hypothesis and
verdict, or "Runs pending" when its timelines are absent.

## Context

The timelines are written by `sdc-run-timeline` (`experiments/<id>/timelines/`).

## Acceptance

`tests/fixture.spec.ts` passes on the fixture build: a model span selected with the keyboard
shows its API requests and tokens, and the comparison draws two timelines.

## Scope

`website/src/routes/LlmIsNotTheRuntime.tsx`, `website/src/components/RunTimeline.tsx`.
