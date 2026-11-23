---
format: aep.planning-md/3
id: story:node-inputs-prepared-by-bound-programs
kind: story
status: proposed
title: What only SDC prepares before a model call is a program its bindings name
summary: A ready step of sdc-step prepares the inputs of spec.author, bug.dissect, plan.decompose, test.reproduce and change.fix; their arms in nodes.rs no longer run.
tags:
- generic-runner
relations:
- decomposes: epic:runner-knows-no-runbook
- depends_on: story:runner-source-names-no-runbook-ratchet
- depends_on: story:binding-states-model-node-inputs-and-output
- depends_on: story:node-program-runs-steps-a-binding-names
- depends_on: story:runner-finds-model-node-by-binding
scope:
- confidence: cited
  path: bindings/sdc-bugfix.yaml
- confidence: cited
  path: bindings/sdc-plan.yaml
- confidence: cited
  path: bindings/sdc-spec.yaml
- confidence: cited
  path: driver/examples/sdc/sdc-bugfix.bindings.json
- confidence: cited
  path: driver/examples/sdc/sdc-plan.bindings.json
- confidence: cited
  path: driver/examples/sdc/sdc-spec.bindings.json
- confidence: inferred
  path: driver/src/bin/sdc-step/prepare.rs
- confidence: inferred
  path: driver/tests/steps/prepare.rs
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T06:01:20Z", actor: "human:operator", revision: 9, executor: "agent:runbook", correlation: "epic-runner-knows-no-runbook-critics-3"}
---
## Outcome

What only SDC computes before a model call is done by a program of SDC that the bindings of the SDC actions name at the point `ready` of `story:node-program-runs-steps-a-binding-names`: the copy of the example specification and the notes for `spec.author`, the reply schema for `bug.dissect`, the file of commands and the epic for `plan.decompose`, the input of the checks and the values of the reproduction for `test.reproduce` and `change.fix`, and the constraints of a ticket. The bindings of the five actions mark the concern `inputs` as moved, so their arms in `nodes.rs` no longer run; `story:node-program-built-ins-removed` deletes them.

## Source

- `epic:runner-knows-no-runbook`, item 2 ("What is SDC only ... becomes bound programs of SDC actions").
- Read at `main` `826b09c7` (the cited lines hold at `3c0eb863`, release 0.7.0: of the paths read, only `runbook/src/commands.rs` and tests of the daemon differ):
  - `driver/src/bin/sdc-node/nodes.rs`: the arms `spec.author` (`:143-156`), `bug.dissect` (`:157-164`, the schema comes from `sdc-tool.sh bugfix schema dissection`), `plan.decompose` (`:169-178`, `in/commands.txt` from `.sdc/gate/ir.json`), `test.reproduce` (`:181-193`) and `change.fix` (`:194-218`, values from the checks and from `repro/repro.json`).
  - `driver/src/bin/sdc-node/call.rs` (481 lines): the constraints of a ticket for `test.reproduce` and `change.fix` (`:163-175`). `driver/src/bin/sdc-node.rs`: epic and story are cut out of the case id (`Ctx::names()`, `:122-137`), `has_ticket()` reads `bug.yaml` (`:145-147`).
  - The five actions are bound in `bindings/sdc-spec.yaml:20`, `bindings/sdc-bugfix.yaml:14,38,64` and `bindings/sdc-plan.yaml:30`.

## Domain relations

- Statement of a model node -> program that prepares its inputs: zero or one per node; the binding owns the reference, the program belongs to the runbook. inferable (inferred from `driver/src/bin/sdc-node/nodes.rs:143-218`, five arms with code before the task is rendered, seven without; no `ess/1` document declares it).

## Scope

- `driver/src/bin/sdc-step/prepare.rs` (the subcommand `sdc-step prepare <action>`) and `driver/tests/steps/prepare.rs`; both files exist empty.
- `bindings/sdc-spec.yaml`, `bindings/sdc-bugfix.yaml`, `bindings/sdc-plan.yaml` and their JSON copies under `driver/examples/sdc/`: the options `--steps` and `--moved` of the five actions, and the output of each statement with the inputs and the seed that are plain files (the arm gave them so far; `story:runner-finds-model-node-by-binding` left the five statements with the class alone).
- No file of the node program, no file under `docs/`, not `driver/tests/sdc_node.rs`, not `driver/Cargo.toml`. Of `driver/src/bin/sdc-step.rs`, `driver/src/bin/sdc-step/` and `driver/tests/steps/` this story edits only its own module, `driver/src/bin/sdc-step/prepare.rs` and `driver/tests/steps/prepare.rs`.
- Overlap, by story:
  - `story:node-program-runs-steps-a-binding-names`: edge (the points, the contract, the two empty files).
  - `story:runner-finds-model-node-by-binding`: edge; it marks the same argv lines of `bindings/sdc-spec.yaml`, `bindings/sdc-bugfix.yaml` and `bindings/sdc-plan.yaml`.
  - `story:review-reply-leaves-node-program`: no file in common (its bindings file is `bindings/sdc-story.yaml`, its modules are `review.rs`); the two can be built at once.
  - `story:node-reply-stored-by-bound-programs` (the argv lines of `bindings/sdc-plan.yaml` and `bindings/sdc-bugfix.yaml`) and `story:finding-loop-leaves-node-program` (`bindings/sdc-spec.yaml`, `bindings/sdc-plan.yaml`): each has an edge to this story that names the files.
  - `story:implement-reply-leaves-node-program` and `story:node-program-built-ins-removed`: each has an edge to this story.
  - `story:one-copy-of-the-bindings` (draft, no typed scope): not in one wave with this story; in either order only the three JSON files are touched by both.
  - `story:bindings-state-their-exits` and `story:producer-id-llm-node` (drafts, no typed scope; `bindings/sdc-spec.yaml`, `bindings/sdc-bugfix.yaml`, `bindings/sdc-plan.yaml` and, for the second, their JSON copies): each not built in the same wave as this story.

## Acceptance

- The test `the_five_bindings_name_their_prepare_step` (`driver/tests/steps/prepare.rs`) reads the bindings: each of the five actions names one `ready` step `sdc-step prepare <action>`, states its output and has `inputs` in `--moved`.
- The golden prompt test below passes with the prompts of the five built from the contributions of the step (the harness starts a node with the argv of its binding, `driver/tests/sdc_node.rs:73-97`). The 11 goldens of variants (`VARIANT_GOLDENS`, `driver/tests/bugfix_prompt.rs:204-208`) are byte-identical too.
- The arm does not run any more: the test `without_its_step_a_moved_node_gets_none_of_the_prepared_inputs` starts each of the five with `inputs` in `--moved` and no step, and the request holds none of what the step adds (for `spec.author` no example specification, for `bug.dissect` no response schema).
- The constraints of a ticket come from the step: `a_ticket_and_its_constraints_reach_test_reproduce`, `a_ticket_and_its_constraints_reach_change_fix` and `a_ticket_reaches_bug_dissect` (`driver/tests/sdc_node.rs:427-439`) pass unchanged.
- A step that cannot do its work ends the node before any adapter starts: the test `a_prepare_step_without_its_source_ends_the_node_and_no_adapter_starts` (for `plan.decompose` without `.sdc/gate/ir.json`) reads the exit code of the node, the line it printed and an empty call log of the adapter.
- `sdc-step prepare` reads the arguments of the action and the case id from the object on its standard input and from `LOOM_CASE`, and no `SDC_*` variable: `git grep -n "env::var\|env_set" -- driver/src/bin/sdc-step/prepare.rs` prints only lines that read a `LOOM_*` name.
- The time of the golden prompt test is measured before and after (one more process per call for five nodes) and both numbers are in the report.
- The steps of this story run under the contract of `story:node-program-runs-steps-a-binding-names` and change nothing of it: `git diff --stat <base> -- docs/loom-bindings.md driver/src/bin/sdc-node.rs driver/src/bin/sdc-node/steps.rs driver/src/node driver/tests/steps/mechanism.rs` prints nothing, and the tests of that story pass unchanged (among them `a_step_gets_path_and_loom_names_only`). The dispatcher, the signature of a module, the library of a step and the helpers of the step tests are final since that story: `git diff --stat <base> -- driver/src/bin/sdc-step.rs driver/src/bin/sdc-step driver/src/node/step.rs driver/tests/steps` lists `driver/src/bin/sdc-step/prepare.rs` and `driver/tests/steps/prepare.rs` and no other file.
- The prompts of the 14 model nodes are byte-identical before and after: the test `the_prompt_of_each_agent_node_is_its_golden_byte_for_byte` (`driver/tests/bugfix_prompt.rs:214`) passes, `git diff --stat <base> -- instructions/golden/` prints nothing (25 files under `instructions/golden/llm-node/`), and no step of the story runs with `SDC_BLESS_PROMPTS=1`. Where the test has to start a node in a new way, only its harness changes, never a golden file.
- The cases of `driver/tests/sdc_node.rs` keep their committed expectations (`expect()`, `:319-329`, 146 files under `driver/tests/expected/`): `git diff --stat <base> -- driver/tests/expected/` prints nothing, and no step of the story runs with `SDC_BLESS_EXPECTED=1`.
- The cycles keep their results: `task test:cycles-loom` (slow lane, not part of `task check`; started in the background and without the `slot` prefix, as its description says; its task names its time with `SLOT_TIMEOUT=30m`) ends with exit 0 and reports each of the 54 cycles of `cycles/tree-suite.yaml` with the result that file commits for it. `git diff --stat <base> -- cycles/` prints nothing: no expected result is edited to get there.
- `task check` is green at the head of the story.
- The last commit of the unit, made after `main` is merged into it, adds one entry under `[Unreleased]` of `CHANGELOG.md`. No other commit of the story changes that file, and the file is not in the typed scope of the story, so two stories of one wave do not meet on it.
- Tests: no test over 60 s and the test step within 120 s; no model call and no network in a test (stub adapter, sh programs); what is expensive (a built binary, a prepared run) is made once per test run, never per test. Each new test is timed before it is added; a test over 5 s is named with its time in the report. Nothing that stays is Python (Rust or POSIX sh).
- A program, variable, file or format id this story creates for a generic thing carries no `sdc` in its name (epic item 6; the names that exist follow stage 2 of `story:project-rename`).
- No shard of `story:runner-source-names-no-runbook-ratchet` changes: `git diff --stat <base> -- runbook/tests/source_names/` prints nothing.
- No measured claim of `docs/` or `experiments/` changes. Where a number would change (the time of a node, the count of processes it starts), the measurement is recorded under `experiments/` and listed in `experiments/INDEX.md` before the claim is edited.

## Not in this story

- Deleting the five arms of `nodes.rs` and the constraint block of `call.rs` (`:163-175`): `story:node-program-built-ins-removed`. Until then the lines of their shards stay.
- The inputs of `change.review` and `change.implement` (`review::inputs`, `implement::inputs`): `story:review-reply-leaves-node-program`, `story:implement-reply-leaves-node-program`.
- What happens to a reply after it is written: `story:node-reply-stored-by-bound-programs`.
