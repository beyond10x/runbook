---
format: aep.planning-md/3
id: story:project-rename
kind: story
status: proposed
title: Rename the project (postponed; the operator decides)
tags:
- conductor-succession
- operator-decision-open
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: docs
- confidence: inferred
  path: driver
- confidence: inferred
  path: goals
- confidence: inferred
  path: protocol
- confidence: inferred
  path: watch
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:25:43Z", actor: "human:timo", revision: 4, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: project-rename

## Outcome

The project carries a name that fits what it covers: it replaces Conductor, takt and (as a catalog)
engineering-protocols, and will cover more than the software delivery cycle (research, incident
response, signals from production).

## Context

The operator, 2026-11-21: "For now, just note down somewhere the intent to renaming this later to a
better thing. Let's postpone it." The decision is open and the operator's. This story records the
intent; nothing starts before the operator picks a name.

Candidates discussed, with their reasons:

- **Docket**: the list of cases in hand; matches case, evidence, authority, decision, grounds.
  Recommended by the coordinating session.
- **Score**: the written music every player follows; fits Canon, Takt, Conductor.
- **Mill**: houses the looms; fits Loom, Spindle, Zwirn.
- **Assay**: proof by test.

A rename touches: the repository name; the binaries `sdc-drive`, `sdc-watch`, `sdc-goals`,
`sdc-findings`, `sdc-finding-kpis`; protocol ids (`sdc.*`); format names (`sdc.*/1`); the docs; the
Gates enrolment.

## Acceptance

One name is chosen by the operator, every occurrence is changed in one change, and the old protocol
ids and formats are either refused with a message naming the new one or read for one version.

## Scope

The whole repository; scope recorded as the top-level directories a rename touches.

## Ambiguities

- The name: requires-stakeholder-input (the operator), postponed on 2026-11-21.
- Refuse old ids or read them for one version: requires-stakeholder-input (the operator).

## Acceptance note

- Accepted as a note under `approval-record:operator-standing-acceptance-2026-11-21`: the intent to rename is accepted; the decision itself, the name, stays the operator's.
