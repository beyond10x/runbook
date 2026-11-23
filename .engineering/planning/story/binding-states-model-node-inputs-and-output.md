---
format: aep.planning-md/3
id: story:binding-states-model-node-inputs-and-output
kind: story
status: proposed
title: The binding of a model node states its inputs, its output and its class
summary: The node program builds a request from what the binding states, through one library reader; seven plain actions lose their arm in nodes.rs; a node finds the files of its own runbook.
tags:
- generic-runner
relations:
- decomposes: epic:runner-knows-no-runbook
- depends_on: story:runner-source-names-no-runbook-ratchet
scope:
- confidence: cited
  path: bindings/sdc-direction.yaml
- confidence: cited
  path: bindings/sdc-intake.yaml
- confidence: cited
  path: docs/loom-bindings.md
- confidence: cited
  path: driver/examples/sdc/sdc-direction.bindings.json
- confidence: cited
  path: driver/examples/sdc/sdc-intake.bindings.json
- confidence: cited
  path: driver/src/bin/sdc-node.rs
- confidence: cited
  path: driver/src/bin/sdc-node/nodes.rs
- confidence: cited
  path: driver/src/bin/sdc-node/setup.rs
- confidence: cited
  path: driver/src/lib.rs
- confidence: inferred
  path: driver/src/node/statement.rs
- confidence: cited
  path: driver/tests/bugfix_prompt.rs
- confidence: cited
  path: driver/tests/sdc_node.rs
- confidence: inferred
  path: runbook/tests/source_names/known/driver/src/bin/sdc-node/nodes.rs.txt
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T06:01:18Z", actor: "human:operator", revision: 9, executor: "agent:runbook", correlation: "epic-runner-knows-no-runbook-critics-3"}
---
## Outcome

The binding of an action states what its model node reads (the input files), the one artifact it writes, the file a first call starts from when there is one, and the class of the node. The node program builds the request from that statement and holds no entry for such an action. The seven actions whose entry in the table of the node program is nothing but that (`idea.dissect`, `questions.write`, `requirements.normalise`, `questions.answer`, `context.default`, `vision.write`, `roadmap.write`) lose their entries, and an action the node program has never heard of runs when its binding states the same.

The statement has one reader, a library module that the node program, the runner and the rules of the reply library share. And a node finds what belongs to the runbook it runs for (its instruction texts, its stub replies, its stub tool) beside the bindings of that runbook, not in the checkout the node program was built in.

This is the first slice of the epic through every layer: the binding, the node program, the test that proves a new action needs no Rust.

## Source

- `epic:runner-knows-no-runbook`, item 1 ("Inputs, the output artifact, the node class and the model role move from `nodes.rs` and `node-classes.json` into the binding of the action") and item 2 ("One generic node program").
- Read at `main` `826b09c7` (the cited lines hold at `3c0eb863`, release 0.7.0: of the paths read, only `runbook/src/commands.rs` and tests of the daemon differ):
  - `driver/src/bin/sdc-node/nodes.rs` (221 lines): `build()` at `:128-221` is one `match` on the action id. Seven arms call `plain(inp, inputs, out, seed)` and nothing else (`:140-142`, `:165-168`), for example `"vision.write" => plain(inp, &["idea.yaml", "requirements.yaml", "context.yaml"], "aep/vision/product.md", None)`. An action with no arm ends at `:219` with `llm-node: no agent node <action>` and exit 64.
  - The class of a node is read from `bindings/tools/node-classes.json` (`format: sdc.node-classes/1`, the member `classes`) by the node program (`driver/src/bin/sdc-node.rs:218-228`; a node in no list is `other`).
  - A binding of a model node is an argv today, for example `[sdc-node, bug.dissect, --model, <model>, --max-turns, "20", --max-budget-usd, "1", --instructions, <json>, --stub, stub-file, <file>, <file>]` (`bindings/sdc-bugfix.yaml:14`). The seven actions are bound in `bindings/sdc-intake.yaml:25,44,52,83` and `bindings/sdc-direction.yaml:16,46,64`.
  - A second copy of the bindings exists as JSON (`driver/examples/sdc/*.bindings.json`); `scripts/check-bindings.sh` and `runbook check` compare the two.
  - `loom.bindings/1` at the pinned Loom (`db2726ff`) is closed. The members `inputs` and `outputs` belong to the `agent` executor and are refused on a `program` executor (`crates/loom-bindings/src/load.rs:269-272`); a tree with an `agent` executor is refused as not run yet (`crates/loom-tree/src/open.rs:289`).
  - The modules of the node program are `#[path]` modules of the binary (`driver/src/bin/sdc-node.rs:30-44`); the library of the crate is `driver/src/lib.rs` (`conform`, `conformance`, `instructions`, `reply`, `run_metrics`). The runner sets an option of a node with `set_option` (`runbook/src/loom_run/bindings_copy.rs:361-377`), which walks the argv in pairs before `--stub`; `--stub` takes the rest of the argv (`driver/src/bin/sdc-node/setup.rs:81`), and the single-valued options are the list `VALUED` (`setup.rs:55-58`).
  - Where a node looks for its files: the tools directory is the option `--tools-dir` or else `bindings/tools` above the binary (`setup.rs:148-160`); the instruction root is the ancestor of the tools directory that has `instructions/` (`sdc-node.rs:79-85`); the stub runs `sdc-tool.sh` of the tools directory (`sdc-node.rs:149-153`); the adapter is `llm-adapter-<name>.sh` of the tools directory (`sdc-node.rs:242-246`). `runbook run` sets `--tools-dir` in its copy of the bindings (`bindings_copy.rs:334-355`); `runbook test` makes no copy (`runbook/src/test_suite.rs`), so a node of a suite outside this checkout reads the tools, the adapters and the instructions of this checkout.
  - Loom at the pin (`db2726ff`) names the bindings directory in no `LOOM_*` variable; with `program_root: bindings` it resolves a program that stands beside the bindings to its absolute real path (`crates/loom-bindings/src/lib.rs:10`).
  - The cases of `driver/tests/sdc_node.rs` start a node with the argv of its action in `bindings/*.yaml` (`binding()`, `:73-97`; `spawn()`, `:226`).

## Domain relations

- Binding of an action -> statement of a model node: zero or one per binding; the binding owns it and it lives exactly as long as the binding. inferable (inferred from `driver/src/bin/sdc-node/nodes.rs:128-221`, one arm per action; no `ess/1` document of this repository declares the relation; the document of Loom `ess/domains/bindings.yaml:29-57` at `db2726ff` types `inputs` and `outputs` on the Agent executor, which the pinned Loom does not run).
- Statement of a model node -> input file: one to many, zero allowed; -> output artifact: exactly one; -> seed file: zero or one. inferable (inferred from the seven `plain()` calls at `nodes.rs:140-168`, each with one `out`).
- Model node -> node class: exactly one. inferable (inferred from `bindings/tools/node-classes.json`, each action in one list of `classes`, and `driver/src/bin/sdc-node.rs:218-228`).

## Form taken in this draft

At the pinned Loom the only member of a `program` executor that can carry the statement is `argv`, so the statement is a set of options of the node program in the argv of the binding. Each option of the statement takes exactly one value and stands before `--stub`, so that `set_option` of the runner keeps working (a list, such as the inputs, is one value). When the Loom pin on `main` at the start of the build runs an `agent` executor or admits a member for it, the form is read again before a line is written; the acceptance below does not depend on the form.

The options of the statement, as defaults of this draft: `--class <name>`, `--inputs <json list>`, `--output <path>` and `--seed <path>`. A statement may hold the class alone. The three others are taken together: a statement with an output is complete, and the inputs and the seed it does not give are none. A statement with no output must hold the class alone, and then the arm of the action in `build()` gives the inputs, the output and the seed as before, until `story:node-program-built-ins-removed` removes the arms. A statement that gives inputs or a seed and no output is refused. An action with a class-only statement and no arm ends as today (`llm-node: no agent node <action>`, exit 64).

For the files of the runbook, the default of this draft: `--tools-dir` names the directory of the runbook; a runbook that is started without a copy (`runbook test`) binds a small sh launcher that stands beside its bindings and passes its own directory in that option (Loom gives the launcher its real path); the stub tool is named by a single-valued option `--stub-tool <name>` and is `sdc-tool.sh` when the option is absent, until `story:node-program-built-ins-removed` makes the 14 bindings state it; an adapter is taken from the tools directory of the runbook when it has one of that name and from the tools beside the node program otherwise. The implementor may take another form that keeps the acceptance.

## Scope

- New library module, the reader of the statement: `driver/src/node/statement.rs` with its line in `driver/src/lib.rs`. It is the first file under `driver/src/node/`, which the check of `story:runner-source-names-no-runbook-ratchet` scans.
- `driver/src/bin/sdc-node/nodes.rs` (the seven arms go), `driver/src/bin/sdc-node/setup.rs` (`VALUED`, the tools directory), `driver/src/bin/sdc-node.rs` (the class, the instruction root, the stub tool, the adapter).
- `bindings/sdc-intake.yaml`, `bindings/sdc-direction.yaml` and their JSON copies under `driver/examples/sdc/`. The two comparers of the copies (`scripts/check-bindings.sh`, `runbook check`) read an argv as a list and are not changed.
- `driver/tests/sdc_node.rs` (new cases), `driver/tests/bugfix_prompt.rs` (the harness that starts a node, not a golden file).
- `docs/loom-bindings.md` (the statement and its options); the shard `runbook/tests/source_names/known/driver/src/bin/sdc-node/nodes.rs.txt`.
- Overlap, by story:
  - `story:runner-finds-model-node-by-binding`, `story:node-program-runs-steps-a-binding-names`, `story:second-runbook-with-model-node`, `story:node-classes-file-names-no-action` and, through them, every later story of the set: each has an edge to this one.
  - `story:one-copy-of-the-bindings` (draft, no typed scope): it deletes the JSON copies. Precondition: it is not built in one wave with this story. In either order nothing but the two JSON files of this scope is touched by both.
  - `story:run-without-model-node-needs-no-caps` (active): it changes `bindings_copy.rs`, which this story reads and does not change; the two do not meet on a line.
  - `story:project-rename` (active, types `driver` and `docs` as a whole): precondition, the stage of it that is in build when this story starts is on `main` first.
  - Drafts with no typed scope, each not built in the same wave as this story: `story:bindings-state-their-exits` (`bindings/sdc-intake.yaml`, `bindings/sdc-direction.yaml`, `driver/src/bin/sdc-node.rs`), `story:producer-id-llm-node` (the two bindings files and their JSON copies), `story:replay-plan-out-of-the-run-copy` (`driver/src/bin/sdc-node/setup.rs`).

## Acceptance

- The bindings of the seven actions state inputs, output, seed and class, and `build()` has no arm for them: `git grep -c` of each of the seven ids in `driver/src/bin/sdc-node/nodes.rs` prints nothing.
- The reader is a library module: `git grep -n "node::statement" -- driver/src/bin/sdc-node.rs driver/src/bin/sdc-node` prints at least one line, and no other code of the node program parses an option of the statement: `git grep -n -- "\"--class\"\|\"--inputs\"\|\"--output\"\|\"--seed\"" -- driver/src/bin/sdc-node.rs driver/src/bin/sdc-node` prints nothing (the list `VALUED` of `setup.rs` takes the four names from the module). Its unit tests read a statement from an argv with no process started.
- A test `a_node_the_program_never_heard_of_runs_from_its_binding` starts the node program with the stub adapter for an action id that stands nowhere in the Rust source, with a binding written for the test: the request holds the stated inputs under their names, and the reply lands at the stated output. Before this story the node ends with `llm-node: no agent node <action>` (exit 64).
- A statement the node program cannot read is refused with exit 64 and a line that names the action and the option, and no adapter starts: inputs or a seed with no output, an input or output path that is absolute or leaves the work directory of the case through `..`, a class that is not one of the known ones, an option of the statement after `--stub`. One test each.
- A statement may hold the class alone. The test `a_node_with_a_class_only_statement_runs_from_its_arm` starts `bug.dissect`, an action with an arm, the way the case `ticket-bug.dissect` does (`driver/tests/sdc_node.rs:437-440`), with `--class` added and no other option of a statement: the dump of the run is byte for byte the committed file `driver/tests/expected/ticket-bug.dissect.txt`. The test `a_class_only_statement_of_an_action_without_an_arm_ends_as_before` reads exit 64 and the line `no agent node`. The arms that are left after this story are the seven of `spec.author`, `bug.dissect`, `plan.decompose`, `change.review`, `change.implement`, `test.reproduce` and `change.fix` (`nodes.rs:143-218`); they give the inputs and the output of these nodes until `story:node-program-built-ins-removed` removes them.
- The first step of the implementor, with the read of `--class` in place and before any other change of this story: show with the test `a_node_with_a_class_only_statement_runs_from_its_arm` that the dump of the class-only run of `bug.dissect` equals `driver/tests/expected/ticket-bug.dissect.txt` byte for byte. The decomposer read this in the source and did not run it. If it does not hold, stop and report.
- The class of a node is read from the statement when the binding has one and from `bindings/tools/node-classes.json` otherwise (the seven nodes this story does not touch); the test `the_class_of_a_statement_wins_over_the_classes_file` holds both halves. The class in the binding of each of the seven equals the one the file has for it (a test compares them). `story:node-classes-file-names-no-action` removes the read of the file.
- The test `a_node_finds_the_tools_of_its_own_runbook` writes a runbook directory outside the checkout (bindings, one instruction text, one stub reply, an sh stub tool, all with invented names) and starts a node for it with the stub adapter as `runbook test` would, with no copy made by the runner: the request holds the instruction text of that directory, the reply is the stub reply of that directory, the stub tool of that directory wrote its marker, and no file of the checkout the node program was built in was read for any of the three (the test runs the node with a checkout path that does not exist in its `PATH` and environment).
- The statement holds paths and names only. The test `a_token_in_the_environment_of_the_runner_reaches_no_file_of_a_node` starts a node with the stub adapter while a sentinel value stands in a variable of the parent process and in the private login directory: the sentinel is in no argv the node starts, in no file of the call record, of the run directory or of the work directory, and `git grep` of the sentinel prints nothing.
- The program contract of Loom is unchanged: only `PATH` and `LOOM_*` reach the node program; the statement arrives in its argv, not in a new variable.
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

- The seven actions whose arm does more than state inputs and an output (`spec.author`, `bug.dissect`, `plan.decompose`, `change.review`, `change.implement`, `test.reproduce`, `change.fix`): `story:node-inputs-prepared-by-bound-programs`, `story:review-reply-leaves-node-program`, `story:implement-reply-leaves-node-program`. Their bindings get the mark of a model node and their class in `story:runner-finds-model-node-by-binding`.
- What the node program does with the reply of `vision.write`, `roadmap.write`, `context.default` and `questions.answer` after it is written (`store.rs`): `story:node-reply-stored-by-bound-programs`.
- How the runner finds a model node and its role: `story:runner-finds-model-node-by-binding`.
- A member of `loom.bindings/1` for the statement: that is a change of Loom, not of this repository.
