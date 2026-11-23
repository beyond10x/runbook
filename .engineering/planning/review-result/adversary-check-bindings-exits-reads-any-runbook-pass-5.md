---
format: aep.planning-md/3
id: review-result:adversary-check-bindings-exits-reads-any-runbook-pass-5
kind: review-result
status: active
title: 'Adversary pass 5 of the unit check-bindings-exits-reads-any-runbook at 9eed401e: red, 0 blockers, 2 warnings, 4 notes'
summary: The script held 1,035 mutations of the YAML bindings file with no raw message and no exit other than 0 or 1. The page and the changelog claimed more than the table covers, and the table was one test of 43.5 s. A closing step without a change of the script narrowed the sentences and split the table; four older kinds of file are a backlog story.
relations:
- reviews: story:check-bindings-exits-reads-any-runbook
revision: 1
---
verdict: NEEDS-CHANGE

unit: `story:check-bindings-exits-reads-any-runbook` at `9eed401e`, tree `<trees>/b10x/runbook/exits-root`, branch `impl/check-bindings-exits-reads-any-runbook`

cases: executed 52 to 55, red 3. Filter `check_bindings` in `driver/`: `test result: FAILED. 52 passed; 3 failed; 0 ignored; 0 measured; 480 filtered out; finished in 53.75s`, 61.0 s wall, load 110.4 to 120.4. The 52 committed cases are green; the 3 failures are the reviewer's red cases, all on behaviour the base has too. The table test alone: `1 passed; finished in 43.51s` at a load of 92 to 114.

origin: introduced 2 / pre-existing 4 / undecided 0, counted from the findings of the report.

What held: the two findings of pass 4 are fixed, and the mutant of each is killed by its case and by two cells of the table. 1,035 mutations of the YAML bindings file of a protocol (47 members with 22 values each), in both modes, gave no exit other than 0 or 1 at the head, no raw line of `jq` or `yq` and no exit 1 without a `FAIL` line that names the file; nothing is accepted at the head that either earlier script refused. Nine of ten mutants of the script are killed by the table or a named case; the tenth is equivalent. The table is red in 70 of 872 runs against the script before round 4. The diff of the script against that script holds only the hunks of round 4, and the merge of the base did not touch it. The full check and `--exits .` exit 0 on the tree. The changelog entry stands under the unreleased heading, and the released part is byte-equal to the base.

What did not hold: the page and the changelog said every member the script reads takes every JSON type with a line that names the file; the table and the fix cover the YAML bindings file of a protocol only. The table was one test of 872 runs and took 43.5 s under load, more than a test of this repository may cost beside its neighbours. Four kinds of file are outside the rule, each the same on the base: YAML that `yq` cannot write as JSON, a bindings file of more than one YAML document, the JSON copies of the bindings, and the protocol files the registrations step reads.

coordinator: decision D684 of the operator's delegate: one closing step and no sixth pass, on the condition that the step changes no line of `scripts/check-bindings.sh`; had it changed one, the unit would have been stopped. The step narrowed the two sentences to what the script does, split the table into one test per member (32 member tests, 436 cells, 872 runs, a case that counts them; the slowest member test 6.9 s at a load of 85 to 99; 70 runs red against the script before round 4, as before the split) and committed the reviewer's three cases as ignored measurements that name the backlog story. Checked at the unit's last commit: the diff of the script against the head of this pass is empty. The four kinds of file are the backlog story `check-bindings-refuses-every-file-by-name`: every file the script reads is refused by a line that names it, with exit 1.

```findings
[
 {"file":"docs/loom-bindings.md","line":175,"category":"documentation","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"The page and the changelog entry said every member the script reads takes every JSON type and ends in a line that names the file. That holds for the YAML bindings file of a protocol, which the table covers, and not for the other files the script reads. Narrowed in the closing step."},
 {"file":"driver/tests/check_bindings_exits_value_table.rs","category":"test-budget","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"The table was one test of 872 runs of the script and took 43.5 s at a load of 92 to 114. Split in the closing step into one test per member with the same cells; the slowest takes 6.9 s."},
 {"file":"scripts/check-bindings.sh","line":92,"category":"refusal-form","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"YAML that yq cannot write as JSON must be refused by a FAIL line that names the file. At base and head the exits step ends in exit 1 with the message of yq and no file named. Backlog: check-bindings-refuses-every-file-by-name; an ignored measurement holds it."},
 {"file":"scripts/check-bindings.sh","line":162,"category":"refusal-form","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"A member of a JSON copy of the bindings must be read only after its type is checked. At base and head a member of the wrong type ends the full check in a raw message of jq, exit 5. Backlog: the same story; an ignored measurement holds it."},
 {"file":"scripts/check-bindings.sh","line":282,"category":"refusal-form","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"A protocol file the registrations step reads must be refused by name when its id or revision is no text. At base and head the full check ends in a raw message of jq, exit 5. Backlog: the same story; an ignored measurement holds it."},
 {"file":"scripts/check-bindings.sh","line":186,"category":"refusal-form","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"A bindings file of more than one YAML document must be refused by a line that names the file. At base and head the full check prints a raw message of jq beside its FAIL lines, exit 1. Backlog: the same story; no case yet."}
]
```
