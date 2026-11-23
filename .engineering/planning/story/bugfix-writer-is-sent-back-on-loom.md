---
format: aep.planning-md/3
id: story:bugfix-writer-is-sent-back-on-loom
kind: story
status: draft
title: A writer of a bug-fix run on Loom is sent back before the case ends
summary: A reproduction that is not red and a test run that failed send the writer back within its budget; the case ends not_reproduced or not_fixed only when the writer is spent.
relations:
- decomposes: epic:ticket-to-merge-request-on-a-real-repository
revision: 1
---
## What is known

`docs/cycles-suite.md` (the bug-fix entries) says it for the Loom runtime: the first `repro_run` that is
`not_red` makes `not_reproduced` a legitimate outcome and Loom ends the case before the test writer is sent
back; the first `test_run` that is `failed` ends the case `not_fixed` before `change.fix` is sent back; the
protocol has no claim that says a writer is spent. On the old driver a writer had its budget of tries. A
consumer's first real run (2026-11-23) has one try per writer for that reason.

## Outcome

On the Loom runtime the test writer and the fix writer are sent back with the result of the run that
failed, up to the budget the bindings name. The case ends `not_reproduced` or `not_fixed` only when that
budget is spent, and the outcome says how many tries were made.

Open: whether the protocol gains a claim for a spent writer, or the bindings state it with what Loom has
today (the rework allowance of the pinned Loom). The story that takes this reads the pinned Loom first and
names a Loom story if Loom lacks a part.

## Acceptance

The suite entries of the bug-fix runbook that are `pending` for this reason are `expected`: a reproduction
that is red on the second try ends `fixed`; a writer that never gets red ends `not_reproduced` after its
budget, with the count.

## Origin

Pre-existing since the bug-fix runbook moved to Loom; named again by a consumer on 2026-11-23.
