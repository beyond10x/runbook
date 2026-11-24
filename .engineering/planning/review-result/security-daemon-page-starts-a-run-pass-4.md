---
format: aep.planning-md/3
id: review-result:security-daemon-page-starts-a-run-pass-4
kind: review-result
status: active
title: 'Security pass 4 of the unit daemon-page-starts-a-run at 029b2690: green with a change needed before merge, 0 blockers, 2 warnings, 5 notes'
summary: The code held in every window of the floor read and for every late answer. Two rules of round 3 were measured by no committed case, and the reviewer's three spec files with 14 cases are committed with the unit. The daemon-side guard for a higher floor is written on the daemon API page.
relations:
- reviews: story:daemon-page-starts-a-run
revision: 1
---
verdict: NEEDS-CHANGE

unit: `story:daemon-page-starts-a-run` at `029b2690`, tree `<trees>/b10x/runbook/page-start`, branch `impl/daemon-page-starts-a-run`

cases: executed 56 to 70 (Playwright, project wide), red 0; Rust `daemon::`, `ui_embed` and `ui_guard` 210. Rust: `Summary [ 29.565s] 210 tests run: 210 passed, 552 skipped`, exit 0, load 67.84 64.47 67.35 before. UI, committed files in groups: `16 passed (52.4s)`, `10 passed (41.2s)`, `14 passed (35.0s)`, `8 passed (1.2m)`, `6 passed (34.3s)`, `2 passed (22.8s)`, at load 70 to 85. The reviewer's three files: `6 passed (43.2s)`, `6 passed (48.2s)`, `2 passed (14.5s)`. The twelve committed files took 302 s of clock in six runs.

origin: introduced 7 / pre-existing 0 / undecided 0

What held: the fixes for F1, F3, F4 and F5 hold on the head. Nothing is sent from `open()` until the floor of that opening was read: not by the button, a button enabled by script, or Enter. A read of an older opening neither enables Start of a newer one nor changes its floor. A failed floor read followed by a successful one offers only `linux` from the failed read on. A late success, 422, 500 or 401 of an earlier opening gives one run per form, and the token is forgotten after a late 401. A 5xx that names any outcome keeps the key. A picked runbook that left the list leaves none selected and Start disabled. The timing commit removed no `expect` line, and three gated cases fail under the two mutants named for it. Token, markup, guard layer, routes, inspector and the changelog tail are unchanged or hold. The code on the head holds; the verdict is NEEDS-CHANGE for the two test gaps.

What did not hold: two rules of fix round 3 are measured by no committed case. With the line that ends the key on a late success of an earlier opening changed, all twelve committed files pass and the same content pressed again starts a second run. With the drops of a read of an older opening removed, all twelve committed files pass. The reviewer's `late` and `ready` files kill both mutants. Notes: a daemon that comes back with a higher floor while the form is open has its floor read already, so a start with the lower level leaves the page and the daemon refuses it with 409 below-floor and starts no run; the form opened again says nothing of an earlier press after a late success; a runbook shown selected without a pick is replaced silently when it leaves the list.

coordinator: the reviewer's three spec files (14 cases) are committed with the unit. The note on a daemon that comes back with a higher floor while the form is open is written on `docs/daemon-api.md` as the daemon's guard: it answers 409 below-floor and starts no run. The notes on the reopened form saying nothing of an earlier run (G5) and on a runbook shown selected without a pick (G6) go to the backlog. G7 is the F2 of pass 3, left unchanged by that round's decision. G4 has no reach found (only a script presses the button of a closed form) and stays as it is. The UI step time goes to the story `ui-test-step-budget`.

```findings
[
 {"file":"ui/daemon/start.js","line":203,"category":"mutant","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"No committed case holds that a late success of an earlier opening keeps the key: with the line changed all twelve committed spec files pass and the same content pressed again starts a second run. The reviewer's ui/test/security-start-4-late.spec.mjs:83 catches it and is committed with the unit."},
 {"file":"ui/daemon/start.js","line":240,"category":"mutant","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"No committed case holds that a read of an older opening is dropped: with both drops removed all twelve committed spec files pass and the older read enables Start of the newer opening. The reviewer's ui/test/security-start-4-ready.spec.mjs:130 catches it and is committed with the unit."},
 {"file":"ui/daemon/start.js","line":161,"category":"concurrency","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"A start must carry no level below the floor the daemon has when it is sent. After the floor of the opening was read, a daemon that came back higher refuses a start with the lower level, 409 below-floor, with no run. No page check can close this, and a second floor read at the press would only narrow it. Written on docs/daemon-api.md as the daemon's guard."},
 {"file":"ui/daemon/start.js","line":233,"category":"boundary","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"The ready state must end when the form closes; it stays true until the next opening. Nothing reaches it: the button of a closed form is not visible, and only a script presses it."},
 {"file":"ui/daemon/start.js","line":213,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The form opened again must say that the earlier press started a run; after a late success it says nothing and the run is named only in the list under the dialog. Backlog."},
 {"file":"ui/daemon/start.js","line":120,"category":"concurrency","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"A runbook the form showed as selected without a pick must not be replaced silently by the next first one when it leaves the list; only a pick made by a change event is followed. Backlog."},
 {"file":"ui/daemon/start.js","line":198,"category":"boundary","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"An answer that names an outcome this command does not have, or a success without a run, must not end the key. This daemon sends none of these. This is F2 of pass 3, unchanged by decision."}
]
```
