---
format: aep.planning-md/3
id: story:bugfix-keeps-a-checks-build-directory-between-steps
kind: story
status: draft
title: A check of the bug-fix runbook keeps its build directory between the steps of a run
summary: Each step starts from a fresh copy, so a build tool that builds inside the tree starts cold (398 s against 50 s measured by a consumer); the checks file names a variable and sdc-bugfix points it at a directory beside the copy, as it does for CARGO_TARGET_DIR.
relations:
- decomposes: epic:ticket-to-merge-request-on-a-real-repository
- informed_by: story:bugfix-any-language
scope:
- confidence: cited
  path: docs/bugfix.md
- confidence: cited
  path: driver/src/bin/sdc-bugfix.rs
revision: 3
---
## Defect

Every step of the bug-fix runbook that runs a check starts from a fresh copy of the code tree
(`driver/src/bin/sdc-bugfix.rs`, `copy_to` resets the copy). A build tool that keeps its build directory
inside the tree therefore starts cold in every step. A consumer measured a cold build of 398 s against
a warm one of 50 s for the same check, so two steps of one run lost about 2 x 348 s. For Rust the
driver already keeps the build output between steps: it sets `CARGO_TARGET_DIR` to a directory beside
the copy.

## Outcome

1. The checks file can name a build directory for its check commands: an environment variable the
   check tool reads (one or more names). `sdc-bugfix` sets each to a directory beside the copy, creates
   it, and keeps it between the steps of one run, as it does for `CARGO_TARGET_DIR`.
2. The directory is the run's own: it lies under the run's work directory beside the copy, never
   inside the copy the writer edits, never shared between two runs, and it is not part of the change the
   run hands back.
3. The step's record says whether the directory existed when the step began (warm or cold).
4. `docs/bugfix.md` says what a check tool must do to use it (build into the named directory, not into
   the tree) and what is not kept (anything written inside the copy).

## Guards

The value is a name of a variable, never a path: a checks file cannot point a build directory outside
the run. The directory is created as new by `sdc-bugfix` or is a directory it created in an earlier
step of the same run (not a link, owned by the user). The verdict rules do not change: a kept build
directory is what `CARGO_TARGET_DIR` already is for Rust.

## Acceptance

With a stand-in check tool that writes a marker file into the directory its variable names: the second
step of a run finds the marker; a second run does not; the copy holds no file of the directory; a
checks file whose member is a path or names `PATH`, `HOME` or a `SLOT_` variable is refused at seed by
the member's name; the record of each step says warm or cold.

## Origin

A consumer's review of a bug-fix run of 2026-11-23 (durations and step names only), relayed on the
operator's host session's order. The consumer's own check script keeps its work directory inside the
copy; that side is the consumer's.
