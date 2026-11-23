---
format: aep.planning-md/3
id: story:declared-answer-reserves-before-its-program-runs
kind: story
status: draft
title: A spend cap with nothing left stops a run before the programs of a declared answer start
summary: The programs of an answers file run before any reservation, also after the cap is passed. Found beside the two guards of the spend-cap story.
relations:
- informed_by: story:spend-cap-with-nothing-left-starts-nothing
revision: 1
---
## Outcome

A spend cap that has nothing left stops a run before the programs of a declared answer start. The programs named by an answers file (`requests`, `apply`, `then`) are not run after the cap is passed.

## Source

The reading notes of the unit `story:spend-cap-with-nothing-left-starts-nothing` found that `answer_declared` runs the programs of an answers file, which are not Loom calls, before any reservation is made. After the cap is passed, the `requests` program still runs. The two guards of that story cover the calls to Loom (`tree run` and `tree approve`); this one covers programs the runner starts itself.

## Scope

- `runbook/src/loom_run/run_loop.rs` (`answer_declared`, the reservation), `runbook/src/loom_run/answers.rs`.
- `runbook/tests/loom_run/`: one case per program of an answers file.

## Acceptance

- With a named cap that has nothing left, a run that reaches a declared answer starts none of its programs and ends with the outcome of a reached cap. A case in `runbook/tests/loom_run/` shows it for `requests`, for `apply` and for `then`. With no cap named, or a cap with room, the answer runs as before.

## Origin

Pre-existing: the code is on the default branch. Found while reading code for `story:spend-cap-with-nothing-left-starts-nothing`; not part of its acceptance.
