---
format: aep.planning-md/3
id: story:review-reply-leaves-node-program
kind: story
status: proposed
title: The review of SDC runs through the steps of the generic node program
summary: A ready step gives the inputs and the reply schema, a reply step checks the reply and the node program asks again; review.rs no longer runs.
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
  path: bindings/sdc-story.yaml
- confidence: cited
  path: driver/examples/sdc/sdc-story.bindings.json
- confidence: inferred
  path: driver/src/bin/sdc-step/review.rs
- confidence: inferred
  path: driver/tests/steps/review.rs
revision: 13
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T06:01:21Z", actor: "human:operator", revision: 13, executor: "agent:runbook", correlation: "epic-runner-knows-no-runbook-critics-3"}
---
## Outcome

The review of SDC (`change.review`) runs through the points of `story:node-program-runs-steps-a-binding-names`. Its inputs and its reply schema are contributions of a program of SDC at `ready`; the check of its reply is a program of SDC at `reply`, which the node program asks again with the message of the check, up to the number the binding states. The binding states the mode setting of the review with its two values and that the node replies under a response schema, so the adapter and the refusal of an unknown mode follow the one rule of that story and no line of `sdc-node.rs` names the review for it. The binding marks the concern `review` as moved, so `review.rs` and the lines of the review in `before_call` no longer run; `story:node-program-built-ins-removed` deletes it. The response schema of a node comes one way for every node: as a contribution of a `ready` step (`bug.dissect` in `story:node-inputs-prepared-by-bound-programs`, the review here).

## Source

- `epic:runner-knows-no-runbook`, item 2 ("the implement and review replies ... becomes bound programs of SDC actions").
- Read at `main` `826b09c7` (the cited lines hold at `3c0eb863`, release 0.7.0: of the paths read, only `runbook/src/commands.rs` and tests of the daemon differ):
  - `driver/src/bin/sdc-node/review.rs` (199 lines): `inputs()` (`:35`), `check()` (`:95`) and `call()` (`:121`, the call with the check of its reply and the asking again). It links `sdc_tools::reply::review` (`check_review`, `review_inputs`, `review_inputs_session`) and `sdc_tools::reply::schemas::review_schema` (`:10-13`).
  - `driver/src/bin/sdc-node.rs:184-187`: the adapter of `change.review` is chosen by the schema class `review`; `:189-193`: the mode `SDC_REVIEW_MODE` (`session` or `facts`). `driver/src/bin/sdc-node/call.rs:299-304`: the call of this node goes to `review::call`. `driver/src/bin/sdc-node/nodes.rs:179`: its inputs go to `review::inputs`.
  - `bindings/tools/node-classes.json` has `schema_reply: ["review"]`. The action is bound in `bindings/sdc-story.yaml:56`.
  - `review::call` (`review.rs:121-199`): the request gets `json_schema` and `review_attempt`; the exits are 0 approved, 1 rejected (standard output is then the `sdc.rejection/1`), 2 an invalid reply or a failed call, 75 the API ended; an invalid reply is asked again with `instructions/shared/retry-review.md` appended to `prompt.md`, at most `SDC_REVIEW_RETRIES` times (default 2).
  - The cases of the review: `driver/tests/sdc_node.rs:1119-1237` (17 tests, from `change_review_approves` to `a_review_mode_that_is_none_is_refused`).

## Domain relations

- Statement of a model node -> reply schema: zero or one; -> program that checks the reply: zero or one, only with a schema; -> most times the node asks again: one number, stated with the check. inferable (inferred from `driver/src/bin/sdc-node/review.rs:95-199` and `bindings/tools/node-classes.json`, member `schema_reply`; no `ess/1` document declares it).

## Scope

- `driver/src/bin/sdc-step/review.rs` (the subcommands `sdc-step review ready|reply`) and `driver/tests/steps/review.rs`; both files exist empty. The step calls `driver/src/reply/review.rs` and `driver/src/reply/schemas.rs` and does not change them.
- `bindings/sdc-story.yaml` and its JSON copy: the options `--steps` and `--moved` of `change.review`, the output of its statement (the arm gave it so far), its mode setting in `--modes` (the key `review_mode`, the variable `SDC_REVIEW_MODE`, the values `session` and `facts`, the default `session`) and the statement that it replies under a response schema in both.
- No file of the node program, no file under `docs/`, not `driver/tests/sdc_node.rs`, no file under `driver/tests/expected/`, not `driver/Cargo.toml`. Of `driver/src/bin/sdc-step.rs`, `driver/src/bin/sdc-step/` and `driver/tests/steps/` this story edits only its own module, `driver/src/bin/sdc-step/review.rs` and `driver/tests/steps/review.rs`.
- Overlap, by story:
  - `story:node-program-runs-steps-a-binding-names`: edge (the points, the asking again, the codes 3, 4 and 5 of a `reply` step, the rule of the adapter and of the mode settings, the request members a `ready` step may give).
  - `story:runner-finds-model-node-by-binding`: edge; it marks the argv line of `change.review` in `bindings/sdc-story.yaml`. It also replaces `model_step_argv` in `driver/src/reply/rules.rs`, a file this story does not change.
  - `story:node-inputs-prepared-by-bound-programs`, `story:node-reply-stored-by-bound-programs`: no file in common (their bindings files are not `bindings/sdc-story.yaml`, their modules are `prepare.rs` and `store.rs`); this story can be built at once with either.
  - `story:finding-loop-leaves-node-program` and `story:implement-reply-leaves-node-program`: each has an edge to this story (`bindings/sdc-story.yaml`).
  - `story:node-program-built-ins-removed`: it has an edge to this story.
  - `story:one-copy-of-the-bindings` (draft, no typed scope): not in one wave with this story.
  - `story:bindings-state-their-exits` and `story:producer-id-llm-node` (drafts, no typed scope; `bindings/sdc-story.yaml` and, for the second, its JSON copy): each not built in the same wave as this story.

## Acceptance

- The test `the_binding_of_the_review_names_its_steps` (`driver/tests/steps/review.rs`) reads the binding: `change.review` names a `ready` step, a `reply` step with the number of times to ask again, states its output, its mode setting and its schema reply, and has `review` in `--moved`.
- The 17 cases of the review in `driver/tests/sdc_node.rs:1119-1237` pass with their assertions and their committed expectations unchanged, in both modes (`session`, `facts`): the same exit codes (0, 1 with the rejection on standard output, 2, 75), the same records, the same number of asks (`the_number_of_asks_of_the_review_is_sdc_review_retries`); in the mode `facts` the request has no operation and 4 turns as before, given by the `ready` step. The two cases of a refusal run through the rule of `story:node-program-runs-steps-a-binding-names` and read the exit code and the line they read before: `a_review_mode_that_is_none_is_refused` (the test at `:1237`, its assertion at `:1239`, exit 126, the line of `driver/tests/expected/review-mode.txt`) and `a_review_adapter_that_takes_no_schema_is_refused` (`:1230`, exit 126, the line of `driver/tests/expected/review-refused.txt`). `git diff --stat <base> -- driver/tests/sdc_node.rs driver/tests/expected/review-mode.txt driver/tests/expected/review-refused.txt` prints nothing.
- The review does not run in process any more: the test `without_its_steps_the_review_node_checks_nothing` starts `change.review` with `review` in `--moved` and no step, with a reply the check would refuse, and the node is not asked again.
- The adapter of the review is chosen by the rule of that story and not by a line that names the node: the test `the_review_runs_on_the_adapter_of_its_class_from_its_binding` starts `change.review` with `review` in `--moved`, the general adapter set to one that takes no schema and the adapter of the class `review` set to an sh adapter, and reads the start of the sh adapter and no start of the other.
- Each time the node asks again is a call of its own in the records of the run (count, cost, the request), and no call starts beyond the budget of the node: the test `a_review_that_never_passes_starts_the_adapter_the_stated_number_of_times` counts the starts of the adapter and reads the records.
- An approval or a close is never recorded under the name of a person by a rule or a model: the record a review writes names the principal `llm-review:<model>`, and the test `a_review_of_a_run_of_a_person_names_no_person` starts the node in a run whose principal is a person and reads no person in the record.
- `git grep -n "env::var\|env_set" -- driver/src/bin/sdc-step/review.rs` prints only lines that read a `LOOM_*` name: the mode and the number of asks reach the step as members of `settings` of its input (the mode because the binding lists it, the number by the table of settings of the contract).
- The steps of this story run under the contract of `story:node-program-runs-steps-a-binding-names` and change nothing of it: `git diff --stat <base> -- docs/loom-bindings.md driver/src/bin/sdc-node.rs driver/src/bin/sdc-node/steps.rs driver/src/node driver/tests/steps/mechanism.rs` prints nothing, and the tests of that story pass unchanged (among them `a_step_gets_path_and_loom_names_only`). The dispatcher, the signature of a module, the library of a step and the helpers of the step tests are final since that story: `git diff --stat <base> -- driver/src/bin/sdc-step.rs driver/src/bin/sdc-step driver/src/node/step.rs driver/tests/steps` lists `driver/src/bin/sdc-step/review.rs` and `driver/tests/steps/review.rs` and no other file.
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

- Deleting `review.rs`, the lines of the review in `call.rs` (`:299-304`), `nodes.rs` (`:179`) and `sdc-node.rs` (`:184-193`): `story:node-program-built-ins-removed`.
- The reply of the implementer: `story:implement-reply-leaves-node-program`.
- The library `driver/src/reply/` itself: it stays the library of the SDC programs.
