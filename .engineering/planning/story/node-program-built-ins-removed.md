---
format: aep.planning-md/3
id: story:node-program-built-ins-removed
kind: story
status: proposed
title: The node program has no code for an action of a runbook
summary: The table of actions, store.rs, findings.rs, review.rs, implement.rs, rework.rs, the SDC blocks of call.rs and sdc-node.rs and the option --moved are deleted; the shards of the node program are gone.
tags:
- generic-runner
relations:
- decomposes: epic:runner-knows-no-runbook
- depends_on: story:runner-source-names-no-runbook-ratchet
- depends_on: story:node-program-runs-steps-a-binding-names
- depends_on: story:node-inputs-prepared-by-bound-programs
- depends_on: story:node-reply-stored-by-bound-programs
- depends_on: story:finding-loop-leaves-node-program
- depends_on: story:review-reply-leaves-node-program
- depends_on: story:implement-reply-leaves-node-program
scope:
- confidence: cited
  path: bindings/sdc-bugfix.yaml
- confidence: cited
  path: bindings/sdc-direction.yaml
- confidence: cited
  path: bindings/sdc-intake.yaml
- confidence: cited
  path: bindings/sdc-plan.yaml
- confidence: cited
  path: bindings/sdc-spec.yaml
- confidence: cited
  path: bindings/sdc-story.yaml
- confidence: cited
  path: docs/loom-bindings.md
- confidence: cited
  path: driver/examples/sdc/sdc-bugfix.bindings.json
- confidence: cited
  path: driver/examples/sdc/sdc-direction.bindings.json
- confidence: cited
  path: driver/examples/sdc/sdc-intake.bindings.json
- confidence: cited
  path: driver/examples/sdc/sdc-plan.bindings.json
- confidence: cited
  path: driver/examples/sdc/sdc-spec.bindings.json
- confidence: cited
  path: driver/examples/sdc/sdc-story.bindings.json
- confidence: cited
  path: driver/src/bin/sdc-node
- confidence: cited
  path: driver/src/bin/sdc-node.rs
- confidence: cited
  path: driver/tests/sdc_node.rs
- confidence: inferred
  path: driver/tests/steps
- confidence: inferred
  path: runbook/tests/source_names/known/driver
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T06:01:21Z", actor: "human:operator", revision: 10, executor: "agent:runbook", correlation: "epic-runner-knows-no-runbook-critics-3"}
---
## Outcome

The node program has no code for an action of a runbook. What the five stories before this one moved to steps is deleted: the table of actions in `nodes.rs`, the files `store.rs`, `findings.rs`, `review.rs`, `implement.rs` and `rework.rs`, every block of `call.rs` and `sdc-node.rs` that names an action, an artifact or a tool of SDC, the lines that chose an adapter by the name of a node and checked three mode settings by name, and the transition option `--moved`. Each deletion has its replacement in place: the steps of the five stories, and for the adapter and the modes the rule of `story:node-program-runs-steps-a-binding-names`, which the bindings of `change.review` and `change.implement` use since `story:review-reply-leaves-node-program` and `story:implement-reply-leaves-node-program`. The 14 bindings state their stub tool. The shards of the node program are gone.

This is the contract step of item 2 of the epic: `story:node-program-runs-steps-a-binding-names` was the expand step and the five stories that bound the programs of SDC were the migrate step.

## Source

- `epic:runner-knows-no-runbook`, item 2 ("One generic node program") and item 5 (the check).
- Read at `main` `826b09c7` (the cited lines hold at `3c0eb863`, release 0.7.0: of the paths read, only `runbook/src/commands.rs` and tests of the daemon differ):
  - The node program has 2,821 lines (`wc -l` of `driver/src/bin/sdc-node.rs` and `driver/src/bin/sdc-node/`): `nodes.rs` 221, `store.rs` 277, `findings.rs` 163, `review.rs` 199, `implement.rs` 255, `rework.rs` 125, `call.rs` 481.
  - `build()` (`nodes.rs:128-221`) is one `match` on the action id and ends with `llm-node: no agent node <action>` (`:219`). `call.rs` matches on the action for the constraints of a ticket (`:163-175`), the findings input (`:177-182`), the review and the implementer (`:299-304`) and `apply_copy()` (`:386-431`). `sdc-node.rs` names actions and artifacts in `before_call` (`:168-249`).
  - The stub runs `sdc-tool.sh` of the tools directory by name (`sdc-node.rs:149-153`); `story:binding-states-model-node-inputs-and-output` adds the option `--stub-tool` with that name as its default.
  - The adapter by the name of a node and the three mode settings: `schema_adapter` (`sdc-node.rs:156-166`), its two calls (`:184-188`, `:203-207`), the table of `SDC_IMPLEMENT_MODE`, `SDC_REVIEW_MODE` and `SDC_IMPLEMENT_COVERAGE` (`:189-199`), `facts()` (`:139-142`), the field `request_mode` (`:73-74`) with its uses in `call.rs` (`:177`, `:240`, `:271`, `:293-294`, `:302`), and the three keys of `.stub/settings.yaml` that set these variables (`setup.rs:237-239`, with `review_adapter` at `:235`).
  - The arms that are left in `build()` after `story:binding-states-model-node-inputs-and-output`: `spec.author` (`nodes.rs:143-156`), `bug.dissect` (`:157-164`), `plan.decompose` (`:169-178`), `change.review` (`:179`), `test.reproduce` (`:181-193`), `change.fix` (`:194-218`) and the arm of `change.implement`.
  - Places that write a name of a runbook in a string, counted by the decomposer (the shards of `story:runner-source-names-no-runbook-ratchet` hold the exact lines): action ids in `nodes.rs` 14, `call.rs` 6, `sdc-node.rs` 5, `findings.rs` 5, `store.rs` 5; artifact paths in `nodes.rs` 21, `implement.rs` 8, `call.rs` 6, `findings.rs` 6, `store.rs` 6, `sdc-node.rs` 2.

## Scope

- `driver/src/bin/sdc-node.rs` and `driver/src/bin/sdc-node/` (five files go; `nodes.rs`, `call.rs`, `setup.rs`, `steps.rs` shrink). The seven arms named under Source go with `build()`, and with them the rule that a statement may hold the class alone: every statement has an output. `schema_adapter`, the table of the three mode settings, `facts()`, `request_mode` and the three keys of `setup.rs` go; the rule of the mechanism story is the only code that chooses an adapter and checks a mode.
- The six bindings files with a model node (`bindings/sdc-intake.yaml`, `bindings/sdc-direction.yaml`, `bindings/sdc-spec.yaml`, `bindings/sdc-plan.yaml`, `bindings/sdc-story.yaml`, `bindings/sdc-bugfix.yaml`) and their JSON copies: `--moved` goes, `--stub-tool` comes.
- `driver/tests/steps/` (the tests of the guards go), `driver/tests/sdc_node.rs` where a case names an option that is gone.
- `docs/loom-bindings.md` (the transition leaves the contract); the shards under `runbook/tests/source_names/known/driver/`.
- Overlap, by story:
  - `story:node-program-runs-steps-a-binding-names`, `story:node-inputs-prepared-by-bound-programs`, `story:node-reply-stored-by-bound-programs`, `story:finding-loop-leaves-node-program`, `story:review-reply-leaves-node-program`, `story:implement-reply-leaves-node-program`: edges; this story is built after all of them and alone.
  - `story:node-classes-file-names-no-action` (`driver/src/bin/sdc-node.rs`, `driver/tests/sdc_node.rs`) and `story:runner-source-names-no-runbook`: each has an edge to this story.
  - `story:runner-finds-model-node-by-binding`: ordered through the edges to the stories that bind steps (the bindings files). `story:run-inputs-declared-by-runbook`, `story:conformance-and-root-case-from-runbook`, `story:decisions-routed-by-runbook-data`, `story:second-runbook-with-model-node`: no file in common; this story can be built at once with each of them.
  - `story:one-copy-of-the-bindings` (draft, no typed scope): not in one wave with this story.
  - Drafts with no typed scope, each not built in the same wave as this story: `story:bindings-state-their-exits` (the six bindings files, `driver/src/bin/sdc-node.rs`), `story:producer-id-llm-node` (the six bindings files and their JSON copies), `story:replay-plan-out-of-the-run-copy` (`driver/src/bin/sdc-node/setup.rs`).
  - `story:project-rename` (active): stage 2 renames the node program; precondition, it is not in build while this story is.

## Acceptance

- `git ls-files driver/src/bin/sdc-node/store.rs driver/src/bin/sdc-node/findings.rs driver/src/bin/sdc-node/review.rs driver/src/bin/sdc-node/implement.rs driver/src/bin/sdc-node/rework.rs` prints nothing.
- The node program has no branch on an action: `git grep -n "match c.node.as_str()\|matches!(c.node.as_str()\|match self.node.as_str()\|no agent node" -- driver/src/bin/sdc-node.rs driver/src/bin/sdc-node` prints nothing.
- The adapter and the modes have one owner, the rule of `story:node-program-runs-steps-a-binding-names`: `git grep -n "schema_adapter\|request_mode\|fn facts\|review_mode\|implement_mode\|implement_coverage\|review_adapter" -- driver/src/bin/sdc-node.rs driver/src/bin/sdc-node` prints nothing, and the eleven tests of that rule in `driver/tests/steps/mechanism.rs` pass unchanged. The cases of the review and of the implementer in `driver/tests/sdc_node.rs` (`:1119-1237`, `:1322-1399`) pass with their assertions and their committed expectations as they are at the base of this story: the same adapter per node and mode, and the same refusal of an unknown mode and of an adapter without a schema, with exit 126 and the lines of `driver/tests/expected/review-mode.txt` and `driver/tests/expected/review-refused.txt`, which are byte for byte the files of the base.
- A mode setting is checked by the node that lists it and by no other: the test `a_node_ignores_a_mode_setting_its_statement_does_not_list` starts an invented action with a value of an invented setting that no entry of its `--modes` names, and the node runs. Before this story every node refused a value of the three settings outside their lists (`sdc-node.rs:189-199`); the report names this change. The test `a_mode_setting_is_checked_only_by_the_node_that_lists_it` (`driver/tests/sdc_node.rs`) shows both sides with a setting of SDC, `SDC_REVIEW_MODE=bogus`: `bug.dissect`, whose statement does not list the setting, started the way the case `ticket-bug.dissect` does (`:437-440`), ends as without the variable (its dump equals `driver/tests/expected/ticket-bug.dissect.txt`), and `change.review`, whose statement lists it, ends with exit 126 and the line of today (`driver/tests/expected/review-mode.txt`).
- `build()` and its seven arms are gone, and a statement always has an output: the test `a_statement_without_an_output_is_refused` reads exit 64 and a line that names the action and the option, for a statement with the class alone; the `git grep` of the line on a branch on an action holds that no arm is left.
- It names no artifact, no setting and no tool of SDC: `git grep -n "shared/constraints\|bug.yaml\|findings.yaml\|questions.yaml\|derived-from\|SDC_REVIEW_\|SDC_IMPLEMENT_\|SDC_COVERAGE_\|rule-ends\|AEP_ACTOR\|sdc-tool\|sdc-step\|sdc-findings" -- driver/src/bin/sdc-node.rs driver/src/bin/sdc-node` prints nothing.
- The transition is over: `git grep -n -- "--moved\|\"moved\"" -- driver/src bindings driver/examples/sdc docs/loom-bindings.md driver/tests` prints nothing.
- The 14 bindings state their stub tool (the test `every_model_node_of_sdc_states_its_stub_tool` reads the bindings), and a node on the stub adapter whose binding states none is refused with exit 64 and a line that names the option (`a_stub_without_a_stub_tool_is_refused`).
- The test `a_node_with_steps_at_every_point_runs_for_an_action_no_code_names` (`driver/tests/steps/mechanism.rs`) passes: an invented action with an sh step at each of the four points.
- `git ls-files runbook/tests/source_names/known/driver` prints nothing, and `no_new_runbook_name_in_runner_or_node_source` passes.
- The time of the golden prompt test and of the test step of the driver crate is measured before and after, and both numbers are in the report.
- The prompts of the 14 model nodes are byte-identical before and after: the test `the_prompt_of_each_agent_node_is_its_golden_byte_for_byte` (`driver/tests/bugfix_prompt.rs:214`) passes, `git diff --stat <base> -- instructions/golden/` prints nothing (25 files under `instructions/golden/llm-node/`), and no step of the story runs with `SDC_BLESS_PROMPTS=1`. Where the test has to start a node in a new way, only its harness changes, never a golden file.
- The cases of `driver/tests/sdc_node.rs` keep their committed expectations (`expect()`, `:319-329`, 146 files under `driver/tests/expected/`): `git diff --stat <base> -- driver/tests/expected/` prints nothing, and no step of the story runs with `SDC_BLESS_EXPECTED=1`.
- The cycles keep their results: `task test:cycles-loom` (slow lane, not part of `task check`; started in the background and without the `slot` prefix, as its description says; its task names its time with `SLOT_TIMEOUT=30m`) ends with exit 0 and reports each of the 54 cycles of `cycles/tree-suite.yaml` with the result that file commits for it. `git diff --stat <base> -- cycles/` prints nothing: no expected result is edited to get there.
- `task check` is green at the head of the story.
- The last commit of the unit, made after `main` is merged into it, adds one entry under `[Unreleased]` of `CHANGELOG.md`. No other commit of the story changes that file, and the file is not in the typed scope of the story, so two stories of one wave do not meet on it.
- Tests: no test over 60 s and the test step within 120 s; no model call and no network in a test (stub adapter, sh programs); what is expensive (a built binary, a prepared run) is made once per test run, never per test. Each new test is timed before it is added; a test over 5 s is named with its time in the report. Nothing that stays is Python (Rust or POSIX sh).
- A program, variable, file or format id this story creates for a generic thing carries no `sdc` in its name (epic item 6; the names that exist follow stage 2 of `story:project-rename`).
- The shards of `story:runner-source-names-no-runbook-ratchet` (`runbook/tests/source_names/known/<source path>.txt`) for the source files this story types lose every line this story removes from the source and gain none. `git diff --stat <base> -- runbook/tests/source_names/` lists only those shards, and the report names the lines removed.
- No measured claim of `docs/` or `experiments/` changes. Where a number would change (the time of a node, the count of processes it starts), the measurement is recorded under `experiments/` and listed in `experiments/INDEX.md` before the claim is edited.

## Not in this story

- The read of the class from `bindings/tools/node-classes.json` in `sdc-node.rs`: `story:node-classes-file-names-no-action`.
- Names that say `sdc` for a generic thing (`sdc-node`, `SDC_*` variables of the node program itself, the directory `.sdc/`): item 6, stage 2 of `story:project-rename`.
- The programs of SDC (`sdc-step`, `sdc-findings`, `sdc-tool.sh`) and the library `driver/src/reply/`: they are SDC and stay.
