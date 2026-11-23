---
format: aep.planning-md/3
id: story:node-program-runs-steps-a-binding-names
kind: story
status: proposed
title: The node program runs the steps a binding names, at four fixed points
summary: One way to run a program a binding names before a call, when a model is to be called, with the reply and after the store (option, cleared environment, input on standard input, exit codes, contract in docs), and one rule for the adapter and the mode settings a statement lists; tests with sh steps.
tags:
- generic-runner
relations:
- decomposes: epic:runner-knows-no-runbook
- depends_on: story:runner-source-names-no-runbook-ratchet
- depends_on: story:binding-states-model-node-inputs-and-output
scope:
- confidence: inferred
  path: Taskfile.yml
- confidence: cited
  path: docs/loom-bindings.md
- confidence: cited
  path: driver/Cargo.toml
- confidence: cited
  path: driver/src/bin/sdc-node.rs
- confidence: cited
  path: driver/src/bin/sdc-node/call.rs
- confidence: cited
  path: driver/src/bin/sdc-node/findings.rs
- confidence: cited
  path: driver/src/bin/sdc-node/implement.rs
- confidence: cited
  path: driver/src/bin/sdc-node/nodes.rs
- confidence: cited
  path: driver/src/bin/sdc-node/review.rs
- confidence: cited
  path: driver/src/bin/sdc-node/setup.rs
- confidence: inferred
  path: driver/src/bin/sdc-node/steps.rs
- confidence: cited
  path: driver/src/bin/sdc-node/store.rs
- confidence: inferred
  path: driver/src/bin/sdc-step
- confidence: inferred
  path: driver/src/bin/sdc-step.rs
- confidence: cited
  path: driver/src/lib.rs
- confidence: inferred
  path: driver/src/node/statement.rs
- confidence: inferred
  path: driver/src/node/step.rs
- confidence: cited
  path: driver/src/reply/adapters.rs
- confidence: cited
  path: driver/tests/main.rs
- confidence: cited
  path: driver/tests/sdc_node.rs
- confidence: inferred
  path: driver/tests/steps
- confidence: cited
  path: runbook/src/doctor.rs
- confidence: inferred
  path: runbook/tests/source_names/known/driver
revision: 14
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T06:01:19Z", actor: "human:operator", revision: 14, executor: "agent:runbook", correlation: "epic-runner-knows-no-runbook-critics-3"}
---
## Outcome

The node program has four fixed points at which it runs programs that the binding of the action names (steps), and one way to run them. A runbook that needs something done before a model call, with the reply or after the store writes a program and names it in its binding; the node program holds no code for it. The node program also gets one rule for the adapter and the mode of a node: the adapter follows the class the statement gives, and a mode the statement does not list is refused. This story builds the mechanism and proves it with sh steps that have invented names. It moves nothing of SDC: `story:node-inputs-prepared-by-bound-programs`, `story:node-reply-stored-by-bound-programs`, `story:finding-loop-leaves-node-program`, `story:review-reply-leaves-node-program` and `story:implement-reply-leaves-node-program` bind the programs of SDC to its points, and `story:node-program-built-ins-removed` deletes what they replaced.

This is the expand step of item 2 of the epic; the five stories named are the migrate step and the last one is the contract step.

## Source

- `epic:runner-knows-no-runbook`, item 2 ("One generic node program: it builds the request from the binding, calls the adapter, checks the reply against what the binding states. What is SDC only ... becomes bound programs of SDC actions").
- Read at `main` `826b09c7` (the cited lines hold at `3c0eb863`, release 0.7.0: of the paths read, only `runbook/src/commands.rs` and tests of the daemon differ):
  - The node program has two places before the call. The first runs for every adapter, the stub among them: `questions.write` is skipped while an interview exists (`driver/src/bin/sdc-node.rs:176-182`) and a node is not called while a finding waits for a person (`wait_for_person`, `:213`). Then the stub branch returns (`:230-238`). The second runs only when a model is to be called: the skip of a node whose derived tree is current (`derived_skip`, `:239-241`).
  - After the reply: the review checks its reply and asks again with the reason appended to `prompt.md`, at most `SDC_REVIEW_RETRIES` times (`driver/src/bin/sdc-node/review.rs:121-199`); the implementer does the same for its coverage document (`session_calls`, `driver/src/bin/sdc-node/implement.rs:123-165`). Each asking again is a call in the log.
  - After the store: `Ctx::post` (`driver/src/bin/sdc-node/findings.rs:60-120`), called from `driver/src/bin/sdc-node/call.rs:340-342`, from the stub branch (`sdc-node.rs:236`) and from `driver/src/bin/sdc-node/implement.rs:212,237`.
  - A node reads the arguments of its action as one JSON object from standard input (`driver/src/bin/sdc-node/setup.rs:60-73`); the arm of `spec.author` reads the member `domain` of it (`nodes.rs:143-156`), and epic and story are cut out of the case id that Loom gives in `LOOM_CASE` (`sdc-node.rs:122-137`). The names Loom gives a bound program at the pin (`db2726ff`) include `LOOM_CASE`, `LOOM_CASE_NAME`, `LOOM_PARENT_CASE_NAME`, `LOOM_RETRY_FEEDBACK` and `LOOM_TIMEOUT_SECONDS`.
  - The single-valued options of the node program are the list `VALUED` (`setup.rs:55-58`); `--stub` takes the rest of the argv (`setup.rs:81`); the runner sets options in pairs before `--stub` (`runbook/src/loom_run/bindings_copy.rs:361-377`).
  - What an arm hands to the prompt today (`nodes.rs:128-221`, `call.rs:163-182`): input files under a name, a seed file, notes, values for the task template, parts appended to the task, a response schema (`Built.schema`).
  - The cases of `driver/tests/sdc_node.rs` start a node with the argv of its action in `bindings/*.yaml` (`binding()`, `:73-97`; `spawn()`, `:226`) and compare a dump of the run with a committed file (`dump()`, `:282`; `expect()`, `:319-329`). The driver crate has one integration-test binary, `driver/tests/main.rs`, with one `mod` line per file.
  - The adapter and the modes today (`before_call`, `driver/src/bin/sdc-node.rs:183-211`): the adapter is the general one (`SDC_LLM_ADAPTER`, the stub when it is not set); for `change.review` it is the adapter of the schema class `review` (`schema_adapter`, `:156-166`, `:184-188`); three mode settings are checked against two values each, and a value outside them ends every node with exit 126 and the line `<VARIABLE>=<value> is neither <a> nor <b>` (`:189-199`; the committed file `driver/tests/expected/review-mode.txt` holds that line); for `change.implement` in the mode `request` the adapter is the one of the schema class `implement` (`:203-207`); a node that `SDC_LLM_NODES` does not list runs on the stub (`:208-212`). The adapter of a schema class is `SDC_<CLASS>_ADAPTER`, else `claude-cli` when the general adapter is `metaharness`, else the general adapter (`class_adapter`, `driver/src/reply/adapters.rs:9-19`); one that takes no response schema (`takes_schema`, `:22-31`) is refused with exit 126 (the committed file `driver/tests/expected/review-refused.txt`).
  - What a mode changes in the request (`driver/src/bin/sdc-node/call.rs:239-295`): the mode `facts` of the review takes another preamble, clears the operations and sets 4 turns; the mode `request` of the implementer clears the operations, sets 3 turns, `max_output_tokens` 32000 and the member `implement_mode`, and the node program writes no prompt of its own (`:240`).
  - In the mode `request` the library makes the requests of one call itself (`driver/src/reply/implement/run.rs:91-232`, called from `driver/src/bin/sdc-node/implement.rs:191-255`): it starts the adapter once per request with a request file and a prompt in a directory `req-<k>` of the call record, at most 2 retries and 2 continuations, each with a whole new prompt; it sums the responses into one `response.json`, and the node program writes one line for the call into the call log. 11 committed files hold such a record (`driver/tests/expected/implement-request-*.txt`, 9 files, and `implement-log-implement-*.txt`, 2 files).
  - Under Loom the three mode settings take their defaults: no binding and no setting of `runbook run` states one (`bindings/PENDING.md:104`). The tests set them through the environment or through `.stub/settings.yaml` (`driver/src/bin/sdc-node/setup.rs:232-240`).
  - `runbook doctor` has one row for each program of the driver crate (`SPECS`, `runbook/src/doctor.rs:95-112`), and the test `the_helper_table_matches_the_driver_binaries` (`:352`) compares the rows with the `[[bin]]` entries of `driver/Cargo.toml`.

## Domain relations

- Statement of a model node -> step: zero to many at each of four points; the binding owns the reference and the order, the program belongs to the runbook. inferable (inferred from the places in code named above, each with a list of action ids or a `match` on the action; no `ess/1` document of this repository declares it).
- Step -> point: exactly one of `before`, `ready`, `reply`, `stored`. inferable (inferred from `driver/src/bin/sdc-node.rs:176-241`, `driver/src/bin/sdc-node/review.rs:121-199` and `driver/src/bin/sdc-node/findings.rs:60-120`).

## Form taken in this draft

The names and spellings below are the defaults of this draft, so that five stories can be written against one text. The implementor may change a spelling in this story; the semantics are the acceptance.

- **Points.** `before`: first thing of a node, for every adapter, the stub among them. `ready`: only when a model is to be called, before the prompt is built. `reply`: after the adapter wrote the reply, before the store. `stored`: after the store, also after the store of the stub. A point holds zero or more steps and runs them in the order the binding lists them.
- **Option.** One single-valued option before `--stub`: `--steps <json>`, an object whose members are the four point names, each a list of argv lists. The word 0 of a step is resolved like a bound program: a name without `/` on the `PATH` the node got, a relative path against the directory of the runbook (`--tools-dir`).
- **Input.** A step gets one JSON object on standard input (decided: standard input, not a file, because that is how the node program itself gets the arguments from Loom, and a step can then be a filter with no path to agree on). Its members: the arguments of the action exactly as the node program read them, the action id, the point, the adapter name, the number of the attempt, the path of the retry feedback or null, the principal, and the paths of the work directory, of the call record, of the directory of this step and of `out/`. It also has a member `settings`: the single-valued options the node was started with, except `--claude-credentials`, and those settings the node program reads from `SDC_*` variables that the table of the contract marks as passed (a step gets them as data, never as variables). A copy of the object is kept in the directory of the step.
- **Environment.** Cleared, then by name: `PATH` and every `LOOM_*` variable the node program itself received. Nothing else: no `SDC_*`, no `AEP_ACTOR`, no variable of an adapter, no `HOME`, no login directory. The working directory is the work directory of the case.
- **What a step may hand back.** A `ready` step may write a contributions document into its directory: input files under a name, a seed file, notes, values for the task template, parts appended to the task, a response schema. The node program reads the document and builds the prompt from it with the same templates in the same order as from an arm. That is why the prompt stays byte-identical: the step is given the same arguments and case names the arm read in process, and the rendering stays in one place. The document may also give what a mode changes in the request: the whole prompt (the node program then writes none from its templates), the instruction texts of the preamble and of the closing (paths under the instruction root of the runbook), the operations (a list, which may be empty), the number of turns, the largest number of output tokens, and named members of the request that the node program does not set itself. It may not give the budget, the model, the effort, the adapter, a login or a path outside the call record and the work directory: such a document is refused with exit 64 and a line that names the member, and no adapter starts.
- **Exit codes.** 0: go on. 3: end the node now; the node prints the standard output of the step and ends with the code the step wrote into its directory, or 0 (this is "do not call" at `before` and `ready`, and "the reply is refused for good" at `reply`). 4, at `reply` only: ask again; the node appends the standard output of the step to the prompt and calls the adapter again, up to the number the binding states with the step; after that number the node ends with a code that is not 0 and the record names the last message. 5, at `reply` only: the step stored the reply itself; the node makes no copy of its own and goes on to `stored`. Any other code: the node fails with one line that names the point, the program and the code. The standard error of a step goes to the standard error of the node.
- **Mode settings.** The statement may list mode settings in one single-valued option before `--stub`, `--modes <json>`: a list in which each entry names a setting (the key it has in a settings file and the variable that carries it), its values as a closed list of words, and its default. The node program reads the value of each listed setting where it reads its other settings and holds no name of a setting of a runbook itself. A value that is not in the list is refused before the point `before` with the exit code and the line of today: exit 126 and `llm-node: <action> could not run: <VARIABLE>=<value> is neither <a> nor <b>`, the two values in the order the statement lists them; no step and no adapter starts. Every setting of SDC has two values; for a list of another length the line ends with `is none of <a>, <b>, <c>`, the values in the order of the statement with a comma and a space between them, a default of this draft. The committed file `driver/tests/expected/review-mode.txt` stays byte for byte when `change.review` comes under the rule. The values of the listed settings reach every step as members of `settings`.
- **Adapter.** One rule for every node. The adapter is the general one (the option `--adapter`, else the stub), and a node the settings do not give to the model runs on the stub, as today. A statement may say that its node replies under a response schema: always (a single-valued option, as a default of this draft `--schema-reply yes`), or in named values of one of its mode settings (a member of the entry in `--modes`). Such a node runs on the adapter of the class its statement gives: the adapter set for that class (the variable `SDC_<CLASS>_ADAPTER`, or the key `<class>_adapter` of a settings file), else the general one, by `class_adapter` and `takes_schema` of `driver/src/reply/adapters.rs`, two functions that name no class and no action. An adapter that takes no response schema is refused before any step with the exit code and the line of today: exit 126 and `llm-node: <action> could not run: refused: node class <class> (<names>) replies under a response schema, but its adapter <adapter> (<where it was set>) takes none: set SDC_<CLASS>_ADAPTER=claude-cli`; for a schema reply in one mode the words `in <VARIABLE>=<value>` follow `schema`, as today (`driver/src/reply/adapters.rs:42-45`, `:56-57`). The words stay in the library, in one function that is given the class, the names and the mode and that `adapters()` (`:34-64`) calls too; the names in the parenthesis are the ones the classes file lists for the class, read by the library as today (`:41`). When the file lists no name for the class (the invented class of a test, and every class after `story:node-classes-file-names-no-action`), the parenthesis holds the action of the node that is refused. The committed file `driver/tests/expected/review-refused.txt` stays byte for byte when `change.review` comes under the rule. The node program names no class and no action to do this.
- **A call of several requests.** The node program is the only one that starts an adapter. A `ready` step, and a `reply` step that ends with 4, may leave a document `next` in their directory that names the request of the next start: a directory under the call record for that request, the prompt file in it, and members of the closed list above. Without that document the code 4 does what the paragraph on exit codes says. Each start counts against the number the binding states with the step and against the budget of the node. The binding states with the step whether the starts of one call are written to the call log one line each (the default) or as one line with their sums. The `reply` step writes whatever else belongs to the record of its call; it has the path of the call record.
- **Open point.** This story binds nothing of SDC, so it does not show that the record the implementer of SDC writes for a call of several requests (`req-<k>/`, `requests.jsonl`, the summed `response.json`, one line in the call log) can be written byte for byte through the form above. The 11 committed files named under Source show it in `story:implement-reply-leaves-node-program`. If one of them cannot be kept, that story changes the tests `a_step_names_the_request_of_the_next_start` and `the_starts_of_one_call_are_one_log_line_when_the_binding_says_so` of this story and the paragraph of the contract on a call of several requests, and nothing else of the mechanism.
- **Transition.** A single-valued option `--moved <list>` names the concerns of an action whose code in the node program no longer runs because a step does the work: `inputs`, `store`, `findings`, `review`, `implement`. It exists only between this story and `story:node-program-built-ins-removed`. A binding that moves the concern that holds the arm of its action (`inputs`, `review` or `implement`) gives a complete statement in the same change: the output, and the inputs and the seed that are plain files; what the arm computed comes from the `ready` step. Until then its statement may hold the class alone (`story:binding-states-model-node-inputs-and-output`).
- **Programs of SDC.** One binary `sdc-step` (`driver/src/bin/sdc-step.rs`) with one module per concern under `driver/src/bin/sdc-step/` (`prepare.rs`, `store.rs`, `findings.rs`, `review.rs`, `implement.rs`) and one test module per concern under `driver/tests/steps/`. This story writes the dispatcher whole: it knows the five subcommands, reads the input object through `driver/src/node/step.rs` and hands it to the one function each module has, with one signature for the five (as a default of this draft `pub fn run(input: &step::Input, args: &[String]) -> i32`). It creates the five modules with a body that ends with exit 64, and `driver/tests/steps/mod.rs` with the `mod` lines of all six test modules and the helpers the five need (start a node with a step, read the marker of a step, read the call log). The later stories fill one module and one test module each and change none of the four shared files, so no two of them meet on a file.

## Scope

- `driver/src/bin/sdc-node.rs`, `driver/src/bin/sdc-node/setup.rs` (`VALUED`), `driver/src/bin/sdc-node/call.rs`, `driver/src/bin/sdc-node/nodes.rs`, `driver/src/bin/sdc-node/findings.rs`, `driver/src/bin/sdc-node/store.rs`, `driver/src/bin/sdc-node/review.rs`, `driver/src/bin/sdc-node/implement.rs` (the points, one guard per concern for `--moved`, and the rule of the adapter and of the mode settings); new `driver/src/bin/sdc-node/steps.rs`. The lines that name a node for the adapter or check the three mode settings (`sdc-node.rs:184-207`) stay beside the rule and get the guards of the concerns `review` and `implement`: they run for a node whose binding has not moved that concern, so nothing of SDC changes in this story.
- New `driver/src/node/step.rs` (the input object and the contributions document, for step programs written in Rust), with its line in `driver/src/lib.rs`.
- `driver/src/node/statement.rs`: the options `--modes` and `--schema-reply` join the statement, read by the reader of `story:binding-states-model-node-inputs-and-output`.
- New `driver/src/bin/sdc-step.rs`, `driver/src/bin/sdc-step/` (five modules that end with exit 64); the `[[bin]]` entry in `driver/Cargo.toml`; one row for `sdc-step` in the table of `runbook/src/doctor.rs` (`:95-112`), which the test at `:352` asks for; `Taskfile.yml` where the node program is built and installed.
- `driver/src/reply/adapters.rs`: the words of the refusal of an adapter get one function that is given the class, the names and the mode; `adapters()` calls it, and its output and its unit tests (`:76-113`) do not change.
- `driver/tests/main.rs` (one `mod` line), new `driver/tests/steps/` (`mod.rs` with the helpers, `mechanism.rs` and five empty modules), `driver/tests/sdc_node.rs` (`dump()` leaves out the directories of steps).
- `docs/loom-bindings.md` (the contract of a step); the shards of the node program, which this story leaves as they are.
- No bindings file.
- Overlap, by story:
  - `story:binding-states-model-node-inputs-and-output`: edge (the statement, its reader and `VALUED`).
  - `story:node-inputs-prepared-by-bound-programs`, `story:node-reply-stored-by-bound-programs`, `story:finding-loop-leaves-node-program`, `story:review-reply-leaves-node-program`, `story:implement-reply-leaves-node-program`, `story:node-program-built-ins-removed`: each has an edge to this story. The first five change, of this scope, only their own module of `sdc-step` and of `driver/tests/steps/` (and `implement.rs` and `rework.rs` in `story:finding-loop-leaves-node-program`); their acceptance holds it with a `git diff --stat`. `story:implement-reply-leaves-node-program` changes two tests of `mechanism.rs` only if the open point above shows it has to.
  - `story:runner-finds-model-node-by-binding`, `story:second-runbook-with-model-node`, `story:decisions-routed-by-runbook-data`: no file in common; this story can be built at once with each of them.
  - `story:run-inputs-declared-by-runbook`, `story:conformance-and-root-case-from-runbook`: `Taskfile.yml`; no edge, not in one wave with this story because both type the file.
  - `story:node-classes-file-names-no-action` (`sdc-node.rs`, `driver/tests/sdc_node.rs`) and `story:runner-source-names-no-runbook` (`runbook/src/doctor.rs`, `docs/loom-bindings.md`): each follows `story:node-program-built-ins-removed` by an edge and so this story.
  - `story:project-rename` (active, types `driver` and `docs` as a whole): precondition, the stage of it that is in build when this story starts is on `main` first.
  - Drafts with no typed scope, each not built in the same wave as this story: `story:bindings-state-their-exits` (`driver/src/bin/sdc-node.rs`), `story:replay-plan-out-of-the-run-copy` (`driver/src/bin/sdc-node/setup.rs`), `story:replay-lane-budget-and-trigger` (`Taskfile.yml`).

## Acceptance

- The contract of a step is written in `docs/loom-bindings.md`: the four points and when each runs, the option, the input object member by member, the environment by name, the contributions document with the closed list of what it may give for the request, the exit codes, the directory of a step, the mode settings, the rule of the adapter, a call of several requests, and that `--moved` is a transition that ends with `story:node-program-built-ins-removed`.
- Tests in `driver/tests/steps/mechanism.rs`, each with sh steps that have invented names and with the stub adapter or an sh adapter written for the test, no model call:
  - `a_before_step_runs_for_the_stub_adapter_and_a_ready_step_does_not`.
  - `a_step_that_ends_with_3_ends_the_node_with_its_text_and_no_adapter_starts` (at `before` and at `ready`; the node ends with 0 and the call log of the adapter is empty).
  - `the_contributions_of_a_ready_step_stand_in_the_request` (an input file, a seed, a note, a value, an appended part, a response schema).
  - `a_step_gets_the_arguments_of_the_action_and_the_case_names` (the object on standard input equals the one the node got, and `LOOM_CASE` is in the environment of the step).
  - `a_step_gets_path_and_loom_names_only` (the step prints the names of its environment; the test starts the node with `HOME`, an `SDC_*` variable, `AEP_ACTOR` and a sentinel variable set, and reads `PATH` and `LOOM_*` names and nothing else).
  - `a_reply_step_that_ends_with_4_is_asked_again_with_its_text` (the text of the step is the end of the next prompt; after the stated number the node ends with a code that is not 0; the adapter was started exactly that number plus one times, and each start is a call in the records with its cost).
  - `no_call_starts_beyond_the_budget_of_the_node_when_a_reply_step_keeps_refusing` (the spend cap fails closed: with `--max-budget-usd` or the turns used up, the adapter is not started again).
  - `a_stored_step_runs_after_the_store_and_after_the_store_of_the_stub`.
  - `a_reply_step_that_ends_with_5_stored_the_reply_and_the_stored_point_still_runs` (the node makes no copy of its own).
  - `a_step_that_fails_ends_the_node_with_a_line_that_names_point_program_and_code` (and no adapter starts when the point is `before` or `ready`; a failing `stored` step makes the node end with a code that is not 0).
  - `the_steps_of_a_point_run_in_the_order_of_the_binding`.
  - `a_steps_option_after_the_stub_or_with_an_unknown_point_is_refused` (exit 64, a line that names the action and the option).
  - `a_token_reaches_no_argv_no_input_and_no_file_of_a_step`: a sentinel value stands in a variable of the parent process and in the private login directory; it is in no argv of a step, not in the object on its standard input, in no file of the directory of the step, of the call record or of the run directory, and `git grep` of the sentinel prints nothing.
- The rule of the adapter and of the mode settings has one test per branch in `driver/tests/steps/mechanism.rs`, each with an invented action, an invented class and an invented setting, on the stub adapter or on sh adapters written for the test, no model call:
  - `a_node_that_states_no_schema_reply_runs_on_the_general_adapter`.
  - `a_node_that_replies_under_a_schema_runs_on_the_adapter_of_its_class` (the adapter set for the class of the statement is started and the general one is not).
  - `a_schema_reply_in_one_mode_takes_the_adapter_of_the_class_in_that_mode_only`.
  - `with_no_adapter_set_for_its_class_a_schema_node_runs_on_the_general_adapter`.
  - `an_adapter_of_a_class_that_takes_no_schema_is_refused_before_any_step` (exit 126 and the line of today with the class, the adapter and the variable of the invented statement, and the action in the parenthesis because the classes file lists no name for the invented class; no step ran and no adapter started).
  - `a_node_the_settings_do_not_give_to_the_model_runs_on_the_stub`.
  - `a_value_the_statement_does_not_list_is_refused` (exit 126 and the line `llm-node: <action> could not run: <VARIABLE>=<value> is neither <a> nor <b>` for an invented setting of two values; no step ran and no adapter started).
  - `a_value_outside_a_list_of_three_is_refused_with_the_line_of_a_longer_list` (an invented setting with three values: exit 126 and the line `llm-node: <action> could not run: <VARIABLE>=<value> is none of <a>, <b>, <c>`).
  - `a_mode_setting_that_is_not_set_takes_the_default_of_the_statement`.
  - `the_mode_settings_of_a_node_reach_its_steps_as_settings`.
  - `a_modes_option_the_node_cannot_read_is_refused` (exit 64; not a list, an entry without values, a default that is not one of the values, the option after `--stub`).
- What a mode changes in the request has its tests in the same file, with sh steps and an sh adapter:
  - `a_ready_step_gives_the_whole_prompt_and_the_limits_of_the_request` (the prompt file is the text of the step, and the request holds its operations, its turns, its output tokens and its named member).
  - `a_contribution_that_names_the_budget_the_model_the_adapter_or_a_login_is_refused` (exit 64 and the name of the member, one case per member; no adapter started).
  - `a_step_names_the_request_of_the_next_start` (the adapter is started with the request file of the directory the step named, under the call record; a directory outside the call record is refused).
  - `the_starts_of_one_call_are_one_log_line_when_the_binding_says_so` (one line with the summed cost of two starts, and two lines without the statement).
  - `no_start_beyond_the_stated_number_or_the_budget_when_a_step_names_the_next_request` (the spend cap fails closed).
- The four files the later stories share are final in this story. `driver/src/bin/sdc-step.rs` dispatches the five subcommands, the five modules have the one signature the contract states, `driver/src/node/step.rs` holds the input object and the contributions document, and `driver/tests/steps/mod.rs` holds the `mod` lines and the helpers. The test `every_step_module_has_the_one_signature` holds the signature, and each of the five later stories has the acceptance line that `git diff --stat <base> -- driver/src/bin/sdc-step.rs driver/src/bin/sdc-step driver/src/node/step.rs driver/tests/steps` lists its own two files only.
- For the nodes of SDC nothing changes in this story: no binding lists a mode setting or a schema reply, the lines `sdc-node.rs:184-207` run as before, and `git grep -c "schema_adapter" -- driver/src/bin/sdc-node.rs` still counts 3. The report gives these lines as part of the list of guards.
- `runbook doctor` has a row for `sdc-step`: `the_helper_table_matches_the_driver_binaries` (`runbook/src/doctor.rs:352`) passes, and the tests of `runbook doctor` in `runbook/tests/cli.rs:140-334` pass with their assertions unchanged.
- The contract has a table of settings: every name that `git grep -oh "env_set(\"SDC_[A-Z_]*\")" -- driver/src/bin/sdc-node.rs driver/src/bin/sdc-node | sort -u` prints stands in it once, with "passed to a step as a member of `settings`" or "not passed"; a name of an adapter or of a login is "not passed". The test `the_settings_table_names_every_setting_the_node_reads` compares the table with the source.
- With no `--steps` and no `--moved` a node does what it did: no binding names a step in this story (`git diff --stat <base> -- bindings driver/examples` prints nothing).
- Each guard of `--moved` has a test with an invented step in place of the concern, and `git grep -n "moved" -- driver/src/bin/sdc-node.rs driver/src/bin/sdc-node` lists every guard; the report gives the list, which `story:node-program-built-ins-removed` deletes line by line.
- `sdc-step` builds, every subcommand ends with exit 64 and one line, and `git grep -n "sdc-step" -- driver/src/bin/sdc-node.rs driver/src/bin/sdc-node driver/src/node` prints nothing: the node program does not know the program of SDC.
- The program contract of Loom is unchanged: a bound program gets `PATH` and `LOOM_*` and nothing else, and a step gets no more than that.
- The time of the golden prompt test and of the test step of the driver crate is measured before and after, and both numbers are in the report.
- A `review-result` that reviews this story with the role security and has no open finding of severity blocker or warning exists in the store before the story moves to implemented.
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

- Binding a program of SDC to a point, and any change of a bindings file: the five stories named in the outcome.
- Deleting the code a step replaces, and `--moved`: `story:node-program-built-ins-removed`.
- The mode settings and the schema reply in the bindings of `change.review` and `change.implement`: `story:review-reply-leaves-node-program` and `story:implement-reply-leaves-node-program`. Deleting `schema_adapter` and the table of the three mode settings: `story:node-program-built-ins-removed`.
- `runbook doctor` looking for the programs a binding names as steps: `story:runner-source-names-no-runbook`.
- A member of `loom.bindings/1` for steps: that is a change of Loom, not of this repository.
