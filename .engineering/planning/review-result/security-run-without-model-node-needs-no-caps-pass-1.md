---
format: aep.planning-md/3
id: review-result:security-run-without-model-node-needs-no-caps-pass-1
kind: review-result
status: active
title: 'Security pass 1 of the unit run-without-model-node-needs-no-caps at ffb33de7: red, 1 blocker, 1 warning, 6 notes'
summary: The check before a Loom call read the run's bindings copy but not the file the tree manifest names, so a model program could start before the refusal. Six mutants of documented guards survived the unit's cases. The decision towards a model node and the records held.
relations:
- reviews: story:run-without-model-node-needs-no-caps
revision: 1
---
verdict: NEEDS-CHANGE

unit: `story:run-without-model-node-needs-no-caps` at `ffb33de7`, tree `<trees>/b10x/runbook/zero-caps`, branch `impl/run-without-model-node-needs-no-caps`

cases: executed 200 to 210, red 3, and one existing case red once on its wall-time bound. Run 1: `Summary [  52.423s] 210 tests run: 207 passed, 3 failed, 538 skipped`, exit 100; the three are the reviewer's red cases and all 200 cases of the unit's filters passed. Run 2: `Summary [  54.426s] 210 tests run: 206 passed, 4 failed, 538 skipped`, exit 100; the fourth is the existing case on its wall-time bound. The real-Loom cases take 5.7 to 16.5 s each at host load 90 to 110.

origin: introduced 5 / pre-existing 2 / undecided 1

What held: every one of 17 further forms of the bindings copy counts as a model node and refuses a missing ledger before any run directory is made. A ledger of other runs is not opened, changed or locked. A recorded call in a run without a model node ends the run with exit 13, states its cost, and records `has_model_node` false. The decision is taken once, in memory only. With a model node nothing is weaker: the ledger, budget, decider and core cases are unchanged from the base and pass. A rewrite of the run's bindings copy by a bound program is refused before the program starts, with Loom as a library. The refusals carry `model_call_without_model_node` and exit 13 collides with no documented code. The mutants that widen the copy kind, remove the pre-check at line 103 and stop the recursion into directories are killed by committed cases. The mutant that removes the refusal at line 403 is killed by committed cases only through the 60 s kill, not by an assertion. Six mutants named in the findings survive the unit's cases.

What did not hold: the check before a Loom call read the run's bindings copy, but Loom reads the bindings file that the tree manifest names, so a manifest changed by an earlier step started a model program and the run ended exit 13 only after the step (blocker). Six mutants of documented guards survived the unit's cases (warning). The page said a model program reached under a name the scan does not read is covered by no cap with or without the rule, which is wrong for a model tool reached through a wrapper that writes cost records. The link to the login file was made also in a run without a model node. The suite runner and the daemon's Loom runtime do not pass through the rule. One existing wall-time assertion failed once.

coordinator: fix round 1 takes F1, F6, the page sentences of F2, F3 and F7, and the cases of F4. The line the unit must hold, from the operator's delegate: a model call attempted in a run that has no model node is refused, with a test that proves it. F1: before every Loom call that can start a program in such a run, the check covers the bindings file Loom will read, and the manifest's `bindings` member must still name the root file of the copy. F6: no login link is made in a run without a model node. F2: the limit (a bound program that starts a model program under a name the scan does not read) is written on `docs/loom-run.md`, each sentence held by a case; F3 corrects the sentence on wrappers; F7 gets one sentence on where the rule applies. The reviewer's cases are committed first. F5 is not closable here and is not written; it goes to Loom, which took it as a story to bind the bindings to the bytes checked. F8 is not in the unit.

```findings
[
 {"file":"runbook/src/loom_run/run_loop.rs","line":421,"category":"acceptance","severity":"blocker","verdict":"NEEDS-CHANGE","origin":"introduced","message":"The check before a Loom call in a run opened without a model node read the bindings copy but not the bindings file that the tree manifest names. A guard must hold that, before every Loom call that can start a program, the manifest's bindings member still names the root file of the copy and the scan covers that file. Fixed in round 1."},
 {"file":"docs/loom-run.md","line":334,"category":"boundary","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"A model program reached by a name the scan does not read starts in a run with caps of 0 and no ledger. A recorded call ends the run after the step with nothing booked; an unrecorded call ends the run with exit 0. No committed binding was found that does this. Written as a limit on the page, each sentence held by a case."},
 {"file":"docs/loom-run.md","line":335,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The sentence that such a program is covered by no cap with or without the rule was wrong for a model tool reached through a wrapper: with a model node its recorded cost is reserved for and booked, without one it is booked nowhere. Corrected in round 1."},
 {"file":"runbook/tests/loom_run/no_model_node.rs","category":"mutant","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"Six mutants of documented guards survived the unit's cases: an unreadable file, the yml extension, the link check, mapping keys, tagged text and unreadable cost records. Two green cases of the reviewer kill all six; they are committed in round 1."},
 {"file":"runbook/src/loom_run/run_loop.rs","line":403,"category":"concurrency","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"The runner reads the bindings copy and Loom reads it again afterwards. A guard must hold that the bytes Loom runs are the bytes the runner checked; this cannot be closed in the runner. Not measured. Taken by Loom as a story."},
 {"file":"runbook/src/loom_run/run_loop.rs","line":112,"category":"boundary","severity":"note","verdict":"INFEASIBLE","origin":"pre-existing","message":"The private link to the login file was made before the decision, so a run without a model node gave its bound programs a path to it. A guard must hold that no login link exists in a run without a model node. Taken in round 1 because it serves the line."},
 {"file":"runbook/src/test_suite.rs","category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"The suite runner and the daemon's Loom runtime do not go through the run loop, so the decision and its refusals do not apply there; they had no caps on the base either. The page gets one sentence on where the rule applies."},
 {"file":"runbook/tests/loom_run/inproc.rs","line":462,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"The existing 20 s wall-time assertion failed once at 20.05 s under host load near 100 and passed at 9.2 s in the other run of the same code. The assertion is at the same line on the base, not run there; the report left the origin undecided. Not in this unit."}
]
```
