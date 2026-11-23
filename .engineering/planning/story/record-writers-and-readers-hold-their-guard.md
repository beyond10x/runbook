---
format: aep.planning-md/3
id: story:record-writers-and-readers-hold-their-guard
kind: story
status: active
title: Every record writer of a run directory and every bounded read of the inspector holds its guard
summary: The older writer of the run directory's records creates its temporary file as new and follows no link, as the writer of runner.json does; a read of the inspector takes at most its limit from the checked descriptor.
relations:
- informed_by: story:runner-records-a-step-in-flight
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: docs
- confidence: cited
  path: runbook/src/loom_run/run_dir.rs
- confidence: cited
  path: runbook/src/loom_run/run_files.rs
- confidence: cited
  path: runbook/tests/loom_run
- confidence: cited
  path: ui/runfiles.mjs
- confidence: cited
  path: ui/test
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T19:21:58Z", actor: "human:operator", revision: 9, executor: "agent:runbook"}
- {from: "proposed", to: "active", at: "2026-11-23T19:21:59Z", actor: "human:operator", revision: 10, executor: "agent:runbook"}
---
## Defect

The security review of `story:runner-records-a-step-in-flight` found two guards that the unit holds for
its own record and that older code of the run directory does not hold. Both are older than that unit.

1. The runner writes `runner.json` through one writer that creates its temporary file as new, under an
   open that follows no link and does not wait, and replaces the record by rename. The other records of
   the run directory (`runbook/src/loom_run/run_dir.rs`, `RunDir::put`: the status record, the stop
   record, the records under `loom/` and the store files) go through an older writer that does not.
2. The inspector reads a record of a run with a size bound (`ui/runfiles.mjs`, `readText`). The bound is
   checked before the read and not held by the read itself.

## Outcome

1. Every record writer of the run directory creates its temporary file as new, under its own open that
   follows no link and does not wait, and replaces the record by rename. No record writer opens a name
   that a program of the run can place before it.
2. A writer that finds its temporary names taken ends the run with an error that names a path inside
   the run, within a bound, and writes nothing outside the run.
3. A read of a run's file by the inspector takes at most the limit plus one byte from the one checked
   descriptor, whatever the file's size is at the time of the read.
4. `docs/` says which writer writes the records of a run directory and what a reader's bound holds.

## Acceptance

The review's three cases, kept outside the repository until this unit, are committed and go from red to
green: two on the status record's writer (a run ends within its bound and writes nothing outside the
run) and one on the reader's bound. The committed cases of the `runner.json` writer stay green,
unchanged. A security review of this unit's diff is green before it merges.

## Origin

Findings S11 and S12 of the second security review of `story:runner-records-a-step-in-flight`,
2026-11-23. The operator's host session picked: that unit merges, and these two are built next as this
story, accepted, with a security review of their own.
