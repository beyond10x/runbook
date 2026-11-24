---
format: aep.planning-md/3
id: story:a-bug-fix-hands-back-only-with-a-spec-reference
kind: story
status: active
title: A bug fix hands back only with a spec reference
summary: After the gate, the bug-fix runbook writes the code merge request's description and a check program decides whether it references a spec; only a fix whose check said ok reaches hand-back, a refused one goes back to its writer.
relations:
- decomposes: epic:ticket-to-merge-request-on-a-real-repository
- informed_by: story:bugfix-hands-back-a-branch-and-a-merge-request
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: bindings/sdc-bugfix.yaml
- confidence: cited
  path: cases/12-bugfix-red-first
- confidence: cited
  path: cycles/old-suite-names.txt
- confidence: inferred
  path: cycles/seeds/bugfix-pager
- confidence: cited
  path: cycles/tree-suite.yaml
- confidence: cited
  path: docs/bugfix.md
- confidence: inferred
  path: docs/cycles-suite.md
- confidence: cited
  path: docs/loom-bindings.md
- confidence: cited
  path: docs/reject-routes.md
- confidence: inferred
  path: driver/examples/sdc
- confidence: cited
  path: driver/examples/sdc/fixtures/bugfix-pager
- confidence: cited
  path: driver/src/bin/sdc-bugfix.rs
- confidence: inferred
  path: driver/tests
- confidence: inferred
  path: protocol/check-ledger.json
- confidence: inferred
  path: protocol/compiled/sdc-bugfix.ir.json
- confidence: cited
  path: protocol/sdc-bugfix.protocol.yaml
- confidence: inferred
  path: protocol/simulate/sdc-bugfix.txt
- confidence: inferred
  path: website/data/protocols
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-24T11:47:33Z", actor: "human:operator", revision: 4, executor: "agent:runbook", correlation: "host-operator-2026-11-24-spec-first-handback"}
- {from: "proposed", to: "active", at: "2026-11-24T11:47:33Z", actor: "human:operator", revision: 5, executor: "agent:runbook", correlation: "host-operator-2026-11-24-spec-first-handback"}
---
## Why

A rule of 2026-11-24 for the consumer of the bug-fix runbook: a change of customer-visible behaviour goes
spec first, and the spec's merge request is the contract before code. A bug fix that reaches hand-back
must therefore name the spec it follows: for a change, a spec merge request of a spec repository (or a
file under the spec directory); for a defect, the spec line the fix restores, or the plain statement
that the spec says nothing. Order of the operator, 2026-11-24, passing on the consumer's request; names
and the check's interface were agreed with the consumer.

## Outcome

A bug-fix run whose fix passed the gate writes the description of its code merge request, and a check
program decides whether that description references a spec. Only a fix whose check said `ok` reaches
hand-back; a refused one goes back to the step that wrote the description and never reaches hand-back.

## The agreed interface

- After `gate.run`, the writer step `handback.describe` writes `work/handback/mr-description.md`: a
  fix summary from what the run already holds (the dissection, the reproducing test, the changed
  files of `impl/` against the base) and the ticket's text (`bug.yaml` `text`). It is a program: these
  inputs suffice, so no model call is made.
- The program action `handback.spec-check` runs the check as `<check> <description file> <kind>`,
  both positional, `<kind>` `defect` or `change`. The check prints one line on standard output,
  `spec-first: allowed: <why>` or `spec-first: refused: <why>`, and exits 0 (allowed) or 77
  (refused). A tool error of the check is also exit 77 with a reason. Exits 75, 126 and 127 stay
  program errors. No result file.
- Evidence `spec_reference`, results `ok` (exit 0) and `refused` (exit 77); its text is the line.
- Claim `spec.referenced` holds only on `ok`. It gates every action that hands back (the later action
  that opens the code merge request) and hand-back itself. While the evidence is `refused`, the fix
  does not lead to hand-back; a refusal goes back to the author step of the description, as a
  rejection goes back to its writer.
- The kind comes from the ticket intake: the key `change_kind: defect|change` of `sdc.ticket/1`,
  default `defect`, carried into `bug.yaml`.
- The check program is the consumer's: its binding names its own argv. This repository ships a
  default check for its own bindings and tests with the same interface and a generic rule: `change`
  passes with a link to a merge request of a spec repository or a path under a spec directory the
  binding names; `defect` passes with a spec line (`<file>:<line>`, or a link with a line anchor) or
  the words "spec says nothing". Rust, a subcommand of `sdc-bugfix`; no Python.

## What the model of claims and evidence cannot say, and what is built instead

- There is no state `fixed` and no move `fixed -> handback`: a case of `sdc.bugfix` ends at one
  outcome, and the hand-back of `story:bugfix-hands-back-a-branch-and-a-merge-request` takes a run
  that ended `fixed`. So the hand-back point is the outcome `fixed`: it requires `spec.referenced`
  besides the gate. No outcome reads a refused reference: Loom ends a case at its first legitimate
  outcome (`docs/cycles-suite.md`, `bugfix-not-red`), so such an outcome would end the case before
  the refusal sends the writer back. A refused reference therefore leaves the case without an
  outcome; the writer runs again, and when its runs are spent the tree is stuck (runner exit 3,
  `runbook test` exit 4), as a spent rework allowance ends elsewhere in this repository. `canon
  simulate` reports that state as one cycle finding, re-pinned with this reason. The model is not
  bent: hand-back is still an outcome read from recorded facts.
- The action that opens the code merge request does not exist yet. A precondition cannot name an
  action that is not there, so the rule is a check: a test reads the protocol and fails for any action
  named `handback.*` other than `handback.describe` and `handback.spec-check` whose precondition does
  not require `spec.referenced`.
- A binding's argv is fixed, the kind is per case. The binding runs `sdc-bugfix handback-check --
  <check argv>`, which appends `handback/mr-description.md` and the kind read from `bug.yaml`, passes
  the check's line through, and maps its exits: 0 and 77 as they are, 75, 126 and 127 as they are
  (program errors), any other exit 126 with a line that names it (the check broke the interface; never
  a result).
- The evidence's text: Loom keeps a program's output as the record's blob, so the check's one line on
  standard output is that text. No result file is read.
- The author step of the description is `handback.describe`: `spec_reference: [refused]` is in its
  `retry_when`, the mechanism a rejection uses, and its `budget.max_runs` is 2. The default writer is a
  program, so a resend writes the same description unless its inputs changed; a consumer that binds a
  model writer gets the refusal line in its feedback.
- The protocol keeps revision 1: its registration `sdc-bugfix@1` is named by committed cases and tests,
  and earlier additions of claims kept the revision too.

## Acceptance

1. A bug-fix cycle whose ticket text carries a spec reference ends `fixed`, with one `spec_reference`
   record `ok` whose blob is the check's `spec-first: allowed:` line (`bugfix-full` of
   `cycles/tree-suite.yaml`).
2. A bug-fix cycle whose description carries no reference records `spec_reference` `refused`, runs
   `handback.describe` again, never ends `fixed` and ends stuck once the writer is spent
   (`bugfix-spec-refused`, a new entry of `cycles/tree-suite.yaml`).
3. Canon's evaluation of the protocol: with the gate passed and `spec_reference` refused, `fixed` is
   blocked, no outcome is legitimate and `handback.describe` is admissible; with `ok`, `fixed` is
   legitimate (`cases/12-bugfix-red-first`).
4. The default check, kind `change`: a description with a spec merge request link or a path under the
   named spec directory exits 0 with `spec-first: allowed:`; one with only a code line exits 77 with
   `spec-first: refused:`. Kind `defect`: a `<file>:<line>` reference, a link with a line anchor or
   "spec says nothing" exits 0; none of them exits 77 (driver tests).
5. A check that exits 75, 126 or 127 makes `handback-check` exit the same code with no result, and an
   exit outside the interface exits 126 naming it (driver tests).
6. `sdc.ticket/1` takes `change_kind` `defect` or `change`, refuses any other value naming the key, and
   the seed writes the value (default `defect`) into `bug.yaml`, from where the check gets it (driver
   tests).
7. `canon check` of `sdc.bugfix` reports 0 findings, `scripts/check-bindings.sh` and `runbook check`
   pass.
8. A test reads the protocol and fails for any `handback.*` action, other than the two above, whose
   precondition does not require `spec.referenced`, and for any outcome other than `fixed` that
   reads the spec reference.

## Origin

The operator's order of 2026-11-24 on the consumer's request; names and the check's interface agreed
with the consumer the same day.
