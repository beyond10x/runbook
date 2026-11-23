---
format: aep.planning-md/3
id: review-result:security-daemon-page-starts-a-run-pass-2
kind: review-result
status: active
title: 'Security pass 2 of the unit daemon-page-starts-a-run at 7210317d: red, 1 blocker, 2 warnings, 4 notes'
summary: The key ended before the body of the answer was read, and a 401 ended it, so two paths still allowed a second run. A pick made while the form loaded was thrown away. No level preset, the token handling and the guards held, and no mutant survived.
relations:
- reviews: story:daemon-page-starts-a-run
revision: 1
---
verdict: NEEDS-CHANGE

unit: `story:daemon-page-starts-a-run` at `7210317d`, tree `<trees>/b10x/runbook/page-start`, branch `impl/daemon-page-starts-a-run`

cases: executed 237 to 246 (Rust `daemon::` 210 to 210, red 0; UI 27 to 36, red 3), red 3. Rust `daemon::`: `Summary [ 19.398s] 210 tests run: 210 passed, 534 skipped`, load 76 80 89. UI, committed specs: run 1 `25 passed (2.0m)`, 2 did not run at the 120 s wall (overlapped the reviewer's own mutant build); run 2 `1 failed, 26 passed (1.0m)`, load 76 to 93; run 3 `27 passed (51.0s)`, load 73 to 76. UI, committed specs and the reviewer's first 8 new cases: `2 failed, 33 passed (1.8m)`, 105.9 s wall, load 99 to 85. The reviewer's two new files alone: `3 failed, 6 passed (37.3s)`.

origin: introduced 6 / pre-existing 1 / undecided 0

What held: the key survives an edit taken back before the next press and a close and reopen; Start is disabled while a command is on its way; the key is 128 random bits, sent in the body only; the daemon's replay rule survives a restart over the same state directory. No level is preset and no start leaves the page without a picked level at or above the floor. The token field is empty after every way of closing, and tokens the browser cannot send are forgotten with no request and no echo. The pass-1 cases are committed with their assertions; no assertion was removed. The guards of F6 and F7 are held by committed cases, and no mutant survived the committed cases. One daemon per spec file weakened no case. Nothing under `runbook/src/api`, `ui/server.mjs` or `ui/web` changed.

What did not hold: the key was dropped when the status line of the answer arrived, before the body was read, so an accepted start whose body did not arrive whole could be followed by a second run (blocker). An answer of the HTTP layer (401) ended the key, so a start after a lost answer and a 401 made a second run with no note on the form. A level picked while the form was still loading was thrown away; this fails closed, and a committed case raced with it and failed in 1 of 3 full runs. The document said `--confinement` has no default anywhere, though a `submit-run` without the member runs at `linux`. No committed case held the check of `start()` that a level was picked, on its own. The daemon replays the first run for a used key whatever the content (pre-existing).

coordinator: fix round 2 takes P1, P2, P3, P5 and P6; the reviewer's two spec files are committed first, as written. P1: the key ends only after the whole answer of the command was read and understood. P2: only an answer that names an outcome of `submit-run` ends the key; the answers of the HTTP layer keep it, with one test for each kind. P3: the second paint keeps a pick that is still at or above the floor just read. P5: the document says the form presets no level and names the level a `submit-run` without the member runs at. P6 is held by the reviewer's case once committed. P4 stays: it is the rule of the story and a committed case asserts it. P7 is pre-existing and goes to the backlog. The step must end within 120 s; if it is over 100 s, the cost paid for each file is lowered, not the cases.

```findings
[
 {"file":"ui/daemon/start.js","line":169,"category":"idempotency","severity":"blocker","verdict":"NEEDS-CHANGE","origin":"introduced","message":"The key of the form was dropped when the status of the answer arrived, before the body was read (line 174), so an accepted start whose body did not arrive whole could be followed by a second run. A guard must hold that the key ends only after the whole answer of the command was read."},
 {"file":"ui/daemon/start.js","line":169,"category":"idempotency","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"A 401, an answer of the HTTP layer that says nothing about the command, ended the key (lines 169-171), so a start after a lost answer and a 401 was a second run and the note about an earlier press was not shown. A guard must hold that only an answer of the command itself, a named outcome, ends the key."},
 {"file":"ui/daemon/start.js","line":199,"category":"race","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"open() painted with reset before and after it read the floor and the list (lines 195-199), so a level picked in between was thrown away and Start stayed disabled; this fails closed. A committed case at ui/test/security-start-idempotency.spec.mjs:108-111 raced with it and failed in 1 of 3 full runs, and ui/test/daemon-start.spec.mjs:150 and :298 pick after a second opening without waiting. A guard must hold that the second paint keeps a pick made since the first, or the cases wait for the end of open()."},
 {"file":"ui/daemon/start.js","line":66,"category":"idempotency","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"Content pressed again after other content was pressed in between gets a new key, with the note on the form. This is the rule of the story and ui/test/security-start-idempotency.spec.mjs:199-201 asserts it. The note is gone once the next start is accepted (line 177)."},
 {"file":"docs/daemon-api.md","line":336,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The document said that --confinement has no default anywhere in this product; a submit-run without the member is accepted and runs at linux. The sentence held for the form, not for the command."},
 {"file":"ui/test/daemon-start.spec.mjs","category":"test-gap","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"No committed test held the check of start() that a level was picked (ui/daemon/start.js:154) on its own: a mutant without it and without the disabled rule was killed only on the disabled state of Start. The reviewer's case ui/test/security-start-key.spec.mjs:271 holds its message."},
 {"file":"runbook/src/commands.rs","category":"idempotency","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"The replay rule of submit-run answers 202 already-submitted with the first run for a used key whatever the content. A guard must hold that a used key with other content is not dropped without a word. The page never sends that case. Backlog."}
]
```
