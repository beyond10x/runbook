---
format: aep.planning-md/3
id: review-result:security-inspector-step-products-pass-5
kind: review-result
status: active
title: 'Security review pass 5 of the inspector unit (step products) at 57c0287f: green, four warnings inside the run for the backlog'
summary: No read of a run file leaves the run and no program opens a run file by name; six of seven mutants killed. Four warnings stay inside the run (a dot part checked by name only, the cost of a YAML parse, the planning route walking dot directories, the inventory spec seeing one spelling) and are filed as backlog stories.
relations:
- reviews: story:inspector-shows-step-products
revision: 1
---
verdict: approve

unit: `story:inspector-shows-step-products` at `57c0287f` (fix round 4: `614070fa`, `2c593f4b`, `ad204866`; merge of `main` `3c0eb863`), tree `<trees>/b10x/runbook/runbook-ui-any-runbook`, branch `feat/inspector-step-products`

cases: `50 passed (22.1s)` and `47 passed (1.2m)` on the specs the round touched; seven mutants of the windowed read and the checked open, six killed (`[head] 3 failed`, `[offset] 3 failed`, `[position] 1 failed`, `[clamp] 1 failed`, `[limit] 5 failed`, `[position-guard] 1 failed`), one alias form of a direct open not seen by the inventory spec (`11 passed`); an attack spec of 39 names for the two reads of round 4, `3 failed, 36 passed`. Load average 69 to 86.

origin: introduced 1 warning / pre-existing 3 warnings and 1 note

What held: no read of a run's file leaves the run, and no program opens a run's file by name. `lastError` reads under the run (39 forms of a name tried: none reads outside the run, none throws). Every `yq` call gets the text on standard input with constant arguments. Every other read of `ui/server.mjs` takes a fixed name, a name a listing of the run returned, or a 64-digit hex digest. `readRegular` with `at` refuses a position that is not a whole number of bytes. The merge of `main` changed only the version files under `ui/`.

coordinator: the unit merges. The four warnings stay inside the run and are filed as one backlog story (`story:inspector-dot-directories-by-real-path`: findings 1, 3 and 4 of the list below) and added to `story:inspector-run-answer-is-bounded` (finding 2). The reviewer's three red cases for finding 1 are kept outside the tree for that story.

```findings
[
 {"file":"ui/decisions.mjs","line":159,"category":"dot-directory","severity":"warning","verdict":"NEEDS-CHANGE","origin":"pre-existing","message":"The refusal of a dot part in lastError and artifactPath looks at the name, not at the real path. Guard to hold: a real path with a dot part is refused by the checked open. Stays inside the run."},
 {"file":"ui/decisions.mjs","line":53,"category":"denial-of-service","severity":"warning","verdict":"NEEDS-CHANGE","origin":"pre-existing","message":"A parse of an artifact's YAML can hold the server for the 10 s of its timeout, and artifacts are parsed again on every poll. Guard to hold: a size bound on the text before the parse and one parse per content digest. Not a read outside the run."},
 {"file":"ui/server.mjs","line":624,"category":"dot-directory","severity":"warning","verdict":"NEEDS-CHANGE","origin":"pre-existing","message":"The route that returns the planning files of work/ walks into dot-named sub-directories, against the rule the file route holds. Guard to hold: the walk skips dot-named directories; the one fixed planning directory is a stated exception or goes."},
 {"file":"ui/test/runfiles-inventory.spec.mjs","line":12,"category":"test-coverage","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"The inventory of file reads counts one spelling of a call in ui/*.mjs and does not see other ways to import or call the file system or a child process. Guard to hold: an allow-list of the imports of node:fs* and node:child_process per module and no dynamic import of them."},
 {"file":"ui/server.mjs","line":782,"category":"name-from-a-run-record","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"The record member of the call log only filters names a listing returned; it is not used as a path. Held."}
]
```
