---
format: aep.planning-md/3
id: story:node-classes-file-names-no-action
kind: story
status: proposed
title: No file beside the bindings lists the actions of a runbook by class
summary: The member classes of node-classes.json is deleted; every reader takes the class of an action from its binding; efforts and default efforts stay.
tags:
- generic-runner
relations:
- decomposes: epic:runner-knows-no-runbook
- depends_on: story:runner-source-names-no-runbook-ratchet
- depends_on: story:binding-states-model-node-inputs-and-output
- depends_on: story:runner-finds-model-node-by-binding
- depends_on: story:node-program-built-ins-removed
- depends_on: story:decisions-routed-by-runbook-data
scope:
- confidence: cited
  path: bindings/tools/node-classes.json
- confidence: cited
  path: docs/driver.md
- confidence: cited
  path: docs/loom-run-decisions.md
- confidence: cited
  path: docs/loom-run-settings.md
- confidence: cited
  path: docs/loom-run.md
- confidence: cited
  path: driver/src/bin/sdc-node.rs
- confidence: cited
  path: driver/src/bin/sdc-protocol-site.rs
- confidence: cited
  path: driver/src/bin/sdc-replay.rs
- confidence: cited
  path: driver/src/reply/adapters.rs
- confidence: cited
  path: driver/tests/expected/review-refused.txt
- confidence: cited
  path: driver/tests/sdc_node.rs
- confidence: inferred
  path: runbook/tests/source_names/known/driver/src/bin/sdc-node.rs.txt
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T06:01:21Z", actor: "human:operator", revision: 12, executor: "agent:runbook", correlation: "epic-runner-knows-no-runbook-critics-3"}
---
## Outcome

No file beside the bindings lists the actions of a runbook by class. The member `classes` of `bindings/tools/node-classes.json` is gone: every program that needs the class of an action reads it from the binding of the action, through the reader of `story:binding-states-model-node-inputs-and-output` and the one function of `story:runner-finds-model-node-by-binding`. What the file keeps is data about classes that names no action: the levels of effort, the default effort of a class and the classes whose reply is a data document.

This is the last step of the move of item 1: the binding states the class (`story:binding-states-model-node-inputs-and-output` for seven nodes, `story:runner-finds-model-node-by-binding` for the other seven), the node program reads the statement first and the file otherwise, the runner reads the statement only; here the read of the file in the node program and the list itself are deleted.

## Source

- `epic:runner-knows-no-runbook`, item 1 ("the node class and the model role move from `nodes.rs` and `node-classes.json` into the binding of the action").
- Read at `main` `826b09c7` (the cited lines hold at `3c0eb863`, release 0.7.0: of the paths read, only `runbook/src/commands.rs` and tests of the daemon differ):
  - `bindings/tools/node-classes.json` (`format: sdc.node-classes/1`): `efforts` (five levels), `default_efforts` (six classes), `classes` (five lists that name 15 actions, 14 of them the model nodes of the bindings and `spec.review`), `schema_reply: ["review"]`.
  - Readers of `classes`: `runbook/src/loom_run/bindings_copy.rs:253-262`, `driver/src/bin/sdc-node.rs:218-228`, `driver/src/bin/sdc-protocol-site.rs:472-476`, `driver/src/bin/sdc-replay.rs:847`, and the tests `driver/tests/sdc_node.rs:454` and `:1806-1828` (the second fails when the file names a node no binding has).
  - One more reader of `classes`, found in revision 2b: `driver/src/reply/adapters.rs:41` takes the names of a class for the parenthesis of the refusal of an adapter (`:42-45`) and for the member `nodes` of the adapters report (`driver/src/bin/sdc-reply.rs:270`). The committed file `driver/tests/expected/review-refused.txt` holds `node class review (spec.review, change.review)`, and the unit test at `adapters.rs:93` asserts those words.
  - Readers of the other members: `driver/src/bin/sdc-node/setup.rs:136-159` (`efforts`), `runbook/src/loom_run/decider/call.rs:78-80` and `runbook/src/loom_run/decider/requester.rs:415-417` (`default_efforts`), `driver/src/conform.rs:273-275`, `driver/src/reply/adapters.rs:35` (`schema_reply`).

## Domain relations

- Model node -> node class: exactly one, stated by the binding (the relation of `story:binding-states-model-node-inputs-and-output`). inferable (inferred from `bindings/tools/node-classes.json`, member `classes`, and `runbook/src/loom_run/bindings_copy.rs:253-259`; no `ess/1` document declares it).
- Node class -> default effort: zero or one per class, data about the class and not about an action. inferable (inferred from `bindings/tools/node-classes.json`, member `default_efforts`).

## Scope

- `bindings/tools/node-classes.json`.
- `driver/src/reply/adapters.rs` (`:41`, the assertion at `:93`) and one line of `driver/tests/expected/review-refused.txt`, as the acceptance says. `story:node-program-runs-steps-a-binding-names` changes `adapters.rs` too and is before this story through the edge to `story:node-program-built-ins-removed`.
- `driver/src/bin/sdc-node.rs` (the read of the class from the file, `:218-228`), `driver/src/bin/sdc-protocol-site.rs`, `driver/src/bin/sdc-replay.rs`, `driver/tests/sdc_node.rs` (`:454`, `:1806-1828`).
- `docs/driver.md` (`:406`, `:416`, `:790`, `:1024`), `docs/loom-run-decisions.md` (`:216`), `docs/loom-run-settings.md`, `docs/loom-run.md` (`:356`); the shard of `sdc-node.rs`, if one is left.
- Overlap, by story:
  - `story:binding-states-model-node-inputs-and-output`, `story:runner-finds-model-node-by-binding`: edges (the statement, the reader, the read of the runner in `bindings_copy.rs`, which is gone before this story).
  - `story:node-program-built-ins-removed`: edge; both change `driver/src/bin/sdc-node.rs` and `driver/tests/sdc_node.rs`. Through it this story follows `story:node-program-runs-steps-a-binding-names`, `story:node-inputs-prepared-by-bound-programs`, `story:node-reply-stored-by-bound-programs`, `story:finding-loop-leaves-node-program`, `story:review-reply-leaves-node-program` and `story:implement-reply-leaves-node-program`.
  - `story:decisions-routed-by-runbook-data`: edge; both change `driver/src/bin/sdc-protocol-site.rs` and `docs/loom-run-decisions.md`. Through it this story follows `story:run-inputs-declared-by-runbook` and `story:conformance-and-root-case-from-runbook` (`docs/loom-run.md`). The coordinator keeps this edge although it puts this story behind the open blocker: the chain of the node program reaches this story later than the answer of the blocker is expected.
  - `story:runner-source-names-no-runbook`: it has an edge to this story.
  - `story:second-runbook-with-model-node`: no file in common.
  - `story:run-settings-are-toml` (active, `docs/loom-run-settings.md`): precondition, it is on `main` before this story starts; this story changes there only the sentences that name the member `classes`.
  - Drafts with no typed scope, each not built in the same wave as this story: `story:decider-inputs-read-once` (`docs/loom-run-decisions.md`, `docs/loom-run.md`), `story:replay-plan-out-of-the-run-copy` (`driver/src/bin/sdc-replay.rs`, `docs/loom-run.md`), `story:replay-lane-budget-and-trigger` (`driver/src/bin/sdc-replay.rs`), `story:bindings-state-their-exits` (`driver/src/bin/sdc-node.rs`), `story:producer-id-llm-node` and `story:driver-md-instruction-files` (`docs/driver.md`), `story:experiment-passes-efforts` (the efforts of `bindings/tools/node-classes.json`, `docs/loom-run-settings.md`).

## Acceptance

- `bindings/tools/node-classes.json` has no member `classes` (`jq -e "has(\"classes\") | not" bindings/tools/node-classes.json` ends with 0), and `git grep -n "\"classes\"\|\[\"classes\"\]" -- driver/src/bin/sdc-node.rs driver/src/bin/sdc-protocol-site.rs driver/src/bin/sdc-replay.rs runbook/src/loom_run/bindings_copy.rs driver/src/reply/adapters.rs` prints nothing.
- The class of each of the 14 model nodes is the one the file had at `826b09c7`: the test `the_14_model_nodes_keep_their_class` holds the 14 pairs as data of the test and reads the class of each from its binding (the pairs stand in test code only).
- The test that fails when a class list names a node no binding has (`driver/tests/sdc_node.rs:1806-1828`) is replaced by `every_model_node_states_a_class_the_classes_file_knows`, which fails when a binding of a model node states no class or one the file does not have in `default_efforts`.
- A node whose binding states no class is refused with exit 64 and a line that names the action (the test `a_model_node_without_a_class_is_refused`); the node program no longer falls back to the file.
- The protocol site (`sdc-protocol-site`) and the replay (`sdc-replay`) show and use the same class per action as before (their existing tests pass with their assertions unchanged).
- The default effort of a node with no `--effort` is the one of its class as before: the recorded file of `story:runner-finds-model-node-by-binding`, `runbook/tests/fixtures/run-copy/model-argv.txt`, holds the value of `--effort` of each of the 14 nodes in the run copy; `the_run_copy_gives_the_model_nodes_their_recorded_argv` passes and `git diff --stat <base> -- runbook/tests/fixtures/run-copy/` prints nothing. `an_effort_option_is_on_the_dry_command_and_in_the_request_of_a_node_of_every_class` (`driver/tests/sdc_node.rs:453`) passes with the classes read from the bindings.
- The prompts of the 14 model nodes are byte-identical before and after: the test `the_prompt_of_each_agent_node_is_its_golden_byte_for_byte` (`driver/tests/bugfix_prompt.rs:214`) passes, `git diff --stat <base> -- instructions/golden/` prints nothing (25 files under `instructions/golden/llm-node/`), and no step of the story runs with `SDC_BLESS_PROMPTS=1`. Where the test has to start a node in a new way, only its harness changes, never a golden file.
- Decided: the parenthesis of the refusal of an adapter holds the action of the refused node. With the member `classes` gone the classes file lists no name for a class, and the rule of `story:node-program-runs-steps-a-binding-names` then puts the action there: the line of `driver/tests/expected/review-refused.txt` reads `node class review (change.review) replies under a response schema` where it read `node class review (spec.review, change.review) replies under a response schema`, and nothing else of the line changes. That one line is recorded again in one commit of this story, and the report names the old and the new line. The adapters report (`adapters()`, printed by `sdc-reply`, `driver/src/bin/sdc-reply.rs:270`) has no node at hand: it writes the line without a parenthesis, and the entry of a class of `schema_reply` in it has no member `nodes`. The unit test `a_schema_class_on_an_adapter_without_a_schema_is_refused` (`driver/src/reply/adapters.rs:88-96`) asserts at `:93` that the line starts with `node class review replies under a response schema, but its adapter metaharness (SDC_REVIEW_ADAPTER) takes none`, and it asserts that the entry `review` of the report has no member `nodes`. The entry of the request mode of the implementer, which the library writes itself (`:60`), is not changed.
- The cases of `driver/tests/sdc_node.rs` keep their committed expectations (`expect()`, `:319-329`, 146 files under `driver/tests/expected/`): `git diff --stat <base> -- driver/tests/expected/` lists `review-refused.txt` and no other file (one line of it, as decided above), and no step of the story runs with `SDC_BLESS_EXPECTED=1` but the one commit that records that file.
- The cycles keep their results: `task test:cycles-loom` (slow lane, not part of `task check`; started in the background and without the `slot` prefix, as its description says; its task names its time with `SLOT_TIMEOUT=30m`) ends with exit 0 and reports each of the 54 cycles of `cycles/tree-suite.yaml` with the result that file commits for it. `git diff --stat <base> -- cycles/` prints nothing: no expected result is edited to get there.
- `task check` is green at the head of the story.
- The last commit of the unit, made after `main` is merged into it, adds one entry under `[Unreleased]` of `CHANGELOG.md`. No other commit of the story changes that file, and the file is not in the typed scope of the story, so two stories of one wave do not meet on it.
- Tests: no test over 60 s and the test step within 120 s; no model call and no network in a test (stub adapter, sh programs); what is expensive (a built binary, a prepared run) is made once per test run, never per test. Each new test is timed before it is added; a test over 5 s is named with its time in the report. Nothing that stays is Python (Rust or POSIX sh).
- A program, variable, file or format id this story creates for a generic thing carries no `sdc` in its name (epic item 6; the names that exist follow stage 2 of `story:project-rename`).
- The shards of `story:runner-source-names-no-runbook-ratchet` (`runbook/tests/source_names/known/<source path>.txt`) for the source files this story types lose every line this story removes from the source and gain none. `git diff --stat <base> -- runbook/tests/source_names/` lists only those shards, and the report names the lines removed.
- No measured claim of `docs/` or `experiments/` changes. Where a number would change (the time of a node, the count of processes it starts), the measurement is recorded under `experiments/` and listed in `experiments/INDEX.md` before the claim is edited.

## Not in this story

- The name and the format id of the file (`sdc.node-classes/1`): item 6, stage 2 of `story:project-rename`.
- `spec.review`, which the file lists and no binding has as a model node: it leaves with the list; the report names it.
