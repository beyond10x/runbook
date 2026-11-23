---
format: aep.planning-md/3
id: story:implement-reply-leaves-node-program
kind: story
status: proposed
title: The implementer of SDC runs through the steps of the generic node program
summary: Inputs, both modes, the coverage check, the tree copy and the rework of change.implement become steps of sdc-step; implement.rs and rework.rs no longer run.
tags:
- generic-runner
relations:
- decomposes: epic:runner-knows-no-runbook
- depends_on: story:runner-source-names-no-runbook-ratchet
- depends_on: story:binding-states-model-node-inputs-and-output
- depends_on: story:node-inputs-prepared-by-bound-programs
- depends_on: story:node-reply-stored-by-bound-programs
- depends_on: story:finding-loop-leaves-node-program
- depends_on: story:review-reply-leaves-node-program
- depends_on: story:node-program-runs-steps-a-binding-names
scope:
- confidence: cited
  path: bindings/sdc-story.yaml
- confidence: inferred
  path: docs/loom-bindings.md
- confidence: cited
  path: driver/examples/sdc/sdc-story.bindings.json
- confidence: inferred
  path: driver/src/bin/sdc-node/steps.rs
- confidence: inferred
  path: driver/src/bin/sdc-step/implement.rs
- confidence: inferred
  path: driver/tests/expected/implement-settings.txt
- confidence: inferred
  path: driver/tests/steps/implement.rs
- confidence: inferred
  path: driver/tests/steps/mechanism.rs
revision: 13
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T06:01:21Z", actor: "human:operator", revision: 13, executor: "agent:runbook", correlation: "epic-runner-knows-no-runbook-critics-3"}
---
## Outcome

The implementer of SDC (`change.implement`) runs through the points of `story:node-program-runs-steps-a-binding-names` like every other node. Its inputs (requirements, the coverage document) are contributions at `ready`; the check of its coverage document and the asking again are a step at `reply`; its second mode (one request whose reply is the files) is a `ready` step that assembles the request and a `reply` step that turns the reply into the tree; the refusal of a tree that leaves a requirement uncovered, the copy of the tree with its coverage document and the rework after a rejected approval are steps of SDC. On the stub, where a node passes the points `before` and `stored` only (`sdc-node.rs:230-239`), the coverage document of the stub (`coverage_stub`, `implement.rs:82`) and the rework of the stub (`stub_rework`, `rework.rs:46`) are a step at `stored` that reads the name of the adapter in its input and does nothing for another adapter. The binding states the two mode settings of the implementer and that the node replies under a response schema in the mode `request`, so the adapter and the refusal of an unknown mode follow the one rule of that story. This story binds and writes programs of SDC; the mechanism already carries what the implementer needs (the whole prompt and the limits of a request from a `ready` step, a call of several requests, the adapter of a class in one mode). The binding marks the concern `implement` as moved, so `implement.rs`, `rework.rs` and the lines of the implementer in `before_call` no longer run; `story:node-program-built-ins-removed` deletes them. This is the last of the 14 model nodes: after it no node of SDC needs an arm.

## Source

- `epic:runner-knows-no-runbook`, item 2 ("the implement and review replies ... becomes bound programs of SDC actions").
- Read at `main` `826b09c7` (the cited lines hold at `3c0eb863`, release 0.7.0: of the paths read, only `runbook/src/commands.rs` and tests of the daemon differ):
  - `driver/src/bin/sdc-node/implement.rs` (255 lines): `requirements()` (`:27`), `inputs()` (`:37`), `coverage_stub()` (`:82`), `session_calls()` (`:123`), `refuse_uncovered()` (`:167`), `request_call()` (`:191`).
  - `driver/src/bin/sdc-node.rs:189-207`: the modes `SDC_IMPLEMENT_MODE` (`session`, `request`) and `SDC_IMPLEMENT_COVERAGE` (`on`, `off`); in request mode the adapter is chosen by the schema class `implement`.
  - `driver/src/bin/sdc-node/call.rs`: the copy of `out/impl` with `impl/.sdc/coverage.json` (`apply_copy`, `:391-403`); the rejection and the rework text, with the limit read from `driver/examples/sdc/rule-ends.json` (`:218-236`).
  - `driver/src/bin/sdc-node/rework.rs` (125 lines): `find_rejection()` (`:30`, format `sdc.rejection/1`) and `stub_rework()` (`:46`, reads the bindings files beside the tools).
  - The action is bound in `bindings/sdc-story.yaml:26`.
  - `session_calls` (`implement.rs:123-165`) asks again with `instructions/shared/retry-coverage.md` appended to `prompt.md`, at most `SDC_COVERAGE_RETRIES` times (default 2), and logs each as a call; `request_call` (`:191-255`) hands the call to `implement::run` of the library, which starts the adapter itself.
  - The cases of the implementer: `driver/tests/sdc_node.rs:1322-1399`, among them `a_coverage_document_that_names_a_requirement_not_built_sends_the_writer_back` (`:1392`) and `a_coverage_line_outside_its_file_sends_the_writer_back` (`:1399`); the rework: `a_rejection_reaches_change_implement` (`:1373`), `a_persons_rejection_reaches_the_stub` (`:526`). Each compares a dump of the run, exit code and records among it, with a committed file under `driver/tests/expected/`.

## Domain relations

- The relation of `story:node-program-runs-steps-a-binding-names` (a statement of a model node names zero to many steps at each of four points) is used here and not widened. inferable (inferred from `driver/src/bin/sdc-node/implement.rs:37-255` and `driver/src/bin/sdc-node/call.rs:218-236,391-403`; no `ess/1` document declares it).

## Scope

- `driver/src/bin/sdc-step/implement.rs` (the subcommands `sdc-step implement ready|reply|stored`) and `driver/tests/steps/implement.rs`; both files exist empty. The step calls `driver/src/reply/implement/` and `driver/src/reply/coverage.rs`.
- `bindings/sdc-story.yaml` and its JSON copy: the options `--steps` and `--moved` of `change.implement`, the output of its statement (the arm gave it so far), its two mode settings in `--modes` (the key `implement_mode`, the variable `SDC_IMPLEMENT_MODE`, the values `session` and `request`, the default `session`, a schema reply in `request`; the key `implement_coverage`, the variable `SDC_IMPLEMENT_COVERAGE`, the values `on` and `off`, the default `off`), and with the `reply` step of the mode `request` the statement that the starts of one call are one line of the call log.
- No file of the node program and nothing of the mechanism, with one exception that `story:node-program-runs-steps-a-binding-names` names as its open point: if one of the 11 committed files of a call of several requests (`driver/tests/expected/implement-request-*.txt`, `implement-log-implement-*.txt`) cannot be kept byte for byte through the form of that story, this story changes the mechanism for it. It then changes the tests `a_step_names_the_request_of_the_next_start` and `the_starts_of_one_call_are_one_log_line_when_the_binding_says_so` in `driver/tests/steps/mechanism.rs`, the code they hold in `driver/src/bin/sdc-node/steps.rs` and the paragraph of the contract on a call of several requests in `docs/loom-bindings.md`, and nothing else; the report says whether it did and why. Of `driver/src/bin/sdc-step.rs`, `driver/src/bin/sdc-step/` and `driver/tests/steps/` this story otherwise edits only its own module, `driver/src/bin/sdc-step/implement.rs` and `driver/tests/steps/implement.rs`.
- The recorded file `driver/tests/expected/implement-settings.txt`.
- Overlap, by story:
  - `story:node-program-runs-steps-a-binding-names`, `story:node-inputs-prepared-by-bound-programs`, `story:node-reply-stored-by-bound-programs`, `story:finding-loop-leaves-node-program` (`bindings/sdc-story.yaml`), `story:review-reply-leaves-node-program` (`bindings/sdc-story.yaml`): edges; this story is built after all of them and alone.
  - `story:runner-finds-model-node-by-binding`: it marks the argv line of `change.implement`; ordered through the edges above.
  - `story:node-program-built-ins-removed`: it has an edge to this story.
  - `story:one-copy-of-the-bindings` (draft, no typed scope): not in one wave with this story.
  - `story:bindings-state-their-exits` and `story:producer-id-llm-node` (drafts, no typed scope; `bindings/sdc-story.yaml` and, for the second, its JSON copy): each not built in the same wave as this story.

## Acceptance

- The test `the_binding_of_the_implementer_names_its_steps` (`driver/tests/steps/implement.rs`) reads the binding: `change.implement` names its steps, states its output and its two mode settings, and has `implement` in `--moved`.
- The cases of the implementer (`driver/tests/sdc_node.rs:1322-1399`) and of the rework (`:526`, `:1373`) pass with their assertions and their committed expectations unchanged, in both modes and with coverage on and off. Among the expectations are the 11 files of a call of several requests; in the mode `request` the request has no operation, 3 turns, `max_output_tokens` 32000 and the member `implement_mode` as before, given by the `ready` step.
- A tree that leaves a requirement uncovered is refused as before: `a_coverage_document_that_names_a_requirement_not_built_sends_the_writer_back` and `a_coverage_line_outside_its_file_sends_the_writer_back` pass, and the exit code and the record they compare are the committed files of these two cases under `driver/tests/expected/`, which this story does not change.
- The settings keep their meaning. The recorded file `driver/tests/expected/implement-settings.txt` has one row for each value of `SDC_IMPLEMENT_MODE` (`session`, `request`), of `SDC_IMPLEMENT_COVERAGE` (`on`, `off`) and of the effort: the adapter that is chosen, whether the request has a response schema, whether a coverage document is asked for, the effort in the request. The test `each_setting_of_the_implementer_selects_what_it_selected` writes the file when started with `SDC_BLESS_EXPECTED=1`, in the first commit of the story and before any other change, and compares with it otherwise; `git log --oneline <base>..HEAD -- driver/tests/expected/implement-settings.txt` prints one line. This holds what `experiments/effort-implement-levels.yaml` and `experiments/implement-cites-requirements.yaml` measured.
- The implementer does not run in process any more: the test `without_its_steps_the_implementer_node_checks_no_coverage` starts the node with `implement` in `--moved` and no step.
- An approval or a close is never recorded under the name of a person by a rule or a model: the rework reads a rejection and writes none; the test `a_rework_call_writes_no_approval_record` compares the approval records before and after a rework call.
- The spend cap fails closed: the calls of a session and each time the request is asked again are calls in the records of the run, none beyond the budget of the node; the test `the_implementer_starts_no_call_beyond_its_budget` counts the starts of the adapter for a coverage document that never passes.
- The mode `request` makes several requests in one call, and each start is made by the node program. The test `a_token_reaches_no_file_of_a_call_of_the_implementer` plants a sentinel value in a variable of the parent process and in the private login directory, runs both modes with an sh adapter, and finds the sentinel in no argv of a step or of the adapter, in no file of the call record, of the run directory or of the work directory; `git grep` of the sentinel prints nothing.
- The adapter of the implementer is chosen by the rule of `story:node-program-runs-steps-a-binding-names` and not by a line that names the node: the test `the_implementer_runs_on_the_adapter_of_its_class_in_the_request_mode_only` starts `change.implement` with `implement` in `--moved` in both modes, with the general adapter and the adapter of the class `implement` set to two sh adapters, and reads which of the two was started in each mode. A value of `SDC_IMPLEMENT_MODE` or `SDC_IMPLEMENT_COVERAGE` outside its list is refused with exit 126 and the line of today, `llm-node: change.implement could not run: <VARIABLE>=<value> is neither <a> nor <b>` with the two values in the order of today (`session`, `request`; `on`, `off`) (`an_implementer_mode_outside_its_list_is_refused`, `driver/tests/steps/implement.rs`).
- The steps of this story run under the contract of `story:node-program-runs-steps-a-binding-names`. `git diff --stat <base> -- driver/src/bin/sdc-node.rs driver/src/bin/sdc-node driver/src/node driver/src/bin/sdc-step.rs driver/tests/steps/mod.rs` prints nothing, and `git diff --stat <base> -- driver/src/bin/sdc-step driver/tests/steps docs/loom-bindings.md` lists `driver/src/bin/sdc-step/implement.rs` and `driver/tests/steps/implement.rs` only. If the open point named under Scope applies, the second command also lists `driver/tests/steps/mechanism.rs` and `docs/loom-bindings.md`, the first also lists `driver/src/bin/sdc-node/steps.rs`, the tests this story changes in the mechanism are the two named there, and every other test of that story passes unchanged (among them `a_step_gets_path_and_loom_names_only`).
- `git grep -n "env::var\|env_set" -- driver/src/bin/sdc-step/implement.rs` prints only lines that read a `LOOM_*` name.
- The prompts of the 14 model nodes are byte-identical before and after: the test `the_prompt_of_each_agent_node_is_its_golden_byte_for_byte` (`driver/tests/bugfix_prompt.rs:214`) passes, `git diff --stat <base> -- instructions/golden/` prints nothing (25 files under `instructions/golden/llm-node/`), and no step of the story runs with `SDC_BLESS_PROMPTS=1`. Where the test has to start a node in a new way, only its harness changes, never a golden file.
- The cases of `driver/tests/sdc_node.rs` keep their committed expectations (`expect()`, `:319-329`, 146 files under `driver/tests/expected/`): `git diff --stat <base> -- driver/tests/expected/` lists `implement-settings.txt`, the new file of this story, and no other file, and no step of the story runs with `SDC_BLESS_EXPECTED=1` but the first commit, which records that file.
- The cycles keep their results: `task test:cycles-loom` (slow lane, not part of `task check`; started in the background and without the `slot` prefix, as its description says; its task names its time with `SLOT_TIMEOUT=30m`) ends with exit 0 and reports each of the 54 cycles of `cycles/tree-suite.yaml` with the result that file commits for it. `git diff --stat <base> -- cycles/` prints nothing: no expected result is edited to get there.
- `task check` is green at the head of the story.
- The last commit of the unit, made after `main` is merged into it, adds one entry under `[Unreleased]` of `CHANGELOG.md`. No other commit of the story changes that file, and the file is not in the typed scope of the story, so two stories of one wave do not meet on it.
- Tests: no test over 60 s and the test step within 120 s; no model call and no network in a test (stub adapter, sh programs); what is expensive (a built binary, a prepared run) is made once per test run, never per test. Each new test is timed before it is added; a test over 5 s is named with its time in the report. Nothing that stays is Python (Rust or POSIX sh).
- A program, variable, file or format id this story creates for a generic thing carries no `sdc` in its name (epic item 6; the names that exist follow stage 2 of `story:project-rename`).
- No shard of `story:runner-source-names-no-runbook-ratchet` changes: `git diff --stat <base> -- runbook/tests/source_names/` prints nothing.
- No measured claim of `docs/` or `experiments/` changes. Where a number would change (the time of a node, the count of processes it starts), the measurement is recorded under `experiments/` and listed in `experiments/INDEX.md` before the claim is edited.

## Not in this story

- Deleting `implement.rs`, `rework.rs`, the lines of the implementer in `call.rs`, `nodes.rs` and `sdc-node.rs`, and the table of actions: `story:node-program-built-ins-removed`.
- The library `driver/src/reply/implement/`: it stays the library of the SDC programs.
- The member `classes` of `bindings/tools/node-classes.json`: `story:node-classes-file-names-no-action`.
