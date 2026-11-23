---
format: aep.planning-md/3
id: review-result:security-driver-test-budget-pass-3
kind: review-result
status: active
title: 'Security pass 3 of the unit driver-test-budget (second part) at 6a5ebf57: red, 0 blockers, 5 warnings, 5 notes'
summary: The pin, the label for the named rows, the reader and the step budget (102.715 s) held. A wrong capture sentence for a dropped value and an unchecked kill came from the unit; three defects exist on main and are backlog.
relations:
- reviews: story:driver-test-budget
revision: 1
---
verdict: NEEDS-CHANGE

unit: `story:driver-test-budget` at `6a5ebf57`, tree `<trees>/b10x/runbook/drv-budget`, branch `impl/driver-test-budget-2`

cases: executed 633 to 639, red 3. Unchanged tree, `slot task test:driver`: exit 0, `Summary [ 102.715s] 633 tests run: 633 passed, 0 skipped`, "test run 102.715s of 120s", load 101.45 at start and 101.36 at end. With the reviewer's cases and `HOME` and `TMPDIR` pointed at empty scratch directories: exit 100, `Summary [ 112.506s] 639 tests run: 636 passed, 3 failed, 0 skipped`, load 84.26 at start and 111.55 at end; the three failures are the reviewer's red cases. Slowest test 14.220 s.

origin: introduced 6 / pre-existing 3 / undecided 1

What held: the pin (the digest is streamed, fails closed on a read error and has no stand-in; no non-pin `ess` was accepted in any run). The capture label for every row the brief names. The reader for a NUL, a lone CR, a value split across two writes, a bad byte inside the value and a stream ending without a newline. The marker of the build-step test lies in the test's own `target/tmp` and the digest comparison is gone. The step budget, 102.715 s at load 101, slowest test 14.2 s. Nothing else changed: no entry below the tests' `HOME`, the shared `ess` unchanged, `CHANGELOG.md` from `## [0.7.0]` on byte-equal to `main`, the website untouched.

What did not hold: the bound on a build step is broken, because the bounded command ends only its direct child (P1, on the base too). The capture buffer stops taking lines at its bound and a later value is reported missing (P2, on the base too), and the fix round introduced only the wording for it: the adapter says the product closed stdout without printing a value it did print (P3). Nothing bounds the length of one output line (P4, on the base too). The unit test of the bound did not check that the child is gone (P5). Notes: the upper bounds of two bound tests let a doubled deadline pass (P6); no test of the digest produces a read error or a short read (P7); neither a test nor the step time shows a poll that never sleeps (P8); the new unit tests write under the temp directory, which on this host lies below home (P9); the digest is taken once per observation and the binary is run again by path after the build step (P10, by reading).

coordinator: P3 and P5 went back to the implementor and are fixed in round 3; P6 went back to be tightened only where a doubled deadline then fails and a margin of 3 times holds; P7 is held by two committed cases; P8 and P9 are left. P1, P2, P4 and P10 exist on `main` without the unit and are filed as the backlog stories `story:bounded-command-leaves-no-process`, `story:conformance-capture-reads-all-output-within-a-bound` and `story:ess-pin-holds-for-every-start-of-an-observation` (decision of the operator's delegate). The reviewer's two red cases for P1 and P2 are kept outside the tree for those stories. Pass 4 confirmed round 3.

```findings
[
 {"file":"driver/src/conform.rs","line":210,"category":"process-bound","severity":"warning","verdict":"NEEDS-CHANGE","origin":"pre-existing","message":"Nothing a bounded command started may outlive the bound; a case shows a process that does. Guard to hold: when the bound ends a command, every process it started is gone. Backlog: story:bounded-command-leaves-no-process."},
 {"file":"driver/src/bin/sdc-conform-adapter.rs","line":123,"category":"capture-label","severity":"warning","verdict":"NEEDS-CHANGE","origin":"pre-existing","message":"The capture buffer stops taking lines at its bound, and a value the product printed after the buffer was full is reported missing; a case shows it. Guard to hold: a value the product printed is read wherever it appears in the output, within a stated bound. Backlog: story:conformance-capture-reads-all-output-within-a-bound."},
 {"file":"driver/src/bin/sdc-conform-adapter.rs","line":309,"category":"capture-label","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"For a product that printed the value past the buffer's bound and closed stdout, the adapter answered that the product closed stdout without printing it, where the base gave the answer with its time limit. The sentence must not be said of a value the reader dropped. Fixed in round 3."},
 {"file":"driver/src/bin/sdc-conform-adapter.rs","line":110,"category":"resource-bound","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"Nothing bounds the length of one output line, so the memory of the adapter follows the longest line a product prints; the head peaks above the base. Guard to hold: the memory of the reader is bounded whatever one line holds. Backlog: story:conformance-capture-reads-all-output-within-a-bound."},
 {"file":"driver/src/conform.rs","line":1330,"category":"test-gap","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"The unit test of the bound reads the answer and the time but not whether the child is gone: with the kill and the wait removed from run_bounded, all 28 pin, capture and bound tests pass. Held after round 3 by a committed case that checks the child is gone."},
 {"file":"driver/src/conform.rs","line":1334,"category":"test-gap","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The upper bounds of the two bound tests (10 s here, 8 s at driver/tests/capture_fate.rs:166, for a 1 s limit) let a doubled deadline pass; both tests measured 1.04 to 1.10 s at load 100."},
 {"file":"driver/src/conformance.rs","line":42,"category":"test-gap","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"No test of the unit produced a read error or a short read in sha256_file, so a mutant returning the digest of a prefix and a mutant ending at a short read both passed. Held after round 3 by two committed cases."},
 {"file":"driver/src/conform.rs","line":215,"category":"test-gap","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"A poll that never sleeps fails no test, and the step time (111.05 s against 86.30 s at loads of 86 to 106) does not separate it from load."},
 {"file":"driver/src/conform.rs","line":1301,"category":"test-hygiene","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The new unit tests write up to 3 MiB under std::env::temp_dir() and remove it; where TMPDIR lies below the home directory, as for sessions on this host, that is below home."},
 {"file":"driver/src/conform.rs","line":756,"category":"pin","severity":"note","verdict":"INFEASIBLE","origin":"undecided","message":"By reading, not measured: the binary run after the build step must be the binary whose digest was checked, and every start of an observation must check the pin. Confinement is named as a limit in docs/conformance.md. Backlog: story:ess-pin-holds-for-every-start-of-an-observation."}
]
```
