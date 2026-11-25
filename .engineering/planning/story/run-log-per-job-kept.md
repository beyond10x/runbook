---
format: aep.planning-md/3
id: story:run-log-per-job-kept
kind: story
status: draft
title: The run log of each job is kept after the run ends
revision: 1
---
## Outcome

The run log of each job of a run is kept after the run ends, one per job.

## Source

A consumer's list of bug-fix asks (2026-11-25). No file or command was recorded.

## Acceptance

- After a run with several jobs, each job's log is readable under the run's directory, one file per job.
- A log holds no secret value (the hide pass that applies to a run's files applies to it).
