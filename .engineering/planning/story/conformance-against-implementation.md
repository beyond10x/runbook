---
format: aep.planning-md/3
id: story:conformance-against-implementation
kind: story
status: proposed
title: Story conformance runs against the implementation
tags:
- conductor-succession
relations:
- decomposes: epic:conductor-succession
- depends_on: story:conformance-observe-after-close
scope:
- confidence: inferred
  path: driver/examples/sdc/sdc-story.bindings.json
- confidence: inferred
  path: driver/examples/sdc/tools/llm-node.sh
- confidence: inferred
  path: driver/examples/sdc/tools/sdc-tool.sh
- confidence: inferred
  path: protocol/compiled
- confidence: inferred
  path: protocol/sdc-release.protocol.yaml
- confidence: inferred
  path: protocol/sdc-story.protocol.yaml
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:25:41Z", actor: "human:operator", revision: 5, executor: "agent:sdc-planning", correlation: "operator-standing-acceptance-2026-11-21"}
---
# Story: conformance-against-implementation

## Outcome

The story stage runs the conformance suite synthesized from the run's own spec against the story's
built implementation through `ess verify conform run --target process`, with an adapter the
implementer delivers. The result is evidence of the story case (a conformance run against the
implementation, `passed` or `failed` with the failing scenario ids), required by the merge approval
and by the release gate. A failing scenario sends the implementer back through the existing retry
path with the scenario ids and messages.

## Context

Today `tests.run` (`sdc-tool.sh` `story-tests-run`, :175-181) runs `conform-run` (:155-174): the
suite against ESS's interpreter of the spec (`--target interpreted`; `billing` for ESS's own
example), plus the tree's `task check`. Nothing runs the spec's scenarios against the built
implementation (`docs/driver.md` gap 16; `docs/eval-design.md:616`, decision D1).

Trial 3, run `runs/trial-3-haiku-3`: story `sync` merged with r4 unbuilt and r5 half built, both
`app.tasks` and `app.sync` passed (attempt 3 notes, line 5), and only the decider's read of the
release noticed (`run-stopped.json` `reason`; notes line 4, cause `rejected_without_rework_path`).

ESS pull request 797 (head `153cac0ec6`, not merged, unreleased; merge and release both wait for
the operator) adds the process target. From ESS `docs/design/conformance-target-process.md` and
`website/docs/guides/verify/target-process.md` at that head:

- Command: `ess verify conform run --suite <suite> --target process [--target-dir <dir>]
  [--target-timeout <s>] --report-format 2 --report-out <file> [--strict] -- <program> [args…]`
  (design § "The command line"). Exit with `--strict`: 0 passed, 1 failed or unsupported,
  3 inconclusive, 2 refused before any scenario, no report (§ "Exit status of a run"; guide § "Run it").
- Protocol `ess-conformance-target/1`, one JSON object per line over the adapter's standard
  streams; besides the handshake, seven required operations: `begin_scenario`, `execute_command`,
  `query_view`, `observe_events`, `configure_external_outcome`, `redeliver_event`, `end_scenario`
  (guide § "The messages"). A declared refusal is an ordinary `ok` with `outcome` and `error`
  (guide § "The messages"; design § "`target_error` and how it maps").
- Adapter rules: requests are open at every depth, ignore unknown members; answers are closed
  (guide § "The messages"; design § "The envelope"). Handshake `name` and `version` at most 256
  bytes each and identical across restarts, since a fault restarts the adapter for the next
  scenario (design § "`handshake`"; guide § "The messages"). A signal to `ess` kills the adapter's
  process group and no report is written (design § "Exit status of a run"). A hung adapter costs
  `--target-timeout` per scenario, default 30 s (design § "Known limits"). `--known-failing` and
  `mutate` are refused with `--target process` (design § "Refusals").
- Reference peer: `ess verify conform serve --target billing|oracle-fixture` (design § "Test peer").
  ESS's own check (the ESS session, 2026-11-21): 34 of 34 billing scenarios, report byte-identical
  to the built-in target's.

Suite and selection (the ESS session, 2026-11-21, verified on ess 0.96.0 with `examples/billing`):

- The suite comes from the spec tree alone: `ess verify conform synthesize --path <spec>
  --suite-format 5 --out suite.json`; `run --path <spec>` without `--suite` synthesizes it itself.
- Scenarios are selected by id, not by a command flag. Ids of a command start with `<command>/`
  (e.g. `billing.invoice.CreateInvoice/outcome/accepted`). The wanted ids go as a sorted JSON array
  into a file, then `ess verify conform select --suite suite.json --ids ids.json --out
  selected.json` (needs a suite-format 5 suite) and `ess verify conform run --suite-input
  selected.json --target … --report-format 2 --report-out r.json --strict`. ESS's run: 2
  scenarios, 2 passed.
- A command is also exercised under other prefixes: entity invariants
  `<entity>/invariant/after/<command>/<outcome>` and binding scenarios. So the story's selection
  takes the ids that start with `<command>/` and the ids that contain `/after/<command>/`, for
  every command the story covers. The selection is built by a program (Rust), not by `jq` in a new
  bash tool.

## Adapter: the two options

1. **A Rust adapter in the implementation**, delivered by the implementer: a binary in `impl/`
   (std only, like the rest of `impl/`) that speaks `ess-conformance-target/1` and calls the
   implementation's own domain code; `begin_scenario` starts from an empty store.
2. **A generic SDC adapter** (a Rust crate in SDC) that drives the implementation's HTTP JSON
   surface, which needs one HTTP convention for commands, views, events and a per-scenario reset
   that every implementation must follow.

**Taken: option 1.** The implementations' HTTP surfaces are not fixed by anything SDC owns
(`change.implement` asks for a JSON API with no shape, `llm-node.sh:333-340`), and
`begin_scenario`, `observe_events` and `configure_external_outcome` need the implementation's
internals, which an HTTP client cannot reach without a test surface in every service anyway. With
option 1, ESS still judges every answer; the adapter reports what it observed, never a verdict
(guide, introduction).

## Acceptance

A stub cycle with a recorded adapter records conformance evidence `failed` naming the failing
scenario ids when the adapter answers one scenario wrong and `passed` after the retry, with the
report file stored as the step's log, and one real run on a recorded change records that
evidence from `--target process` against `impl/`, which `change.merge` and the release gate
require.

## Scope

- `driver/examples/sdc/tools/sdc-tool.sh` (`conform-run`, `story-tests-run`)
- `driver/examples/sdc/tools/llm-node.sh` (`change.implement`: the adapter as a deliverable)
- `driver/examples/sdc/sdc-story.bindings.json`, `protocol/sdc-story.protocol.yaml`,
  `protocol/compiled/` (a new evidence kind, or `conformance_run` with a `target`)
- `protocol/sdc-release.protocol.yaml` (the release gate requires it)
- A Rust tool that builds the id selection from the story's commands

## Ambiguities

- `--suite-input` together with `--target process` was not run by ESS; its design page says the
  suite options are unchanged for the process target (design § "The command line").
- A new evidence kind or `conformance_run` with a `target`: chosen when the story is worked.
- The implementer writes the adapter that tests its own code; an adapter that answers without
  calling the domain code is what `story:independent-reviews` and `story:review-node-is-a-model`
  must catch.

## Defaults

- Earlier default (the coordinating session, 2026-11-21): a runner on the SDC side first, and a
  request to ESS only if no existing target kind executes the implementation. ESS 0.96.0 has none
  (`sdc-tool.sh:159`), so the request was made: ESS pull request 797, recorded as
  `upstream-blocker:ess-target-process-release`.
