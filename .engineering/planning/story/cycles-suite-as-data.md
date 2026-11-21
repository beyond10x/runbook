---
format: aep.planning-md/3
id: story:cycles-suite-as-data
kind: story
status: active
title: Runbook's finding cycles as data in cycles/suite.yaml
summary: 'Step 5 of the move onto Loom: cycles/suite.yaml (runbook.cycles/1) lists every finding cycle with its stub, switches, expected verdict and what is not yet expressible; a check holds it equal to the script.'
tags:
- conductor-succession
- loom
relations:
- decomposes: epic:conductor-succession
- informed_by: story:loom-bindings-files
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: cycles/suite.yaml
- confidence: cited
  path: docs/cycles-suite.md
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T22:50:43Z", actor: "human:timo", revision: 2, executor: "agent:wave-rb1-cycles-suite", correlation: "operator-standing-acceptance-2026-11-21"}
- {from: "proposed", to: "active", at: "2026-11-21T22:50:43Z", actor: "human:timo", revision: 3, executor: "agent:wave-rb1-cycles-suite", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: cycles-suite-as-data

## Outcome

The finding cycles of `scripts/run-finding-cycles.sh` (each: a stub setup, a run, an expected verdict) exist as data in `cycles/suite.yaml`, format `runbook.cycles/1`: one entry per cycle with its protocol, stub, switches, arguments, expected verdict, state (`expected` or `pending`) and source lines in the script. Every assertion the script cannot say declaratively is listed under the cycle's `unexpressed`, never dropped. This is step 5 of the move of Runbook onto Loom; the script stays and keeps running the cycles until Loom's `tree test` reads this file.

## Context

Loom has no suite format yet (wave/2026-11-21-w7 has no `tree test`), so `runbook.cycles/1` is defined here and the distinct `unexpressed` kinds are the input for Loom's format.

## Acceptance

- `cycles/suite.yaml` has one entry per name in the script's `expected` and `pending` arrays, no duplicate ids.
- `task check:cycles-suite` fails when the script and the suite disagree on the set of ids or on a state, and is part of `task check`.
- `docs/cycles-suite.md` describes the file; `aep plan artifact validate` passes.
