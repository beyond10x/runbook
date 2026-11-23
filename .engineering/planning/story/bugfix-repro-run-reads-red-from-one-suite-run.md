---
format: aep.planning-md/3
id: story:bugfix-repro-run-reads-red-from-one-suite-run
kind: story
status: draft
title: repro.run of the bug-fix runbook reads its verdict without a second full suite job
summary: repro.run runs the reproducing test on the tree and then the whole suite on the base, two pool jobs in sequence (947 s measured by a consumer); the verdict comes from one suite run read per test, a kept base verdict, or two jobs at once.
relations:
- decomposes: epic:ticket-to-merge-request-on-a-real-repository
- informed_by: story:bugfix-runs-the-reproducing-test-before-the-suite
- informed_by: story:bugfix-check-names-its-pool-time
scope:
- confidence: cited
  path: docs/bugfix.md
- confidence: cited
  path: driver/src/bin/sdc-bugfix.rs
revision: 3
---
## Defect

`repro.run` of the bug-fix runbook decides "the new test is red and nothing else is" from two pool jobs
in sequence (`driver/src/bin/sdc-bugfix.rs`, `repro_run`): the reproducing test alone on the changed
tree, then the whole suite on the unchanged base. A consumer measured one such step at 339 s plus 608 s,
947 s in total. In four runs on the same ticket the base tree was byte-identical each time, and its suite
ran three times.

## Outcome

The step reaches the same verdicts (`red`, `not_red`, and no verdict when a command did not finish) in
less pool time. Three ways were named by the consumer; the design is settled before the build:

1. One suite run on the changed tree, read per test: the check tool reports each test's result (a
   report directory in a format the checks file names), and the step reads "only the reproducing test
   failed" from it. Cost measured by the consumer: about 36 s more than the reproducing test alone.
   Needs a checks-file member for the per-test report and a rule for a suite that reports nothing.
2. The base suite's verdict is kept per digest of the base tree, the checks file and the check tool,
   and read instead of a second run. Saves nothing in a first run. Needs a place for the record that a
   program of a run cannot write, and a rule for when it is stale.
3. Both jobs run at the same time, when the checks file says the check tool allows two runs at once
   (fixed ports, shared build directories).

Guards whatever the way: a verdict is recorded only from a command that ran in the pool and finished
(`story:bugfix-check-names-its-pool-time`); a kept verdict is never read for a base tree, a checks file
or a tool that differs by one byte; a report the check tool wrote is read as data with a size bound,
through no link, and its text reaches no console, log line or event of the runner.

## Acceptance

With a stand-in check tool that records its calls and their times: for the way chosen, the step's
record names each command it ran and each verdict it read without running; a base that differs by one
byte runs its suite again; a suite report that names a second failing test gives `not_red`; a report
that is missing, cut or unreadable gives the two-job path or no verdict, never `red`. `docs/bugfix.md`
says which commands `repro.run` runs and when.

## Origin

A consumer's review of a bug-fix run of 2026-11-23 (durations and step names only), relayed on the
operator's host session's order.
