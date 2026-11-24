---
format: aep.planning-md/3
id: story:bugfix-checks-are-one-action-each
kind: story
status: active
title: Each check command of the bug-fix runbook is an action with its own time bound and evidence
summary: The suite and the reproducing test, before and after the fix, are four actions of the protocol, the reproducing test first; a bound ends one action and leaves the evidence of the others.
relations:
- decomposes: epic:ticket-to-merge-request-on-a-real-repository
- depends_on: story:bugfix-timeout-is-no-verdict
scope:
- confidence: cited
  path: bindings/sdc-bugfix.yaml
- confidence: cited
  path: driver/src/bin/sdc-bugfix.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T13:04:45Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
- {from: "proposed", to: "active", at: "2026-11-23T13:04:45Z", actor: "human:operator", revision: 3, executor: "agent:runbook"}
---
## Defect

From the same run as `story:bugfix-timeout-is-no-verdict`: `repro.run` is one bound program that makes two
full builds (the suite on the base, then the reproducing test alone), each with its own wait in the build
pool, under one time bound of 3000 s. On a loaded host one build took about 24 minutes after 10 minutes of
queue, and the second never started. `tests.run` has the same form
(`story:bugfix-runs-the-reproducing-test-before-the-suite`).

## Outcome

1. Each check command of the bug-fix runbook is an action of its own in the protocol, with its own time
   bound and its own evidence: the suite on the base, the reproducing test on the base, the reproducing
   test after the fix, the suite after the fix.
2. The order is the one that decides earliest: the reproducing test before the suite, in both places.
3. The outcomes of the case stay the ones the page lists (`not_reproduced`, `not_fixed`, fixed) plus the
   error of `story:bugfix-timeout-is-no-verdict`; the cycles of the suite that hold them are updated with
   the protocol.
4. The bound of each action is stated in the bindings; the checks file of a repository can name a longer
   time per command within the pool's limit (`story:bugfix-check-names-its-pool-time`).

## Acceptance

`runbook check` passes on the changed protocol and bindings; the cycles of the bug-fix runbook in
`cycles/tree-suite.yaml` run as `expected` with four check actions; with stand-ins, a bound that ends the
third action leaves the evidence of the first two in place and ends the case as the error.

## Origin

A consumer's defect report of 2026-11-23, relayed by the operator's host session.
