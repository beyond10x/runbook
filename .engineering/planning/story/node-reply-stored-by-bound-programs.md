---
format: aep.planning-md/3
id: story:node-reply-stored-by-bound-programs
kind: story
status: proposed
title: What SDC does with a reply is a program its bindings name
summary: A reply step of sdc-step does the writes to the planning store, the YAML merges and the special copies of seven nodes; store.rs no longer runs for them.
tags:
- generic-runner
relations:
- decomposes: epic:runner-knows-no-runbook
- depends_on: story:runner-source-names-no-runbook-ratchet
- depends_on: story:binding-states-model-node-inputs-and-output
- depends_on: story:node-program-runs-steps-a-binding-names
- depends_on: story:node-inputs-prepared-by-bound-programs
- depends_on: story:runner-finds-model-node-by-binding
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
  path: driver/examples/sdc/sdc-bugfix.bindings.json
- confidence: cited
  path: driver/examples/sdc/sdc-direction.bindings.json
- confidence: cited
  path: driver/examples/sdc/sdc-intake.bindings.json
- confidence: cited
  path: driver/examples/sdc/sdc-plan.bindings.json
- confidence: inferred
  path: driver/src/bin/sdc-step/store.rs
- confidence: inferred
  path: driver/tests/steps/store.rs
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T06:01:20Z", actor: "human:operator", revision: 9, executor: "agent:runbook", correlation: "epic-runner-knows-no-runbook-critics-3"}
---
## Outcome

What SDC does with a reply after the model wrote it is done by a program of SDC that the bindings of the SDC actions name at the point `reply` of `story:node-program-runs-steps-a-binding-names`: the writes to the planning store for `vision.write`, `roadmap.write` and `plan.decompose`, the merges on YAML for `context.default` and `questions.answer`, the install of the reproduction for `test.reproduce` and the choice of the reply file for `bug.dissect`. The step stores the reply itself and says so with its exit code; the bindings of the seven actions mark the concern `store` as moved, so `store.rs` and the arms of `apply_copy()` no longer run for them; `story:node-program-built-ins-removed` deletes them. A node with no such step keeps the one generic store: the stated output is copied back into the work directory of the case.

## Source

- `epic:runner-knows-no-runbook`, item 2 ("the writes to the planning store ... becomes bound programs of SDC actions").
- Read at `main` `826b09c7` (the cited lines hold at `3c0eb863`, release 0.7.0: of the paths read, only `runbook/src/commands.rs` and tests of the daemon differ):
  - `driver/src/bin/sdc-node/store.rs` (277 lines): `apply()` at `:268-274` is a `match` with five arms (`vision.write`, `roadmap.write`, `plan.decompose`, `context.default`, `questions.answer`) over `vision()` (`:65`), `roadmap()` (`:94`), `decompose()` (`:132`), `context()` (`:226`) and `answer()` (`:253`); the first three call `aep` (`:53-63`).
  - `driver/src/bin/sdc-node/call.rs`: `apply_copy()` at `:386-431` has arms for `change.implement` with coverage, `bug.dissect` and `test.reproduce` (the last runs `sdc-tool.sh bugfix repro-install`) and one generic arm that copies `out/<output>` back.
  - `driver/src/bin/sdc-node.rs:168-174`: the node program sets `AEP_ACTOR` for the planning store: the principal of the run when it is a `<kind>:<name>` principal and not a person, else `agent:<node>` ("no person takes a tool step").

## Domain relations

- Statement of a model node -> program that stores its reply: zero or one per node; with none the reply is copied to the stated output. inferable (inferred from `driver/src/bin/sdc-node/store.rs:268-274` and `driver/src/bin/sdc-node/call.rs:386-431`; no `ess/1` document declares it).

## Scope

- `driver/src/bin/sdc-step/store.rs` (the subcommand `sdc-step store <action>`) and `driver/tests/steps/store.rs`; both files exist empty.
- `bindings/sdc-direction.yaml`, `bindings/sdc-intake.yaml`, `bindings/sdc-plan.yaml`, `bindings/sdc-bugfix.yaml` and their JSON copies: the options `--steps` and `--moved` of the seven actions.
- No file of the node program, no file under `docs/`, not `driver/tests/sdc_node.rs`, not `driver/Cargo.toml`. Of `driver/src/bin/sdc-step.rs`, `driver/src/bin/sdc-step/` and `driver/tests/steps/` this story edits only its own module, `driver/src/bin/sdc-step/store.rs` and `driver/tests/steps/store.rs`.
- Overlap, by story:
  - `story:node-program-runs-steps-a-binding-names`: edge.
  - `story:node-inputs-prepared-by-bound-programs`: edge; both edit the argv lines of `plan.decompose` in `bindings/sdc-plan.yaml` and of `bug.dissect` and `test.reproduce` in `bindings/sdc-bugfix.yaml`.
  - `story:runner-finds-model-node-by-binding`: edge; it marks argv lines of `bindings/sdc-plan.yaml` and `bindings/sdc-bugfix.yaml`.
  - `story:finding-loop-leaves-node-program`: it has an edge to this story (`bindings/sdc-intake.yaml`, `bindings/sdc-direction.yaml`, `bindings/sdc-plan.yaml`, and the order of the points `reply` and `stored`).
  - `story:review-reply-leaves-node-program`: no file in common; the two can be built at once.
  - `story:implement-reply-leaves-node-program` and `story:node-program-built-ins-removed`: each has an edge to this story.
  - `story:one-copy-of-the-bindings` (draft, no typed scope): not in one wave with this story.
  - `story:bindings-state-their-exits` and `story:producer-id-llm-node` (drafts, no typed scope; the four bindings files of this scope and, for the second, their JSON copies): each not built in the same wave as this story.

## Acceptance

- The test `the_seven_bindings_name_their_store_step` (`driver/tests/steps/store.rs`) reads the bindings: each of the seven actions names one `reply` step `sdc-step store <action>` and has `store` in `--moved`.
- The built-in store does not run any more for them: the test `without_its_step_a_moved_node_is_stored_by_the_generic_copy_only` starts `vision.write` with `store` in `--moved` and no step, and nothing was written to the planning store.
- A node with no store step copies its reply to the stated output (the test `a_node_with_no_store_step_copies_its_reply_to_the_stated_output`, with an invented action).
- A store step that fails ends the node with that fact in the record of the call, and the node does not report success: the test `a_store_step_that_fails_is_in_the_record_and_the_node_fails` reads the exit code of the node and the record.
- An approval or a close is never recorded under the name of a person by a rule or a model: the step sets the actor of the planning store by the rule of `driver/src/bin/sdc-node.rs:168-174`, from the member principal of its input, and the test `a_store_step_never_writes_under_the_name_of_a_person` starts it with a principal `human:<x>` and with one `person:<x>` and reads `agent:<node>` in what `aep` recorded.
- `sdc-step store` gets its actor from the object on its standard input and sets `AEP_ACTOR` only for the `aep` process it starts: `git grep -n "env::var\|env_set" -- driver/src/bin/sdc-step/store.rs` prints only lines that read a `LOOM_*` name.
- The steps of this story run under the contract of `story:node-program-runs-steps-a-binding-names` and change nothing of it: `git diff --stat <base> -- docs/loom-bindings.md driver/src/bin/sdc-node.rs driver/src/bin/sdc-node/steps.rs driver/src/node driver/tests/steps/mechanism.rs` prints nothing, and the tests of that story pass unchanged (among them `a_step_gets_path_and_loom_names_only`). The dispatcher, the signature of a module, the library of a step and the helpers of the step tests are final since that story: `git diff --stat <base> -- driver/src/bin/sdc-step.rs driver/src/bin/sdc-step driver/src/node/step.rs driver/tests/steps` lists `driver/src/bin/sdc-step/store.rs` and `driver/tests/steps/store.rs` and no other file.
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

- Deleting `store.rs` and the arms of `apply_copy()` (`call.rs:386-431`): `story:node-program-built-ins-removed`. Until then the lines of their shards stay.
- The copy of the implemented tree with its coverage document (`apply_copy`, the arm `change.implement`): `story:implement-reply-leaves-node-program`.
- Findings a node raised and the marker of a derived tree: `story:finding-loop-leaves-node-program`.
