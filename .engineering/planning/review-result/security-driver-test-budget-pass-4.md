---
format: aep.planning-md/3
id: review-result:security-driver-test-budget-pass-4
kind: review-result
status: active
title: 'Security pass 4 of the unit driver-test-budget (second part) at dbf71410: green, 0 blockers/warnings, 2 notes'
summary: 'The capture sentence, the kill test, the pin and the merge held. Two notes: the buffer bound is not held to the byte by a committed test, and one message is approximate. The step ran 143.498 s at load 98 to 107.'
relations:
- reviews: story:driver-test-budget
revision: 1
---
verdict: approve

unit: `story:driver-test-budget` at `dbf71410`, tree `<trees>/b10x/runbook/drv-budget`, branch `impl/driver-test-budget-2`

cases: executed 637 to 642, red 0. Unchanged tree, `slot task test:driver`: task exit 201, `Summary [ 143.498s] 637 tests run: 637 passed, 0 skipped`, "the test run took 143.498s, over the 120s budget", load 97.66 at start and 107.05 at end. With the reviewer's five cases, `cargo nextest run --locked --color never --no-fail-fast`: exit 0, `Summary [ 104.507s] 642 tests run: 642 passed, 0 skipped`, load 116.33 at start and 100.01 at end, slowest test 13.3 s. The slowest test of the unchanged run was 20.4 s.

origin: introduced 2 / pre-existing 0 / undecided 0

What held: the sentence "the product closed stdout without printing it" is said only of a stream the buffer took whole; a stream of exactly the bound with the value last is captured, a value whose line crosses the bound is captured, and a full buffer followed by the value names the part read and its bound. The bound test of round 3 fails under the reviewer's mutant that removes the kill and the wait. Every committed case fails under its own mutant. The pin holds: the fix round changed no line of the digest or the pin comparison, a non-pin `ess` in a scratch `HOME` is refused with both digests named, and nothing was written below that `HOME`. The merge held: `CHANGELOG.md` from `## [0.7.0]` on is byte-equal to `main`, the website is untouched, nothing of `main` was lost. The fix round changed no behaviour outside P3.

What did not hold: two notes. No committed test holds the capture buffer's bound to the byte or the wording of the new sentence (two mutants pass every committed test and fail only the reviewer's cases). The phrase "the buffer takes the first 1048576 bytes of a stream" is approximate, because a line that starts before the bound is taken whole. The two timeout branches of the adapter still give the base's sentence for a value past the buffer; the report lists no finding for them because they are the backlog behaviour. The step ran over 120 s in this run, carried by the sum and not by one test.

coordinator: the unit merges after its five cases are committed (they hold N1). N2 and the two timeout branches of the adapter go to `story:conformance-capture-reads-all-output-within-a-bound`. The step time of 143.5 s at a load of 98 to 107 is a measurement: the same step took 70.9 s at a load of 86 to 91.

```findings
[
 {"file":"driver/tests/bounds_and_digest.rs","line":205,"category":"test-gap","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"No committed test holds the bound of the capture buffer to the byte or the wording of the new sentence: a buffer that takes one more line at the bound and a buffer that refuses a line crossing the bound both pass every committed test. Held by the five green cases of the reviewer's untracked case file, to be committed."},
 {"file":"driver/src/bin/sdc-conform-adapter.rs","line":324,"category":"capture-message","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The phrase that the buffer takes the first 1048576 bytes of a stream is approximate: a line that starts before the bound is taken whole. By reading, not measured: the count is taken after CR stripping and U+FFFD replacement, and a read error on the pipe gives the same parenthesis though no bound was reached. Backlog: story:conformance-capture-reads-all-output-within-a-bound."}
]
```
