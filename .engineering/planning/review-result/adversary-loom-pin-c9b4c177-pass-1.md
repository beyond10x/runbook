---
format: aep.planning-md/3
id: review-result:adversary-loom-pin-c9b4c177-pass-1
kind: review-result
status: active
title: 'Adversary pass 1 of the unit loom-pin-c9b4c177 at ae9f7f0e: green, 3 warnings fixed in round 2, 5 notes'
summary: All seven claims held; the not-called fix survives four neighbour cases. Three warnings the unit introduced (a surviving mutant, an unobserved sentence of the suite page, two refusals of check-bindings) are fixed in round 2. The acceptance line on the suite entries is not observed at the load of this host.
relations:
- reviews: story:loom-pin-c9b4c177
revision: 1
---
verdict: approve

unit: `story:loom-pin-c9b4c177` at `ae9f7f0e`, tree `<trees>/b10x/runbook/pin-nc`, branch `impl/loom-pin-c9b4c177`, scope `git diff 3c0eb863 ae9f7f0e`

cases: 1 driver case and 4 runbook cases of the adversary, all passing on the real code; the runbook suite `28 passed` (86.7 s at a load average of 84), the driver baseline `197 passed` (17.6 s); four mutants of the workspace rule, three killed by the unit's tests and one by the adversary's case.

origin: introduced 3 warnings and 4 notes / pre-existing 1 note

What held: all seven claims. The not-called fix of Loom `c9b4c177` arrives (a writer is started once while a finding waits) and survives four neighbours: an exit 79 the binding does not list, a program not called on every evaluation, two writers not called at once, a writer that writes and then exits 79. The workspace rule reads the directory where Loom started the program. The pin names one git rev for the three Loom crates, with no patch and no path. `--cycle-timeout` and `--list` behave as the pages say. No suite entry lost `pending`; one gained it (`spec-person-wait`, a new cycle). Nothing of `main` was lost in the merge.

Not attacked: a not-called exit after a rejection.

coordinator: the three warnings went back to the implementor and are fixed in round 2 (`2ed89054`, `85fbb2b2`, `ace9c6d6`): the adversary's cases are committed under names that say what they hold; `scripts/check-bindings.sh` refuses an unknown `exits` class and a class that is not a list by a named line, with a test of three cases that was red before; `docs/cycles-suite.md` says only what was observed. The acceptance line "no entry changes from expected to another state" is NOT observed: at the load of this host (76 to 100) seven of the eight `change.review` and request entries end on Loom's 60 s bound while still progressing. It is observed by one run of the suite at a load where a cycle ends inside the bound; the story is not moved to implemented before that run.

```findings
[
 {"file":"driver/src/reply/records.rs","category":"test-gap","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"A mutant that counts an empty LOOM_CASE as set survived the unit's 216 driver tests. Fixed in round 2: driver/tests/workspace_empty_loom_case.rs kills it."},
 {"file":"docs/cycles-suite.md","line":114,"category":"acceptance","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"The page said the change.review and request entries are shown as expected at a quiet host; no such run was made, and three of them ended CycleTimedOut at 60.4 to 60.9 s at a load of 76 to 88 while still progressing. Fixed in round 2: the page lists each run with result, time and load, and says the line is not observed on this host."},
 {"file":"scripts/check-bindings.sh","line":124,"category":"validation","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"An exits class Loom does not read was not refused, and a class given as a number stopped on a raw jq error. Fixed in round 2: both are refused by a named FAIL line (driver/tests/check_bindings_exits.rs, 3 cases, red before)."},
 {"file":"driver/src/reply/records.rs","category":"dead-branch","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The <case_dir>/work branch of case_work has no bound flow on main 0.7.0; only a hand-run sdc-reply and driver tests reach it. A later unit may remove it."},
 {"file":"runbook/src/test_suite.rs","category":"coverage","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"DifferenceReason::CycleTimedOut is matched nowhere in the runner; the printed line is Loom's and is distinguishable."},
 {"file":"driver/src/reply/coverage.rs","category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"sdc-reply coverage --work reads the directory it is given whatever LOOM_CASE holds; the workspace rule holds for case_work only."},
 {"file":"docs/cli.md","line":156,"category":"docs","severity":"note","verdict":"NEEDS-CHANGE","origin":"introduced","message":"The page said a --cycle-timeout outside 1 to 60 is refused in Loom's words; the message is the runner's own (exit 2), and a word or an empty value is refused by the option parser (exit 64). Fixed in round 2."},
 {"file":"runbook/tests/test_suite","category":"timing","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The runbook test step of the unit (28 tests) took 86.7 s at a load average of about 84, against the budget of 120 s."}
]
```
