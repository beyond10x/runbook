---
format: aep.planning-md/3
id: story:orchestrate-wave-integrate
kind: story
status: proposed
title: Dispatch a wave and record one result per wave
tags:
- orchestration
relations:
- decomposes: epic:orchestrate-area-case
- depends_on: story:orchestrate-wave-admit
- depends_on: story:orchestrate-wave-approve
- depends_on: story:orchestrate-intake-triage
- depends_on: story:story-merge-step
- depends_on: story:epic-stories-to-portfolio
scope:
- confidence: inferred
  path: cases/11-orchestrate-wave-integrate
- confidence: inferred
  path: driver/src/bin/sdc-wave-integrate.rs
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T01:30:36Z", actor: "human:operator", revision: 4}
---
# Story: orchestrate-wave-integrate

## Outcome

An approved wave opens one `sdc.story` case per story, and `wave.integrate` merges them and
writes one `wave_result` per wave whatever its size; a failed story goes back into `intake`
(`docs/orchestration.md` § 2.2, § 2.5).

## Context

The parent reads one aggregate per wave so N does not enter the state space (§ 2.6). Merge
conflicts and red gates leave the branch unmerged and are filed into `intake` (§ 2.5). The
copied `stories_delivered` record for `sdc.epic` comes from here.

## Acceptance

In `cases/11-orchestrate-wave-integrate`, a wave of two stories where one fails its gate yields
one `wave_result` `failed`, one merged branch, one new `intake` entry naming the failed story,
and a `stories_delivered` record for the merged one.

## Scope

A new program `sdc-wave-integrate` (calls the merge step), one fixture case.

## Canon children form

Canon accepted request 14 as Canon `story:child-outcome-subject` (Canon wave 8, after
`story:child-outcome-revision`, wave 7): `{children: {in: [passed], subject: wave, revision: current}}`
reads only the `case_outcome` records about `wave` at its current revision
(`docs/orchestration.md` § 2.6). Not needed here: the draft protocol works without it through
`wave_result`, and the form does not replace `wave_result` where that record also states that the
integration gate passed. So no upstream blocker. The runner's duty the form relies on (an `open`
`case_outcome` per opened child, `subject` = the `case_open` subject) `sdc-drive` meets today
(`driver/src/main.rs:491`, `:503-507`).

## Tooling note

- New tool: the Rust binary `sdc-wave-integrate` (`driver/src/bin/sdc-wave-integrate.rs`), in place of `driver/examples/sdc/tools/orchestrate-wave-integrate.sh` named before.
(AGENTS.md, "Rules": anything new that runs is Rust; existing bash tools get fixes only.)
