---
format: aep.planning-md/3
id: review-result:adversary-check-bindings-exits-reads-any-runbook-pass-4
kind: review-result
status: active
title: 'Adversary pass 4 of the unit check-bindings-exits-reads-any-runbook at 5c9f20f1: red, 0 blockers, 1 warning, 3 notes'
summary: An executor.argv of false passed the shape check and ended both modes in a raw message of jq, against the sentence of the page. A result key ending in a newline did the same under --exits. Round 4 took both and added a table of every member with every JSON type.
relations:
- reviews: story:check-bindings-exits-reads-any-runbook
revision: 1
---
verdict: NEEDS-CHANGE

unit: `story:check-bindings-exits-reads-any-runbook` at `5c9f20f1`, tree `<trees>/b10x/runbook/exits-root`, branch `impl/check-bindings-exits-reads-any-runbook`

cases: executed 48 to 51, red 2. `cargo test --locked --test main check_bindings` in `driver/`: `48 passed; 2 failed; 446 filtered out; finished in 6.50s`. The 48 committed cases are green; the 2 failures are the reviewer's red cases, and a third case of the reviewer is green.

origin: introduced 2 / pre-existing 2 / undecided 0, counted from the findings of the report.

What held: every form of a link that points at nothing is refused under `--exits` by one line that names it (alone, beside good files, named like a pool file, a link to a directory, a chain with a missing end, a loop), and the full check prints its `FAIL exits` line. A read of the exits step that fails ends the script non-zero. Three mutants of the registrations step (both readers made to succeed, the bindings reader made to succeed, the flag test never failing) are killed by the committed case. The output of the full check on the committed files, cut before the answers file, is byte-equal to the base on the merged tree. `cd --`, a null `executor`, and `evidence` that is a mapping or a number each give one line that names file and action. The changelog entry stands under the unreleased heading, and the released part is byte-equal to the base.

What did not hold: a value of `executor.argv` that is `false` was taken as an empty list by the shape check and ended both modes in a raw message of `jq` with exit 5, while the page and the changelog said an `executor.argv` that is no list is refused by one named line. A result key that ends in a newline passed the check of the key's form and ended `--exits` in a raw message of `jq`. A fourth mutant of the registrations step (the reader of the protocol files made to succeed) survived the committed case, whose fixture holds no unreadable protocol file. In two older steps (the instruction files of the JSON copies, the answers file) a failing reader ends the full check in exit 0; base and head behave the same.

coordinator: fix round 4 takes the two introduced findings, commits the reviewer's three cases (two red at that commit, one that kills the surviving mutant) and adds a table of every member the script reads from the YAML bindings file of a protocol with every JSON type. A fifth pass follows on that diff (decision D675 of the operator's delegate: merge only after an introduced red is fixed, and stop the unit after a fifth pass). The two older steps are the backlog story `check-bindings-steps-fail-on-a-failed-reader`: a step whose reader fails must fail the script.

```findings
[
 {"file":"scripts/check-bindings.sh","line":33,"category":"shape-check","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"A member of a bindings file must be read only after its type is checked. At this head an executor.argv that is false passed the shape check as an empty list and ended both modes in a raw message of jq, exit 5, with no FAIL line, where the page and the changelog said such a value is refused by one named line. The reviewer's case is committed."},
 {"file":"scripts/check-bindings.sh","line":31,"category":"shape-check","severity":"note","verdict":"NEEDS-CHANGE","origin":"introduced","message":"The check of a result key's form must hold to the end of the text. At this head a key that ends in a newline passed it and ended --exits in a raw message of jq, exit 5. The reviewer's case is committed."},
 {"file":"scripts/check-bindings.sh","line":232,"category":"lost-status","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"The step over the instruction files of the JSON copies must fail the script when its reader fails. At base and head a failing reader ends the full check in exit 0 with the step's pass line. Backlog: check-bindings-steps-fail-on-a-failed-reader."},
 {"file":"scripts/check-bindings.sh","line":310,"category":"lost-status","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"The step over the answers file must fail the script when its reader fails. At base and head a failing reader ends the full check in exit 0 with the step's pass line. Backlog: the same story."}
]
```
