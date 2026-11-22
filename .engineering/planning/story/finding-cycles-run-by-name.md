---
format: aep.planning-md/3
id: story:finding-cycles-run-by-name
kind: story
status: active
title: A finding cycle can be run by name
summary: run-finding-cycles.sh --only <name>[,<name>...] runs the named cycles alone, pulls in the cycle whose output one reads, and ends the verdict with how many were selected
tags:
- test-budget
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: docs/cycles-suite.md
- confidence: cited
  path: scripts/run-finding-cycles.sh
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T04:05:25Z", actor: "human:operator", revision: 2, correlation: "operator-standing-acceptance-2026-11-22"}
- {from: "proposed", to: "active", at: "2026-11-22T04:05:25Z", actor: "human:operator", revision: 3, correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: finding-cycles-run-by-name

## Outcome

`scripts/run-finding-cycles.sh <scratch dir> --only <name>[,<name>...]` runs the named finding cycles
and nothing else, and `task cycles:only SCRATCH=<dir> NAMES=<names>` passes the names through. A
cycle that failed for a reason outside the run (on 2026-11-22 one cycle of the release gate failed
because its test step waited in the build pool) can be repeated alone in minutes, not in a full run of
over an hour.

## Context

The script runs every cycle one after another, 74 minutes on a free machine and about 3 hours under
load. There was no way to repeat one. The script is prototype glue that Loom's suite runner replaces;
this is the smallest bridge until then: every cycle's block is gated by `sel`, and a cycle that reads
the output of an earlier one (`needs`) pulls that one in.

## Acceptance

- A selected cycle runs the same code with the same checks as in a full run; a cycle that is not
  selected neither runs nor is counted, shown by a run of two cheap cycles whose output holds only
  those two.
- A name the script does not list ends it with exit 2 and names it before any cycle runs or any
  build starts.
- A selected cycle that reads the output of an earlier cycle runs that cycle too and says so in one
  line (`cycles: also running <name> (needed by <name>: ...)`), never silently skipping it.
- The verdict line of a selected run reads `cycles: selected S of T, M as expected, K failed: ...`
  and is never the line of a full run; it compares the selected cycles only, and the exit code is
  non-zero when one of them failed.
- `task check:cycles-suite` stays green and `docs/cycles-suite.md` describes the form.
