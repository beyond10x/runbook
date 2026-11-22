---
format: aep.planning-md/3
id: story:red-then-green-as-evidence
kind: story
status: active
title: The bug-fix runbook's red then green is evidence the protocol requires, and the cycles assert on it
summary: The protocol's repro_run red and test_run passed records are the red-then-green evidence; the bug-fix cycles assert on them and on the protocol's own refusal of a fix never seen red.
tags:
- conductor-succession
- loom
relations:
- decomposes: epic:conductor-succession
- informed_by: story:bugfix-cycle
scope:
- confidence: cited
  path: cycles/suite.yaml
- confidence: cited
  path: docs/bugfix.md
- confidence: cited
  path: scripts/run-finding-cycles.sh
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:02:52Z", actor: "human:operator", revision: 2, executor: "agent:rb2-u3-red-then-green", correlation: "operator-standing-acceptance-2026-11-22"}
- {from: "proposed", to: "active", at: "2026-11-22T01:02:52Z", actor: "human:operator", revision: 3, executor: "agent:rb2-u3-red-then-green", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: red-then-green-as-evidence

## Outcome

The bug-fix runbook proves a fix by "red then green" (the regression test fails on the tree before
the fix and passes after it). Under Loom the cycle suite can only assert on what the runtime
records: evidence a check program submits. This story finds what the protocol already records,
makes the cycles assert on exactly that, and shows the protocol itself refusing a fix whose test was
never red. Decision already taken with the Loom session: red-then-green is recorded as evidence by
the bug-fix protocol's check programs.

## What exists (read before any change)

- Which program decides red and green: `sdc-bugfix repro-run` (red) and `sdc-bugfix tests-run`
  (green), `driver/src/bin/sdc-bugfix.rs` (`repro_run`, `tests_run`), called through
  `bindings/tools/sdc-tool.sh bugfix repro-run|tests-run`, bound in
  `driver/examples/sdc/sdc-bugfix.bindings.json` and `bindings/sdc-bugfix.yaml`.
- Red: the program copies the unchanged tree plus the new test, runs the one test (non-zero exit that
  matches the `red` rule) and the rest of the suite (exit 0). Its exit status 0 is recorded as
  `result: red`, any other as `not_red`.
- Green: the program checks the test file is byte for byte the red one, runs the test and the
  whole suite on the fixed tree (both exit 0). Exit 0 is `result: passed`, any other `failed`.
- What each writes: a `canon-evidence/1` record by the runtime from the binding, never a file of the
  script. `canon-evidence/1` refuses unknown fields, so the exit status is not a field of the record:
  the result word carries it (`red` = the test exited non-zero and the rest passed; `passed` = exit 0),
  the numeric status is in the step's log blob.
- Evidence kinds of `protocol/sdc-bugfix.protocol.yaml`: `dissection_check`, `repro_run`,
  `implementation_change`, `test_run`, `story_record`, `gate_run`.
- The two observations already are evidence kinds, with the digests: `repro_run` `red`, subject
  `repro` at the test's revision, `upstream_revisions.change` the digest of the unchanged tree it was
  observed on; `test_run` `passed`, subject `change` at the fixed tree's digest,
  `upstream_revisions.repro` the test's revision.
- The protocol already requires them: `change.fix` needs claim `repro.red`; `tests.run` and
  `story.update` need `repro.red` and `change.built`; outcome `fixed` needs `repro.red` and
  `tests.pass`. `cases/12-bugfix-red-first/green-without-red` is a fix and a green suite with no red
  record: `change.fix`, `story.update` blocked and `fixed` blocked.
- What the cycles asserted: `red_then_green` in `scripts/run-finding-cycles.sh` (a jq over the
  case's evidence records, not a file the script writes), and for `bugfix-never-red` zero red
  records and zero `implementation_change` records.

## Decision

No second evidence kind and no protocol change: the protocol already requires an equivalent
evidence (`repro_run` red then `test_run` passed). The cycles are made to assert on it, more
tightly, and `bugfix-never-red` also asserts the protocol's own evaluation of the case.

## Acceptance

- A fix whose test was never red is refused by the protocol evaluation: for the recorded case
  `cases/12-bugfix-red-first/green-without-red` `canon evaluate` gives `change.fix` blocked and
  `fixed` blocked, and `evaluate-cases.sh` holds it; the cycle `bugfix-never-red` asserts on the run's
  `decision.json` that `repro.red` is false, `change.fix` blocked and `fixed` blocked.
- The evidence of a good fix holds both observations with their digests: `bugfix-full` asserts one
  `repro_run` red with a tree digest, one `implementation_change` after it, one `test_run` passed at
  a different tree digest naming the red test's revision.
- The recorded cases (52 `decision.json`) still evaluate to their expected outcomes, and both
  binding forms stay in step (`scripts/check-bindings.sh`).
- `bugfix-never-red` and `bugfix-full` pass one by one, each under 60 s.
- `task check:cycles-suite` and `aep plan artifact validate` exit 0.

## Scope

`scripts/run-finding-cycles.sh` (the `bugfix-*` cycle bodies), `cycles/suite.yaml` (their entries),
`docs/bugfix.md`, `docs/cycles-suite.md`.
