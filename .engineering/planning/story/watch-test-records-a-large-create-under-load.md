---
format: aep.planning-md/3
id: story:watch-test-records-a-large-create-under-load
kind: story
status: draft
title: The watch test of a large file records its create on a loaded host
revision: 1
---
## Outcome

`watch/tests/watch.rs` `binary_and_too_large_have_no_diff` records both creates on every run, also on a loaded host.

## Found

2026-11-25 about 17:05 CET, `task check` of the run-tree branch (which does not touch `watch/`), in the workstation
pool: `tests/watch.rs:248` failed with left `[("create", "img.bin")]`, right
`[("create", "big.txt"), ("create", "img.bin")]`. The watcher stopped with "1 changes recorded this session": the
2 MiB write of `big.txt` was not recorded before `r.pause()` ended and `r.stop()` ran. The same test passed in the two
runs before on the same head.

## Acceptance

- The test waits on the event it asserts (the change record of `big.txt`), not on a fixed pause, with a bound under
  60 s.
- A stress run of the test (`--stress-count 50`, judged by its Summary line) passes on a loaded pool.
- No product code changes unless a deterministic red-first test shows the watcher itself loses the event.
