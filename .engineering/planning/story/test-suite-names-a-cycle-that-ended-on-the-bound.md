---
format: aep.planning-md/3
id: story:test-suite-names-a-cycle-that-ended-on-the-bound
kind: story
status: draft
title: runbook test shows a cycle that ended on its bound as failed, naming step and cause
summary: No code or case of the repository names CycleTimedOut; the printed line is Loom's own text. A note of adversary pass 1 of the Loom pin unit c9b4c177.
relations:
- informed_by: story:loom-pin-c9b4c177
revision: 1
---
## Outcome

`runbook test` shows a cycle that ended on its time bound as a failed cycle that names its step and the cause `CycleTimedOut`, and a case holds that line.

## Source

Adversary pass 1 of `story:loom-pin-c9b4c177` measured that `DifferenceReason::CycleTimedOut` is named only in a doc comment of `runbook/src/test_suite.rs`: no code of `runbook/src` or `driver/src` matches it, and the line printed for it is Loom's own text (`FAILED: steps[1] (run, CycleTimedOut): expected an end within 1 s, found 1.3`). The line is distinguishable by a reader, but no case of the repository holds it.

## Scope

- `runbook/src/test_suite.rs`, `runbook/tests/test_suite/`.
- `docs/cli.md` (the sentence on `--cycle-timeout`).

## Acceptance

- A case with `--cycle-timeout 1` and a cycle that runs longer ends with a non-zero exit, prints a line that names the step and `CycleTimedOut`, and counts the cycle as failed in the closing line. The other cycles of the suite are unaffected. The case runs under 10 s.

## Origin

Introduced by the pin unit `story:loom-pin-c9b4c177` (the option `--cycle-timeout`). Held by the review record of its adversary pass 1 (note N2); fix round 2 left it as it was.
