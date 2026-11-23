---
format: aep.planning-md/3
id: review-result:security-driver-test-budget-pass-1
kind: review-result
status: active
title: 'Security pass 1 of the unit driver-test-budget (second part) at 2644b4c0: red, 1 blocker, 3 warnings, 1 note'
summary: The digest cache in front of the pin check accepted a binary that is not the pin, which a build step could reach; the digest cache was removed. Capture label, a FIFO hang and test writes below home also failed. Mutation and schema guards held.
relations:
- reviews: story:driver-test-budget
revision: 1
---
verdict: NEEDS-CHANGE

unit: `story:driver-test-budget` at `2644b4c0`, tree `<trees>/b10x/runbook/drv-budget`, branch `impl/driver-test-budget-2`

cases: `Starting 446 tests across 1 binary`, `Summary [ 113.906s] 446 tests run: 440 passed, 6 failed, 0 skipped`; executed 436 to 446, red 5. The five capture cases that hold the invariants of the capture change ran `5 tests run: 5 passed`. The step ran 113.9 s at load about 145; the implementor measured 68.4 s at load 70.

origin: introduced 6 / pre-existing 0 / undecided 0

What held: no test was weakened (mutants of the decision request shape, the start-command word check and the closed-stream branch each turn a test red). The committed schema that the shape tests read is guarded by `ess generate --kind schema --check`, so reading the committed file instead of running the generator per test loses no guard. The counting tests hold under cargo-nextest, and the schema cache is on disk under `target/`, not an in-process global. An honest swap, rename, hard link, symlink retarget and relative against absolute path all recompute the digest and are refused. The shared `ess` binary was untouched.

What did not hold: the digest cache in front of the pin check accepted a binary whose bytes differ from the pin, and a model-written build step of an observed product can reach it, so the pin no longer resisted the adversary it exists for (blocker). A cache entry that is a FIFO blocked the check with no bound. A product that had exited could be reported as a capture on a closed stream instead of as not started, under load. The test step wrote host state below the home directory. One doc sentence about the ctime was false.

coordinator: all five findings went back to the implementor in fix round 1. The digest cache and its variable and default location are removed from the product code, so the digest of the binary is taken on every call as on `main`; the reviewer's cases are committed first as tests of the unit and hold the guard in the form "no file below the home directory or named by a variable makes another binary the pinned one"; the capture label is decided by the product's exit state after the stream's end is seen; the tests write nothing below the home directory. Pass 2 found the pin held.

```findings
[
 {"file":"driver/src/conformance.rs","line":1051,"category":"integrity","severity":"blocker","verdict":"CONFIRMED","origin":"introduced","message":"binary_sha256 trusted a readable cache entry whose stat stamp matched and skipped the hash, so a binary that is not the pin was accepted; a model-written build step of an observed product shares the home directory and can write the default cache, which the base always re-hashes and refuses. Guard: no file and no variable can stand in for the hash of the binary."},
 {"file":"driver/src/conformance.rs","line":1051,"category":"integrity","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"A cache entry that is a FIFO made the read block with no timeout, against the stated rule that a cache that cannot be read is digested again, and held a build slot in runbook run."},
 {"file":"driver/src/bin/sdc-conform-adapter.rs","line":281,"category":"concurrency","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"The single 20 ms re-check of the child lost a race under load, so a product that had exited was reported as a capture on a closed stdout instead of as not started; the unavailable verdict was unchanged but the detail and the logged event were wrong."},
 {"file":"driver/tests/conformance.rs","line":32,"category":"integrity","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"Tests that did not override the cache location made resolve_ess write host state below the home directory (entries for the stub product and the real ess), against the rule that a test step writes nothing outside the tree and its scratch."},
 {"file":"driver/src/conformance.rs","line":1042,"category":"integrity","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"The doc claim that the ctime moves with every write and cannot be set, so a swapped file never matches, is false for a write through an already-writable shared mapping; no one reaches it for the read-only binary, so it is a documentation-accuracy point only."}
]
```
