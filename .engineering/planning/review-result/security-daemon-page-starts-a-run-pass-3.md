---
format: aep.planning-md/3
id: review-result:security-daemon-page-starts-a-run-pass-3
kind: review-result
status: active
title: 'Security pass 3 of the unit daemon-page-starts-a-run at 4df26618: red, 0 blockers, 1 warning, 4 notes'
summary: A start with a level of an earlier opening could leave the page while the floor was read again; the daemon refused it and no run started. One form held at most one run for 20 kinds of answer, the token was found nowhere, and all seven mutants were killed.
relations:
- reviews: story:daemon-page-starts-a-run
revision: 1
---
verdict: NEEDS-CHANGE

unit: `story:daemon-page-starts-a-run` at `4df26618`, tree `<trees>/b10x/runbook/page-start`, branch `impl/daemon-page-starts-a-run`

cases: executed 40 to 55 (Playwright, project wide), red 1; Rust `daemon::`, `ui_embed` and `ui_guard` 210, unchanged. Rust: `Summary [ 19.945s] 210 tests run: 210 passed, 535 skipped`, exit 0, load 90.73 101.45 100.76 before. UI, the 40 committed cases in three runs: `16 passed (1.1m)`, `10 passed (57.0s)` and `14 passed (48.8s)`, load 82 to 118. The reviewer's red case alone: `1 failed`, 8.8 s wall. The eight committed files in three groups took 184 s of clock in sum.

origin: introduced 5 / pre-existing 0 / undecided 0

What held: one form starts at most one run for 20 kinds of answer after the daemon took the command, and through a sequence of a lost answer, a 500, a 401 and a cut body. A slow body ending after close and reopen leaves Start disabled and gives one run. The second paint keeps a pick that is still offered and drops a level below the floor just read. A form closed between the paints stays closed and gets nothing put back into the token field. The token is found in none of the places searched after each event of the claim. Content never becomes markup. Nothing outside `ui/daemon/start.js`, the tests, `docs/daemon-api.md` and `CHANGELOG.md` changed in round 2, and the changelog tail is byte-equal to the base. All seven mutants were killed by committed cases.

What did not hold: while the form read the floor of a new opening, a start with a level that only the floor of an earlier opening offered could leave the page. The daemon refused it with 409 below-floor and no run started, so the finding is a warning that is red by the brief's own line. A runbook picked between the paints that left the list was replaced silently by the first one, with Start enabled. A comment on the key rule listed 5xx among the answers that keep the key, while the code ended the key for any answer that names an outcome. A late answer of an earlier press could empty an intake typed into the reopened form.

coordinator: fix round 3 takes F1, F3, F4 and F5; the reviewer's four spec files are committed first under names that say what they hold, with the red case unchanged. F1: from `open()` until the floor of that opening was read and painted, Start is disabled and a press sends nothing. F3: no runbook is selected, Start is disabled, and the form says the picked runbook is no longer listed. F4: a 5xx keeps the key whatever its body names; the key ends only for an answer of the command itself. F5: an answer is applied only to the opening that sent it. F2 is not changed; the comment at that place says what such an answer does. P4 and P7 stand. The step time of the UI specs goes to the story `ui-test-step-budget`; a timing change that replaces fixed waits by gates the test opens was decided before pass 4.

```findings
[
 {"file":"ui/daemon/start.js","line":154,"category":"acceptance","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"While the form reads the floor of an opening, a start with a level that only the floor of an earlier opening offered left the page and was refused 409 below-floor, with no run started (ui/test/security-start-3-floor.spec.mjs:48, red). A guard must hold that no start leaves the page from open() until the floor of that opening was read and painted."},
 {"file":"ui/daemon/start.js","line":185,"category":"boundary","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"A guard would hold that an answer that names an outcome but no run the page can show does not end the key while the form stays filled. This daemon sends no such answer: both 2xx outcomes carry the run id and no 5xx names an outcome. Left unchanged by decision; the comment at that place says what such an answer does."},
 {"file":"ui/daemon/start.js","line":114,"category":"concurrency","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"A runbook picked between the two paints that left the list was replaced silently by the first runbook of the list with Start enabled. A guard must hold that no runbook is selected, or the form says so."},
 {"file":"ui/daemon/start.js","line":176,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The comment on the key rule listed 5xx among the answers that keep the key, while the code ended the key for any answer that names an outcome whatever its status. Decided: a 5xx keeps the key whatever its body names, and the comment says what the code does."},
 {"file":"ui/daemon/start.js","line":197,"category":"concurrency","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"A late answer of an earlier press could empty an intake typed into the form after it was closed and opened again. A guard must hold that an answer is applied only to the opening that sent it. Read from the code and from a passing case, not from a logged value."}
]
```
