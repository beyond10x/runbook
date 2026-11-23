---
format: aep.planning-md/3
id: review-result:security-inspector-step-products-pass-4
kind: review-result
status: active
title: 'Security review pass 4 of the inspector unit (step products) at bf3097f8: red, round 3 holds, two older reads outside the checked open'
summary: The guards of round 3 hold under mutation. Two reads in ui/decisions.mjs, older than the unit, did not go through the one checked open with the run as root; both went back to the implementor because the unit states that rule. Two notes for the backlog.
relations:
- reviews: story:inspector-shows-step-products
revision: 1
---
verdict: NEEDS-CHANGE

unit: `story:inspector-shows-step-products` at `bf3097f8`, tree `<trees>/b10x/runbook/runbook-ui-any-runbook`, branch `feat/inspector-step-products`

cases: two new spec files, `4 failed, 2 passed` and `1 failed`; the specs of round 3 `15 passed`, `17 passed`; `daemon-ui` and `read-only` `7 passed`; `ui_guard` and `ui_embed` `36 passed`.

origin: introduced 2 notes / pre-existing 2 warnings and 1 note

What held: `readText` refuses a file with more than one name; the three direct opens of `ui/server.mjs` go through `readRegular` with `at`; the listing stops at its limit; a link into a dot-directory answers like a path outside `work/`.

What did not hold: two reads in `ui/decisions.mjs`, both older than the unit, did not go through the one checked open with the run as root. The unit's rule ("every read of a run's file goes through the run-path check and the one checked open") needs them, so both went back to the implementor (fix round 4, commits `614070fa`, `2c593f4b`, `ad204866`).

coordinator: findings 1, 2 and 4 are fixed in round 4 and confirmed by pass 5. Findings 3 and 5 are filed in the backlog (`story:inspector-run-answer-is-bounded`).

```findings
[
 {"file":"ui/decisions.mjs","line":148,"category":"read-outside-run","severity":"warning","verdict":"NEEDS-CHANGE","origin":"pre-existing","message":"lastError read the files a run's own record names without the run as root. Guard to hold: the read is rooted at the run (a step log at the case directory, inside blobs/) and refuses a dot part. Held after round 4 by ui/test/decisions-reads-the-run.spec.mjs."},
 {"file":"ui/decisions.mjs","line":49,"category":"read-outside-run","severity":"warning","verdict":"NEEDS-CHANGE","origin":"pre-existing","message":"The YAML of an artifact was parsed by a program that opened the file by name after the check. Guard to hold: the text read through the checked open is what is parsed (yq on standard input). Held after round 4 by ui/test/decisions-yaml-parses-what-was-read.spec.mjs."},
 {"file":"ui/server.mjs","line":563,"category":"denial-of-service","severity":"note","verdict":"NEEDS-CHANGE","origin":"pre-existing","message":"fsChanges reads a changes file of any size in chunks within one request; the work per request has no bound. Backlog: story:inspector-run-answer-is-bounded."},
 {"file":"ui/test/runfiles-opens.spec.mjs","category":"test-coverage","severity":"note","verdict":"NEEDS-CHANGE","origin":"introduced","message":"No spec held the routing of fsChange through readRegular, the clamp of at or its limit. Held after round 4 by ui/test/runfiles-at.spec.mjs and ui/test/runfiles-inventory.spec.mjs."},
 {"file":"ui/server.mjs","line":106,"category":"visibility","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"A refused read of events.jsonl or changes.jsonl shows as absent; the page does not say that a read was refused. By design (fail closed); backlog note."}
]
```
