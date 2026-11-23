---
format: aep.planning-md/3
id: review-result:security-run-settings-are-toml-pass-2
kind: review-result
status: active
title: 'Security pass 2 of the unit run-settings-are-toml at 20581a7f: green, 0 blockers, 1 warning (test gap), 6 notes'
summary: No refusal of the reader prints text of the file, held over 343 syntax-error files and every number a setting cannot take. One mutant is killed only by the reviewer's cases, which are committed with the unit. Four older value echoes are backlog.
relations:
- reviews: story:run-settings-are-toml
revision: 1
---
verdict: approve

unit: `story:run-settings-are-toml` at `20581a7f`, tree `<trees>/b10x/runbook/settings-toml`, branch `impl/run-settings-are-toml`

cases: executed 78 to 86 (the settings, core, login and decider lanes of the runbook `cli` binary), red 0. `test result: ok. 86 passed; 0 failed; 1 ignored`, 21.47 s, load 89 at start and 81 at end. Driver: 23 passed in 0.94 s; `sdc-experiment` unit tests 24 passed.

origin: introduced 5 / pre-existing 2 / undecided 0

What held: no refusal of the reader prints text of the file. The parser's words are built from fixed text only, read in the pinned parser crates; 343 files (49 syntax errors at 7 placements, a marker text on the bad line, on the neighbour lines, in a key, in a table name and after a byte order mark) each end with exit 2, nothing started, no part of the marker on either stream and under 700 bytes of standard error. A number a setting cannot take (a date, an integer over 64 bits, a float) is refused with the member named and the value not printed, at the top, in a table, in an array, in an inline table and with a dotted key. A JSON file is refused as JSON after a mark, blank lines and comment lines. The walk reads what the library read: nothing is accepted that was refused before the fix round and no number is read as another. The replaced slow test holds its claim on the child rig in 1.03 s. The merge of `main` added nothing of the unit. Ten of eleven mutants are killed by a committed case.

What did not hold: one mutant (a refusal of an integer over 64 bits prints the integer's own text) passes every committed case and fails only the reviewer's two number cases. Four refusals of older code print the file's value of `format`, `run`, `confinement` and `decider.principal` without a bound, as on `main`; none of the four is the login member. Member names are printed raw and without a bound, as on `main`.

coordinator: the reviewer's cases are committed before the unit merges. The four older value echoes, the unbounded member names and the size bound of the settings file go to the backlog. The 160-character cut of the parser's words stays as defence; no input reaches it.

```findings
[
 {"file":"runbook/tests/loom_run/settings_guards.rs","category":"test-gap","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"The mutant in which a refusal of an integer over 64 bits prints the integer's own text passes every committed case; the reviewer's two number cases kill it. Committed with the unit."},
 {"file":"runbook/src/loom_run/settings.rs","line":150,"category":"information-disclosure","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"Four refusals print the file's value of format, run, confinement and decider.principal without a bound. A guard must hold that a refusal names the member and prints no value of the file. Same code on main. Backlog."},
 {"file":"runbook/src/loom_run/settings.rs","line":145,"category":"information-disclosure","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"Member names are printed raw and without a bound in refusals. Names only, never values; same on main. Backlog."},
 {"file":"runbook/src/loom_run/settings.rs","line":78,"category":"message","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"An integer between 2^63 and 2^64 is refused with the words that it must fit 64 bits, though it fits unsigned. It is refused as before the fix round; only the wording is off."},
 {"file":"runbook/tests/loom_run/settings.rs","line":476,"category":"test-gap","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"After the move to the child rig no live run is compared by bytes with the in-process golden records. Low risk: the in-process run does not read the input form."},
 {"file":"driver/tests/settings_record_order.rs","category":"test-gap","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The order-independence case runs three readers of the records. The other readers read by member name by inspection; no case runs them."},
 {"file":"runbook/src/loom_run/settings.rs","line":28,"category":"test-gap","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"The 160-character cut of the parser's words cannot be held by a case: every parser message is built from fixed strings shorter than the cut."}
]
```
