---
format: aep.planning-md/3
id: review-result:security-run-settings-are-toml-pass-1
kind: review-result
status: active
title: 'Security pass 1 of the unit run-settings-are-toml at 7259b5d7: red, 1 blocker, 2 warnings, 4 notes'
summary: A refusal of a syntax error printed text of the settings file, a JSON file after a mark lost its JSON hint, and the JSON cases passed with the refusal removed. The guards on confinement, caps and unknown keys held.
relations:
- reviews: story:run-settings-are-toml
revision: 1
---
verdict: NEEDS-CHANGE

unit: `story:run-settings-are-toml` at `7259b5d7`, tree `<trees>/b10x/runbook/settings-toml`, branch `impl/run-settings-are-toml`

cases: executed 260 to 273 (`loom_run::` of the runbook `cli` binary), red 3. Driver `main` binary: 21 to 22, red 0.

origin: introduced 5 / pre-existing 2 / undecided 0

What held: `--confinement` has no default and is never lowered by a file; a cap is never read as larger, as absent or as unlimited; a member has one place; an unknown key is refused by name at the top and in every table; the login path is replaced in the record.

What did not hold: a refusal of a syntax error printed text of the settings file, where the reader on `main` printed only a line and a column. A guard must hold that no refusal of the settings reader prints a line, a value or a part of a value of the file. A JSON file after a byte order mark or a comment line was refused as a TOML syntax error without the JSON hint the page promises. The committed JSON cases passed with the JSON refusal removed. The records follow the order of the settings file, which differs from 0.7.0 for a file `sdc-experiment` wrote, and the page did not say so. The page did not say which spellings of a whole number are read.

coordinator: fix round 1 takes findings 1, 2, 3, 4 and 6 and replaces a test of 23.1 s by one on the child rig. Finding 5 (the settings file has no size bound) and finding 7 (a scripted decider's limit of the wrong type) exist without the unit and go to the backlog.

```findings
[
 {"file":"runbook/src/loom_run/settings.rs","line":23,"category":"integrity","severity":"blocker","verdict":"NEEDS-CHANGE","origin":"introduced","message":"A refusal of a TOML syntax error printed text of the settings file. A guard must hold that a syntax error names the file, the line number, the column and the parser's words for the kind of error, and no text of the file."},
 {"file":"runbook/src/loom_run/settings.rs","line":29,"category":"contract-drift","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"A JSON file after a byte order mark or a comment line was refused only as a TOML syntax error, without the JSON hint the page promises."},
 {"file":"runbook/tests/loom_run/settings.rs","line":277,"category":"mutant","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"The committed JSON-refusal cases asserted only TOML and the page name, so the mutant that removes the JSON refusal passed them. A case that requires the word JSON kills it."},
 {"file":"runbook/src/loom_run/settings.rs","line":100,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The records settings.json and run-settings.jsonl follow the member order of the settings file; on main the order differed for a launcher-written file. Equal by member name."},
 {"file":"runbook/src/loom_run/settings.rs","line":19,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"The settings file has no size bound. A guard must refuse a file over a stated size before it is parsed. Backlog."},
 {"file":"docs/loom-run-settings.md","category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"Hex, octal, binary, underscored and plus-signed integers are read as whole numbers for decider and model limits and the page did not say so; the option-bound caps must equal the option."},
 {"file":"runbook/src/loom_run/decide.rs","line":102,"category":"boundary","severity":"note","verdict":"INFEASIBLE","origin":"pre-existing","message":"A scripted decider's max_call_micro_usd of the wrong type reads as 0 instead of being refused; the scripted decider is a test double no record can name."}
]
```
