---
format: aep.planning-md/3
id: story:runner-finds-model-node-by-binding
kind: story
status: proposed
title: The runner finds a model node by what its binding states, not by the program name
summary: One function classifies an action from the statement of its binding and fails closed; the runner reads the class there; the seven bindings left get their mark and class.
tags:
- generic-runner
relations:
- decomposes: epic:runner-knows-no-runbook
- depends_on: story:runner-source-names-no-runbook-ratchet
- depends_on: story:binding-states-model-node-inputs-and-output
- depends_on: story:run-without-model-node-needs-no-caps
- depends_on: story:runner-records-action-times-and-output
- depends_on: story:loom-run-records-its-claims
scope:
- confidence: cited
  path: bindings/sdc-bugfix.yaml
- confidence: cited
  path: bindings/sdc-plan.yaml
- confidence: cited
  path: bindings/sdc-spec.yaml
- confidence: cited
  path: bindings/sdc-story.yaml
- confidence: cited
  path: docs/loom-run.md
- confidence: cited
  path: driver/examples/sdc/sdc-bugfix.bindings.json
- confidence: cited
  path: driver/examples/sdc/sdc-plan.bindings.json
- confidence: cited
  path: driver/examples/sdc/sdc-spec.bindings.json
- confidence: cited
  path: driver/examples/sdc/sdc-story.bindings.json
- confidence: cited
  path: driver/src/bin/sdc-check-routes.rs
- confidence: cited
  path: driver/src/reply/rules.rs
- confidence: cited
  path: runbook/src/loom_run/bindings_copy.rs
- confidence: cited
  path: runbook/src/loom_run/budget.rs
- confidence: inferred
  path: runbook/src/loom_run/model_node.rs
- confidence: cited
  path: runbook/src/loom_run/replay.rs
- confidence: cited
  path: runbook/src/loom_run/run_dir.rs
- confidence: inferred
  path: runbook/tests/fixtures/run-copy
- confidence: cited
  path: runbook/tests/loom_run/mod.rs
- confidence: inferred
  path: runbook/tests/loom_run/model_node.rs
- confidence: inferred
  path: runbook/tests/source_names/known/runbook/src/loom_run/run_dir.rs.txt
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T06:01:19Z", actor: "human:operator", revision: 11, executor: "agent:runbook", correlation: "epic-runner-knows-no-runbook-critics-3"}
---
## Outcome

The runner decides that an action is a model node from what the binding of the action states, takes the class of the node (and with it the model role) from the binding, and names no program to do so. The decision fails closed: an action the runner cannot classify counts as a model node, so the caps and the ledger apply to it.

The statement and its reader exist (`story:binding-states-model-node-inputs-and-output`, seven bindings). This story is the runner half: the runner, the replay, the run directory writer, the route check and the rules of the reply library read the statement through that reader, and the seven bindings that story does not touch get the mark of a model node and their class.

## Source

- `epic:runner-knows-no-runbook`, item 1 ("A model node is marked as one in the binding; the runner stops finding it by the program name").
- Read at `main` `826b09c7` (the cited lines hold at `3c0eb863`, release 0.7.0: of the paths read, only `runbook/src/commands.rs` and tests of the daemon differ):
  - `runbook/src/loom_run/bindings_copy.rs` (378 lines): `const MODEL_TOOL: &str = "sdc-node"` (`:35`); an action is a model node when the last path part of `argv[0]` equals it (`:330`); only then the runner sets `--tools-dir`, `--model`, `--max-turns`, `--max-budget-usd`, `--effort`, `--adapter` and `--claude-credentials` in the copy (`:334-355`). The role is looked up by the class (`:335`), and the class comes from `tools/node-classes.json` (`class_of`, `:253-259`).
  - The same test by program name stands in `runbook/src/loom_run/replay.rs:43,301,313`, `runbook/src/loom_run/run_dir.rs:244`, `driver/src/reply/rules.rs:16-18` (`model_step_argv`, which matches any member of the argv) and `driver/src/bin/sdc-check-routes.rs:517`. `runbook/src/loom_run/budget.rs:10` reads `--max-budget-usd` from the argv of "a model tool".
  - All 14 model nodes are bound with `argv: [sdc-node, <action>, ...]` (`git grep -c "argv: \[sdc-node" -- bindings/` sums to 14 over six files).
  - The seven model nodes whose bindings `story:binding-states-model-node-inputs-and-output` does not touch are bound in `bindings/sdc-spec.yaml:20`, `bindings/sdc-bugfix.yaml:14,38,64`, `bindings/sdc-plan.yaml:30` and `bindings/sdc-story.yaml:26,56`.
  - Tests that hold the copy today: `models_turns_budgets_and_adapter_land_on_the_model_nodes_of_their_role` (`runbook/src/loom_run/bindings_copy/tests.rs:245`), `an_effort_lands_on_the_model_nodes_of_its_class_and_on_no_other_node` (`:333`), `a_login_file_is_named_in_the_copy_only_by_the_link_it_is_given_and_never_in_a_result` (`:290`).
  - Not read at `826b09c7`, stated by the coordinator: the active `story:run-without-model-node-needs-no-caps` adds the function `decide` in `runbook/src/loom_run/model_node.rs` on its branch.

## Domain relations

- Binding of an action -> kind of node (model node or not): exactly one, decided once at the opening of a run from the binding. inferable (inferred from `runbook/src/loom_run/bindings_copy.rs:330`; no `ess/1` document declares it).
- Node class -> model role: one to one, by equal name. inferable (inferred from `runbook/src/loom_run/bindings_copy.rs:335` and `ROLES` at `:23`).

## Scope

- `runbook/src/loom_run/bindings_copy.rs`, `runbook/src/loom_run/replay.rs`, `runbook/src/loom_run/run_dir.rs`, `runbook/src/loom_run/budget.rs`, and `runbook/src/loom_run/model_node.rs` once it is on `main`.
- `driver/src/reply/rules.rs`, `driver/src/bin/sdc-check-routes.rs`. The reader `driver/src/node/statement.rs` is called and not changed.
- `bindings/sdc-spec.yaml`, `bindings/sdc-bugfix.yaml`, `bindings/sdc-plan.yaml`, `bindings/sdc-story.yaml` and their JSON copies: the mark of a model node and the class of the seven nodes named above, as single-valued options before `--stub` (a statement that holds the class alone, which `story:binding-states-model-node-inputs-and-output` allows). Their inputs and outputs stay in the arms of the node program until the stories named under Overlap.
- `runbook/tests/loom_run/model_node.rs` (new, with its line in `runbook/tests/loom_run/mod.rs`), the recorded file `runbook/tests/fixtures/run-copy/model-argv.txt`, `docs/loom-run.md`; the shard of `run_dir.rs`, which this story leaves as it is.
- Overlap, by story:
  - `story:run-without-model-node-needs-no-caps` (active, `bindings_copy.rs`, `run_loop.rs`): edge. Precondition: that story is in build now and lands first. This story takes its classification function (`decide` in `runbook/src/loom_run/model_node.rs`), keeps its tests and replaces what the function reads (the statement instead of the program name).
  - `story:node-inputs-prepared-by-bound-programs`, `story:node-reply-stored-by-bound-programs`, `story:finding-loop-leaves-node-program`, `story:review-reply-leaves-node-program`, `story:implement-reply-leaves-node-program`, `story:node-program-built-ins-removed`: they edit the same argv lines of the four bindings files. Each has an edge to this story (direct or through one of the others) that names the file.
  - `story:node-program-runs-steps-a-binding-names`: no file in common; the two can be built at once.
  - `story:second-runbook-with-model-node`, `story:node-classes-file-names-no-action`, `story:runner-source-names-no-runbook`: each has an edge to this story. `story:decisions-routed-by-runbook-data` changes `COMPANIONS` in `bindings_copy.rs` and follows this story through its edge to `story:second-runbook-with-model-node`.
  - `story:model-step-is-the-lead-of-the-argv` (draft, no typed scope) tightens `model_step_argv` in `driver/src/reply/rules.rs`, the function this story replaces. Precondition: that draft is not built before this story; after it the coordinator reads that draft again (its outcome is then held by the classification of this story).
  - `story:runner-records-action-times-and-output`, `story:loom-run-records-its-claims` (active, no typed scope, both change `run_dir.rs`): edges; both are in build now and land first. This story changes one test by program name there (`run_dir.rs:244`).
  - `story:one-copy-of-the-bindings` (draft): as in `story:binding-states-model-node-inputs-and-output`, not in one wave with this story.
  - Drafts with no typed scope, each not built in the same wave as this story: `story:bindings-state-their-exits` (the four bindings files), `story:producer-id-llm-node` (the four bindings files and their JSON copies), `story:route-findings-name-loom-stops` (`driver/src/bin/sdc-check-routes.rs`), `story:replay-plan-out-of-the-run-copy` (`runbook/src/loom_run/replay.rs`, `docs/loom-run.md`), `story:decider-inputs-read-once` (`runbook/src/loom_run/run_dir.rs`, `docs/loom-run.md`, `runbook/tests/loom_run/mod.rs`).

## Acceptance

- One function answers "is this binding a model node, and of which class". The bindings copy, the replay plan, the run directory writer, the route check and the rules of the reply library call it, and it reads the statement through `driver/src/node/statement.rs`. `git grep -n "MODEL_TOOL" -- runbook/src` prints nothing, and `git grep -n "sdc-node" -- runbook/src/loom_run/bindings_copy.rs runbook/src/loom_run/replay.rs runbook/src/loom_run/run_dir.rs driver/src/reply/rules.rs driver/src/bin/sdc-check-routes.rs` prints nothing outside test code.
- The decision is written down as a table in `docs/loom-run.md` and each row has a test in `runbook/tests/loom_run/model_node.rs` with an sh program and no model call: `a_binding_with_a_statement_is_a_model_node`; `a_statement_the_runner_cannot_read_counts_as_a_model_node` (unknown class, half of the options: caps and ledger apply, the call is counted, and the opening prints a line that names the action); `an_executor_kind_the_runner_does_not_know_is_a_model_node`; `a_program_with_no_statement_is_no_model_node`.
- A binding with no statement cannot reach a model: the node program started from such a binding ends before any adapter starts (`a_node_without_a_statement_starts_no_adapter`), and the runner gives `--claude-credentials` to no action it did not classify as a model node (`no_login_option_on_an_action_that_is_no_model_node`).
- For the SDC runbook the set of model nodes is the same 14 actions, and the run copy of the bindings has the same argv values. The recorded file `runbook/tests/fixtures/run-copy/model-argv.txt` holds, for one fixed set of settings and each of the 14 actions, the values of `--model`, `--max-turns`, `--max-budget-usd`, `--effort` and `--adapter` in the copy; the test `the_run_copy_gives_the_model_nodes_their_recorded_argv` compares the copy with it.
- Recorded files: a file this story calls recorded is written in the first commit of the story, before any other change, by the named test started with `RUNBOOK_BLESS_EXPECTED=1` (the test writes the file when the variable is set and compares with it otherwise, the way `SDC_BLESS_EXPECTED` works in `driver/tests/sdc_node.rs:319-329`). That commit changes no file under `runbook/src` or `driver/src`, and `git log --oneline <base>..HEAD -- <file>` prints one line: no later commit of the story changes the file.
- The class of all 14 is read from the binding by the runner: `git grep -n "fn class_of\|node-classes.json" -- runbook/src/loom_run/bindings_copy.rs` prints only lines that read the members `efforts` and `default_efforts` (the read of the member `classes`, `:253-262`, is gone).
- Bounds kept, each with the test that holds it: `--confinement` is required and has no default, a file value never lowers a confinement an option or the floor of the daemon sets, the spend cap fails closed and the runner never passes its own environment to a bound program (the existing tests of `runbook/tests/loom_run/flags.rs`, `settings.rs` and `spend.rs` pass with their assertions unchanged); a login is never in argv, a log, a run directory or a committed file: `a_login_file_is_named_in_the_copy_only_by_the_link_it_is_given_and_never_in_a_result` passes unchanged, and the new test `a_sentinel_login_reaches_no_argv_of_the_copy_and_no_file_of_the_run` plants a sentinel value in the login file and finds it in no argv of the copy, no log and no file of the run directory.
- A `review-result` that reviews this story with the role security and has no open finding of severity blocker or warning exists in the store before the story moves to implemented.
- The prompts of the 14 model nodes are byte-identical before and after: the test `the_prompt_of_each_agent_node_is_its_golden_byte_for_byte` (`driver/tests/bugfix_prompt.rs:214`) passes, `git diff --stat <base> -- instructions/golden/` prints nothing (25 files under `instructions/golden/llm-node/`), and no step of the story runs with `SDC_BLESS_PROMPTS=1`. Where the test has to start a node in a new way, only its harness changes, never a golden file.
- The cycles keep their results: `task test:cycles-loom` (slow lane, not part of `task check`; started in the background and without the `slot` prefix, as its description says; its task names its time with `SLOT_TIMEOUT=30m`) ends with exit 0 and reports each of the 54 cycles of `cycles/tree-suite.yaml` with the result that file commits for it. `git diff --stat <base> -- cycles/` prints nothing: no expected result is edited to get there.
- `task check` is green at the head of the story.
- The last commit of the unit, made after `main` is merged into it, adds one entry under `[Unreleased]` of `CHANGELOG.md`. No other commit of the story changes that file, and the file is not in the typed scope of the story, so two stories of one wave do not meet on it.
- Tests: no test over 60 s and the test step within 120 s; no model call and no network in a test (stub adapter, sh programs); what is expensive (a built binary, a prepared run) is made once per test run, never per test. Each new test is timed before it is added; a test over 5 s is named with its time in the report. Nothing that stays is Python (Rust or POSIX sh).
- A program, variable, file or format id this story creates for a generic thing carries no `sdc` in its name (epic item 6; the names that exist follow stage 2 of `story:project-rename`).
- No shard of `story:runner-source-names-no-runbook-ratchet` changes: `git diff --stat <base> -- runbook/tests/source_names/` prints nothing.
- No measured claim of `docs/` or `experiments/` changes. Where a number would change (the time of a node, the count of processes it starts), the measurement is recorded under `experiments/` and listed in `experiments/INDEX.md` before the claim is edited.

## Not in this story

- The reader of the statement and the statements of `idea.dissect`, `questions.write`, `requirements.normalise`, `questions.answer`, `context.default`, `vision.write` and `roadmap.write`: `story:binding-states-model-node-inputs-and-output`.
- The rule that a run without a model node needs no caps: `story:run-without-model-node-needs-no-caps`.
- The name of the node program on `PATH` and in `runbook doctor` (`runbook/src/doctor.rs:109`): a program name, item 6 of the epic.
- The other readers of the member `classes` of `bindings/tools/node-classes.json`: `story:node-classes-file-names-no-action`.
