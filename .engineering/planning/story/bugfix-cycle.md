---
format: aep.planning-md/3
id: story:bugfix-cycle
kind: story
status: proposed
title: 'Bug fix cycle: red test, fix, green, gate, as its own protocol'
summary: 'One bug fix on an existing crate runs through sdc.bugfix in autopilot: dissect, red test, fix, green, story, gate.'
tags:
- conductor-succession
- eval
relations:
- decomposes: epic:conductor-succession
scope:
- confidence: inferred
  path: cases/12-bugfix-red-first
- confidence: inferred
  path: docs/bugfix.md
- confidence: inferred
  path: driver/examples/sdc/fixtures/bugfix-pager
- confidence: inferred
  path: driver/examples/sdc/sdc-bugfix.bindings.json
- confidence: inferred
  path: driver/examples/sdc/tools/llm-node.sh
- confidence: inferred
  path: driver/src/bin/sdc-bugfix.rs
- confidence: inferred
  path: protocol/sdc-bugfix.protocol.yaml
- confidence: inferred
  path: scripts/run-trial.sh
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T10:34:41Z", actor: "human:timo", revision: 3, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: bugfix-cycle

## Outcome

One bug fix on an existing small Rust crate runs through `sdc-drive`, Canon and the autopilot
without a person, as its own small protocol `sdc.bugfix`: the bug report is dissected, a test that
reproduces the bug is written and observed red by a program, the fix is written, the same test is
observed green with the rest of the suite, the bug's story is moved to implemented, the crate's gate
passes, and the case closes by itself.

## Context

The operator, 2026-11-21: if the full cycle from an idea is too much to get through, "start with
something smaller, like implementing the red test for a bug and then fixing it, then seeing the
test green… just take a smaller bit of work to run it through". Then, deciding the shape (D146):
"It's more likely not about creating a vision and a roadmap and epics and stuff… oh, there's a bug,
this is the input, dissect it, write the red test, fix it, update the story, run the gate, done."
And: "Break it down, make it composable. That's the right way."

Scoping of 2026-11-21, why not `sdc.story`:

- A story case can be opened alone (`eval/run-adversaries.sh:97-103`); `gate.passed` is its own
  claim (`protocol/sdc-story.protocol.yaml:150-159`).
- It records no red: `tests.run` needs `change.built` (`protocol/sdc-story.protocol.yaml:175-179`),
  which only `change.implement` produces (`:160-169`), so the seeded revision is never tested; the
  implementer cannot run commands (`driver/examples/sdc/tools/llm-node.sh:375`).
- `tests.run` needs an ESS specification (`driver/examples/sdc/tools/sdc-tool.sh:165-253`).
- It has 49,152 of the 65,536 states (`docs/protocol-notes.md:364-366`); one more evidence kind
  costs 4 times (`:354-356`).

## Design (decided, D146)

`protocol/sdc-bugfix.protocol.yaml`, one case, no child cases, `sdc.story` unchanged. Steps, each
admissible only after the one before: the bug report is the input; `bug.dissect` (LLM, a data
document under a schema, checked by a program); `test.reproduce` (LLM writes one test) and
`repro.run` (program: that test fails on the unchanged code while the rest of the suite passes,
recorded with the test id and the code revision); `change.fix` (LLM; the test file may not change);
`tests.run` (the same test id and the whole suite pass on the new revision); `story.update` (the
bug's story moved to implemented through `aep` by an `agent:` actor, created if no parent handed it
in); `gate.run` (build, test, clippy `-D warnings`, fmt check). Named outcomes `fixed`,
`not_reproduced`, `not_fixed`. No approval and no decider request in this version; a merge approval
is the next addition. Composable: one input artifact plus a code tree, paths relative to the case's
work directory, so a parent can open it as a child case.

## Acceptance

A run on the seeded crate at `--autopilot` closes by itself with outcome `fixed`, and its case
record shows a `repro_run` with result `red` for the reproducing test id at one change revision and
a `test_run` with result `passed` for the same test id at a later change revision. Programs
produce both records. No LLM judges red or green. Stub cycles show: the full flow closes; a test
that passes on the unchanged code is not red; a fix that edits the test is refused; the fix is not
admissible without the red record.

## Scope

- `protocol/sdc-bugfix.protocol.yaml`, `protocol/compiled/sdc-bugfix.ir.json`.
- `driver/examples/sdc/sdc-bugfix.bindings.json`.
- `driver/src/bin/sdc-bugfix.rs`: the checks, Rust.
- `driver/examples/sdc/tools/llm-node.sh`: nodes `bug.dissect`, `test.reproduce`, `change.fix`;
  `driver/examples/sdc/tools/node-classes.json`.
- `scripts/run-trial.sh`: `--bugfix <seed dir>`.
- `driver/examples/sdc/fixtures/bugfix-pager/` (seed crate with one planted bug, stub trees).
- `cases/12-bugfix-red-first/`.
- `docs/bugfix.md`.
