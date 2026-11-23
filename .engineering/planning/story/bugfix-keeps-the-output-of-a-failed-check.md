---
format: aep.planning-md/3
id: story:bugfix-keeps-the-output-of-a-failed-check
kind: story
status: active
title: A bug-fix run keeps the whole output of every check command, and the evidence carries the failure lines
summary: Each check command leaves its own output file in the run; a failed check's evidence names it and carries the first failure lines, so a later step can read why it failed.
relations:
- decomposes: epic:ticket-to-merge-request-on-a-real-repository
scope:
- confidence: cited
  path: driver/src/bin/sdc-bugfix.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T12:21:20Z", actor: "human:operator", revision: 2, executor: "agent:runbook"}
- {from: "proposed", to: "active", at: "2026-11-23T12:21:20Z", actor: "human:operator", revision: 3, executor: "agent:runbook"}
---
## Defect

From the first bug-fix run of release 0.8.0 on a real repository, 2026-11-23. The run ended `not_fixed`
after 20 minutes 18 seconds and 3 model calls: the reproducing test passed after the fix, the suite ended
with 386 passed and 3 failed. The reason of the 3 failures could not be read from the run afterwards:

- The evidence of `tests.run` held the last 41 lines of output: summary lines, no assertion text.
- The check log of the step was overwritten by the next command of the same step.
- The test reports the check tool wrote into the run's tree were overwritten too.

The cause was found by hand outside the run: the fix changed a method that an existing test helper reaches
by reflection. One more try of the writer with the suite's output would likely have ended fixed.

## Outcome

1. Every check command of a bug-fix run keeps its whole output in a file of its own in the run directory,
   named by step, try and command; no command overwrites the output of another.
2. Where the checks file names a directory of test reports, its content after a failed command is kept
   under the same name, within a bound in files and bytes that the record states when it cuts.
3. The evidence of a failed check names that file and carries the lines of the first failures (the tool's
   own failure lines with what follows them, within a bound), not the tail of the output.
4. A later step of the same run can read the file: this is what `story:bugfix-writer-is-sent-back-on-loom`
   hands the writer.

## What a guard must hold

The kept output is a file of the run under `work/`, read by the inspector only through its checked open.
It is test output of the repository under test: it never reaches a console, a log line or an event of the
runner, and it is not copied into evidence beyond the bounded failure lines.

## Acceptance

With a stand-in check tool: two commands in one step each leave their own output file; a failing command
whose failure line is 500 lines above the end of its output has that line in the evidence; a report
directory over the bound is kept cut, with the count of what was left out.

## Origin

A consumer's run report of 2026-11-23, relayed by the operator's host session.
