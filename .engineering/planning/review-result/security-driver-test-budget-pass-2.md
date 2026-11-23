---
format: aep.planning-md/3
id: review-result:security-driver-test-budget-pass-2
kind: review-result
status: active
title: 'Security pass 2 of the unit driver-test-budget (second part) at b67a0c69: red, 0 blockers, 3 warnings, 4 notes'
summary: The pin held and nothing is written below home. The capture change moved the cause of a late-exiting product to a missing capture, a non-UTF-8 line hid a value, and the step took 124.08 s at load 110 to 114, over 120 s.
relations:
- reviews: story:driver-test-budget
revision: 1
---
verdict: NEEDS-CHANGE

unit: `story:driver-test-budget` at `b67a0c69`, tree `<trees>/b10x/runbook/drv-budget`, branch `impl/driver-test-budget-2`

cases: executed 620 to 623 (whole step), red 3. Measured step on the unchanged tree: `Summary [ 124.080s] 620 tests run: 620 passed, 0 skipped`, exit 0, load 110 to 114. With the reviewer's cases: `Summary [ 142.834s] 623 tests run: 620 passed, 3 failed, 0 skipped`, exit 100, load 112 to 118 (a mutant build ran beside it, so not a timing). Empty `HOME` and `TMPDIR` for every test process, `--test main`: `Summary [ 133.543s] 447 tests run: 447 passed, 0 skipped`, load 113 to 120. A binary that is not the pin as `ESS_BIN`, seven modules: `Summary [ 6.356s] 93 tests run: 56 passed, 37 failed, 354 skipped`, load 116 to 120. The step took 124.08 s at load 110 to 114 against the 120 s of its check.

origin: introduced 4 / pre-existing 1 / undecided 1

What held: no binary whose bytes differ from the pin is accepted at this head; the product diff against the base touches only the adapter and the stub example, and no digest cache or its name is left in a built binary. The 26 tests that need the host's `ess` all fail under a stand-in with other bytes. The pass-1 cases were kept with their assertions, one strengthened, one dropped because it cannot fail on this code. Nothing is written below the home directory. The wait after the streams ended covers the natural gap between the end of stdout and the child being waitable many times over, and no probe left a zombie or a surviving process.

What did not hold: the capture change moves the recorded cause of a product that ends more than 250 ms after its streams ended from not started to a missing capture, and the table is retried (warning). A reader that ends at the first line that is not UTF-8 misses a value printed after it, and the message then says the product closed stdout without printing it. A doc comment and a changelog entry say more than the code does. One test passes without the build step having run when the host's `ess` is not the pin. The step ran 124.08 s at load 110 to 114, over the 120 s of its check and of the story's acceptance; its cost is mostly the digest at every start, the 100 ms poll quanta and the fixed wait.

coordinator: all seven findings went to the implementor in fix round 2. The reviewer's three cases were committed first, red, as a module named for what it holds; the fixed wait is removed and the label is decided by what the product does next within the same timeout as the base; the reader reads bytes and sets the closed flag only at the end of the stream; the doc comment and the changelog entry are corrected; the build-step test uses a marker file with a control; the budget was taken in this round with the target of at most 120 s at the present load (poll quanta, then a streamed digest that still covers every byte, no cache). Pass 3 found the label, the marker test and the step budget held.

```findings
[
 {"file":"driver/src/bin/sdc-conform-adapter.rs","line":280,"category":"contract-drift","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"A product whose capture stream ended and that exits more than 250 ms later is reported as a missing capture (event capture, cause capture_missing, table retried) where the base reported product_not_started for any exit within timeout_s."},
 {"file":"driver/src/bin/sdc-conform-adapter.rs","line":107,"category":"boundary","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"The stream reader ends at the first line that is not UTF-8, so a capture value printed after such a line on an open stream is never read."},
 {"file":"driver/src/bin/sdc-conform-adapter.rs","line":118,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The closed flag is set when the reader ends for any reason, so a product that printed a non-UTF-8 line and then the value is said to have closed stdout without printing it."},
 {"file":"driver/src/bin/sdc-conform-adapter.rs","line":47,"category":"contract-drift","severity":"note","verdict":"NEEDS-CHANGE","origin":"introduced","message":"The doc comment of EXIT_AFTER_STREAMS says both streams ended, while the wait starts when the one stream of the capture ended."},
 {"file":"driver/tests/ess_pin_guard.rs","line":317,"category":"mutant","severity":"note","verdict":"NEEDS-CHANGE","origin":"introduced","message":"The check that the build step did not run compares the file's digest with the pin, so the test passes without the build step having run when the host's ess is not the pin."},
 {"file":"Taskfile.yml","line":109,"category":"acceptance","severity":"warning","verdict":"CONFIRMED","origin":"undecided","message":"The driver test step took 124.080 s for 620 tests at load 110 to 114, over the 120 s its own check and the story's acceptance name."},
 {"file":"CHANGELOG.md","line":20,"category":"contract-drift","severity":"note","verdict":"NEEDS-CHANGE","origin":"introduced","message":"The entry says the tests resolve the pinned ess once per test and that every start of sdc-conform hashes it, but tests with several observations take one digest per observation and check-table takes none."}
]
```
