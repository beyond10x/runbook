---
format: aep.planning-md/3
id: story:install-build-commit-moves-only-with-head
kind: story
status: draft
title: The build compiles the crate again only when the commit of HEAD changes
summary: Operations that rewrite the log of HEAD without moving it make the build compile the crate again. From adversary pass 3 of the install unit, left open by fix round 3.
relations:
- informed_by: story:install-script-lays-out-a-prefix
revision: 1
---
## Outcome

The binary's record of the commit it was built from changes, and the crate is compiled again, only when the commit of HEAD changes. An operation that rewrites the log of HEAD and leaves HEAD where it is compiles nothing.

## Source

Adversary pass 3 of `story:install-script-lays-out-a-prefix` measured that several git operations that leave HEAD on the same commit rewrite the log of HEAD, and that the build, which watches that log (`runbook/build_commit.rs:85`), compiles the crate again after each. A release install then waits for a build that changes nothing. Fix round 3 left it open.

## Scope

- `runbook/build_commit.rs` (what the build watches).
- `runbook/tests/build_commit/`: one case per operation class, run with real cargo.

## Acceptance

- After an operation that leaves HEAD on the same commit, cargo's own output has no `Compiling runbook` line. After a change of the commit, it has one, and the binary prints the new commit. Cases in `runbook/tests/build_commit/` show both.
- The guards of the unit keep holding: a build of another commit than HEAD is refused by the install script.

## Origin

Introduced by the install unit (`runbook/build_commit.rs`). Held by the review record of its adversary pass 3 (finding N8).
