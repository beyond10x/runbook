---
format: aep.planning-md/3
id: story:the-race-specs-swap-without-mv-exchange
kind: story
status: draft
title: The two race specs swap a directory without mv --exchange and pass on a build node
tags:
- backlog
revision: 1
---
## Context

Two UI cases race a reader against a directory swap made by a child process:
`ui/test/security-products-race.spec.mjs:27` and `ui/test/security-products-2-race.spec.mjs:27`. The child loops
`mv --exchange -T d lnk` (`:60` and `:59`). The race part is bounded by what it saw (each state served or refused 100
times), not by a time.

On a build node of the shared pool (glibc 2.34) both cases ended at their 30 s timeout on 2026-11-27, and the UI step
stopped at its 120 s budget with 34 cases not run. On the main host (coreutils 9.12) they pass.

Inferred cause, not verified on the node: `mv --exchange` exists from GNU coreutils 9.5 on. A node with an older `mv`
refuses the flag in every turn of the loop, no swap happens, the case never sees the swapped state 100 times and runs
until its timeout.

## Acceptance

1. Both cases pass on a host whose `mv` has no `--exchange`, and still fail when the reader follows a swapped link (a
   mutant of the reader's check goes red).
2. The swap no longer depends on the coreutils version: the child makes the exchange through a call the test controls
   (for example `renameat2` with `RENAME_EXCHANGE` from a small program the test builds or a Node helper), or the case
   checks the tool once and fails at once with a line naming the missing tool, never by a timeout.
3. The race part ends within 5 s on the main host and on a build node.

## Evidence

- A gate probe of the shared pool on a build node, 2026-11-27: UI step 648 passed, 2 failed (these two, "Test timeout of
  30000ms exceeded"), 34 not run at the 120 s budget; `task check` exit 201 after 306 s.
- The two cases' source: the child process line `while :; do mv --exchange -T d lnk; done`.
