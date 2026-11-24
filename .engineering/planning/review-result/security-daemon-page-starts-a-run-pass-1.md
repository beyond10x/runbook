---
format: aep.planning-md/3
id: review-result:security-daemon-page-starts-a-run-pass-1
kind: review-result
status: active
title: 'Security pass 1 of the unit daemon-page-starts-a-run at b0092824: red, 1 blocker, 2 warnings, 4 notes'
summary: The idempotency key changed on each press, so an answer lost on the way back could be followed by a second run. A token the browser cannot send locked the form, and the preset level was the weakest admissible one. The token check, the Host and Origin guard, the floor and the markup guard held.
relations:
- reviews: story:daemon-page-starts-a-run
revision: 1
---
verdict: NEEDS-CHANGE

unit: `story:daemon-page-starts-a-run` at `b0092824`, tree `<trees>/b10x/runbook/page-start`, branch `impl/daemon-page-starts-a-run`

cases: executed 207 to 227 (Rust `daemon::` 196 to 203, red 0; UI 11 to 24, red 4), red 4. Rust `daemon::` with the reviewer's cases: `Summary [ 55.215s] 203 tests run: 203 passed, 495 skipped`. UI, committed start and read-only specs: `11 passed (55.0s)`. UI, the reviewer's idempotency and token cases: `4 failed, 4 passed (1.7m)`.

origin: introduced 7 / pre-existing 0 / undecided 0

What held: the token check and the Host and Origin guard on the new view and routes, in one guard layer; the token in page memory only, with the read cookie authorising no command; the floor, so that a start built by hand below it is refused with 409 and writes nothing; nothing typed becomes a principal; content never becomes markup; the view names no host path; the inspector stays read-only; the stub runtime stays the default; no network library and no loosened content policy. Mutants that remove the floor check, the view filter or the token exemption of the view, and mutants B to E, were killed by committed cases.

What did not hold: the idempotency key was new for each press, so a start repeated after an answer that did not arrive could make a second run (blocker). A token the browser could not send as a header locked the form until reload. A token typed and not sent stayed in the field after the form closed. The preset confinement was the floor, the weakest admissible level, while a submit that names none runs at `linux`. The page called the view "the runbooks the daemon can start" while the stub runtime lists entries it cannot start. No committed case held the Host and Origin guard on the new view and on `submit-run`, nor the disabled Start button and the password field with autocomplete off: mutants of both passed the committed suite.

coordinator: fix round 1 takes all seven findings. Decided for F5: no level is preset; the form offers the levels at or above the floor, none selected, and Start stays disabled until one is picked. For F1 the key belongs to the filled form and changes when the daemon answered or the content changed. The reviewer's cases are committed first, as written. Each spec file starts one daemon to keep the step within 120 s. F4 is a change of the document only.

```findings
[
 {"file":"ui/daemon/start.js","line":129,"category":"idempotency","severity":"blocker","verdict":"NEEDS-CHANGE","origin":"introduced","message":"A new idempotency key for each press allowed a second run after an answer lost on the way back. A guard must hold that the key belongs to one filled form and changes only after the daemon answered it or the content of the form changed. docs/daemon-api.md:324 and ui/test/daemon-start.spec.mjs:130 stated a fresh key for each press."},
 {"file":"ui/daemon/start.js","line":114,"category":"boundary","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"A token that cannot be sent as a header kept the form locked, and the message said the daemon could not be reached. A guard must hold that a token that cannot be sent is forgotten, the field is shown again, and the message says so."},
 {"file":"ui/daemon/start.js","line":173,"category":"secret-handling","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"A token typed and not sent stayed in the field #st-token after Close or Escape. A guard must hold that closing the form empties the field."},
 {"file":"docs/daemon-api.md","line":100,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The view runbooks is documented as the runbooks the daemon can start, but under the stub runtime every entry of the runbook directory is listed (runbook/src/commands.rs:801-813) and a start of an entry without a script ends FailedToStart."},
 {"file":"ui/daemon/start.js","line":90,"category":"default","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"The preset confinement was the floor, the weakest admissible level, while a submit that names none gets linux (runbook/src/commands.rs:464). Decided by the coordinator: no level is preset."},
 {"file":"runbook/tests/daemon/runbooks_view.rs","category":"test-gap","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"No committed test sends a foreign Host or Origin to the view runbooks or to submit-run: a mutant that exempts both at runbook/src/api/auth.rs:176 passed the committed daemon tests. The reviewer's module holds it."},
 {"file":"ui/test/daemon-start.spec.mjs","category":"test-gap","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"No committed test held that Start is disabled while a command is on its way (ui/daemon/start.js:96) or that the token field is a password field with autocomplete off (ui/daemon/start.js:166): a mutant of both passed the committed spec. The reviewer's two UI cases hold both."}
]
```
