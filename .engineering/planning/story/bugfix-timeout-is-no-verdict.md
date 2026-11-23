---
format: aep.planning-md/3
id: story:bugfix-timeout-is-no-verdict
kind: story
status: active
title: A check of the bug-fix runbook that did not end by itself records no verdict
summary: A step ended by a time bound, the pool's limit or a signal writes no verdict and ends the case as an error naming the step, never as not_reproduced or not_fixed.
relations:
- decomposes: epic:ticket-to-merge-request-on-a-real-repository
- informed_by: story:bugfix-check-names-its-pool-time
scope:
- confidence: cited
  path: driver/src/bin/sdc-bugfix.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T13:04:44Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
- {from: "proposed", to: "active", at: "2026-11-23T13:04:44Z", actor: "human:operator", revision: 3, executor: "agent:runbook"}
---
## Defect

From the second bug-fix run of release 0.8.0 on a real repository, 2026-11-23. The step `repro.run` was
ended by the runtime at its time bound (3000 s): the run log holds the action's finish with the refusal
"timed out". The evidence of the same action says `repro_run result: not_red`, and the case ended
`not_reproduced` ("the test passes on unchanged code"). The reproducing test had not run: its job was still
waiting in the build pool. In the run before, the same test was red on the same base.

## Outcome

1. A check that did not end by itself records no verdict. This holds for every way it can be ended: the
   bound of the step, the pool's limit on run time, a signal, a pool that was not reached.
2. `sdc-bugfix` writes the evidence of a verdict (`red`, `not_red`, `green`, `not_green`) only after every
   command the verdict depends on has ended with a result of its own. Nothing written before that point can
   be read as a verdict: no default value, no file left from an earlier command or an earlier try.
3. A step ended without a verdict ends the case as an error that names the step and the cause (`timed
   out`), never as `not_reproduced` or `not_fixed`. The outcome file and the exit code of `runbook run` say
   so; the page of the bug-fix runbook lists the outcome.
4. The time a command waited in the pool's queue and the time it ran are both in the step's record, so a
   reader sees which of the two used the bound.

## What a guard must hold

A verdict is a claim about the repository under test: it is recorded only from a command that ran to its
end. A run that cannot decide says that it cannot decide.

## Acceptance

With a stand-in check tool and a stand-in pool client: (a) a first command that ends and a second that is
ended by the step's bound leave no verdict in the evidence, and the case ends as the error naming
`repro.run` and `timed out`; (b) the same for `tests.run`; (c) a command ended by the pool's limit (exit
124) gives the same; (d) the record holds queue time and run time per command.

## Origin

A consumer's defect report of 2026-11-23, relayed by the operator's host session.
