---
format: aep.planning-md/3
id: story:contradiction-supersedes-only-what-it-replaces
kind: story
status: proposed
title: Contradiction supersedes only what it replaces
summary: A contradiction finding's decision supersedes only the cited answers it declares it replaces (runs/trial-3-mixed-3 f5/q5)
tags:
- conductor-succession
- late-findings
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: inferred
  path: driver/examples/sdc/tools/llm-node.sh
- confidence: inferred
  path: driver/src/bin/sdc-findings.rs
- confidence: inferred
  path: scripts/run-finding-cycles.sh
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T12:27:39Z", actor: "human:timo", revision: 3, executor: "agent:sdc-superseded", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: contradiction-supersedes-only-what-it-replaces

## Outcome

A contradiction finding's decision supersedes only the cited answers it replaces: an answer the
finding cites for another reason keeps `status: accepted`, its requirement keeps citing it, and its
"Decision qN" sentence stays.

## Context

D141 (`sdc-findings` `amend_one`, commit 460b486) supersedes every question a contradiction finding
cites whose answer differs from the decision. `runs/trial-3-mixed-3`: the build's contradiction f5
cites r3 and q5 and was decided as q8 "Offline step only opens the cached list; offline edits are
queued in the sync story". q5 answers a different question ("Cache the app and the list on each
online visit", when the list is cached) and still holds, but under D141 it would be superseded by
q8 and r3 would lose its cite and sentence.

The narrow rule first proposed (supersede only when the new answer and the old one are options of
the same question or the same field) also stops the case D141 was built for: in
`runs/trial-3-haiku-5` the build's f8 offered "store the list in the data file" and "implement the
Postgres wire protocol by hand", and q10's answer "Postgres database for the list" is neither one of
f8's options nor were f8's options among q10's (Postgres, hosted key-value store). No exact form in
today's findings tells the two cases apart.

Proposed: a finding declares what it replaces. The findings schema of `llm-node.sh` gets an
optional `replaces`, the cited question ids whose answer the finding's options replace (for f8:
`[q10]`; for f5: none). `check_finding` refuses an id in `replaces` that is not among the cites
or is not an answered question; `amend_one` supersedes only the ids in `replaces` (a trace gap
decided again keeps superseding its own earlier answer). Fixture `finding-superseded-story.yaml`
gets `replaces: [q3]`.

## Acceptance

- Cycle `superseded-answer` still passes (q3 superseded by q4, r1 cites q4 only).
- A new cycle or unit test on the mixed-3 shape: a contradiction citing r3 and q5 without
  `replaces` is decided; q5 stays `accepted`, r3 cites q5 and the new answer.
- A finding whose `replaces` names a question it does not cite is rejected at promote with the
  reason.

## Limits

- It moves the judgement to the agent that raises the finding: an agent that omits `replaces` on
  a real replacement leaves both answers accepted, as before D141; the citation rule does not catch
  that.
