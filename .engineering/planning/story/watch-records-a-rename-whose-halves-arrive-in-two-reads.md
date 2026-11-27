---
format: aep.planning-md/3
id: story:watch-records-a-rename-whose-halves-arrive-in-two-reads
kind: story
status: draft
title: The watcher records a rename whose two inotify halves arrive in two reads
summary: drain drops an unmatched MOVED_FROM at its end, so a rename split over two reads is recorded as a delete and a create; mv_is_a_rename failed so once
tags:
- backlog
revision: 1
---
## Context

The `sdc-watch` case `mv_is_a_rename_and_over_an_existing_file_also_a_delete` (`watch/tests/watch.rs:176-193`)
runs `mv a.txt moved.txt`, pauses 400 ms (`watch.rs:73-75`), runs `mv b.txt c.txt` over an existing file, and
expects `rename moved.txt`, `delete c.txt`, `rename c.txt` (`watch.rs:188`).

The watcher pairs the two halves of a rename by cookie inside one `drain` (`watch/src/watch.rs:308-348`): a
file's `MOVED_FROM` is put in `self.moves` and marked (`watch.rs:388-390`), its `MOVED_TO` takes the source
out of `self.moves` (`watch.rs:391-396`). At the end of every `drain`, `self.moves` is emptied
(`watch.rs:337-343`): for a file the pending half is dropped. inotify(7) says the `IN_MOVED_FROM` and
`IN_MOVED_TO` events of one rename are not guaranteed to be inserted into the queue atomically, and that an
application pairing them should allow a short time for the second half. When a read ends between the two
halves, the `MOVED_TO` arrives in the next `drain` with no source, and the rename is recorded as a delete
and a create.

## Evidence

One `task check` on 2026-11-27 (watch step, load average 17.91 at its start, `Summary [ 6.418s] 52 tests
run: 51 passed, 1 failed`); passed 52 of 52 after:

```
panicked at tests/watch.rs:188:5:
  left: [("delete", "a.txt"), ("create", "moved.txt"), ("delete", "c.txt"), ("rename", "c.txt")]
 right: [("rename", "moved.txt"), ("delete", "c.txt"), ("rename", "c.txt")]
```

The first `mv` was recorded as `delete a.txt` and `create moved.txt`: the record lost the rename, which is a
product defect of the watcher, not of the test. *Inferred*: the two halves were read in two `drain` calls.

## Acceptance

- A deterministic red-first case: the pairing gets the `MOVED_FROM` of a file in one drain and the
  `MOVED_TO` with the same cookie in the next, and the record shows `delete` plus `create` before the change.
- After the change a `MOVED_FROM` whose other half has not come waits a bounded time across drains for it
  (the bound named in the code), and the case records one `rename` with `from_path`; an unmatched half still
  becomes a delete once the bound passes.
- `mv_is_a_rename_and_over_an_existing_file_also_a_delete` passes 50 of 50 runs under load
  (`--stress-count 50`, judged by its Summary line). No pause of the test is lengthened as the fix.
