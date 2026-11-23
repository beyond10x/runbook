---
format: aep.planning-md/3
id: story:run-inputs-declared-by-runbook
kind: story
status: proposed
title: 'runbook run loses --bugfix, --bugfix-ticket and --goals: a runbook declares its inputs and its program before the open'
summary: A person gives a file with --input <name>=<file>; the story creates the declarations file of a runbook and its one reader; bugfix.rs and goals.rs leave the runner.
tags:
- generic-runner
relations:
- decomposes: epic:runner-knows-no-runbook
- depends_on: story:runner-source-names-no-runbook-ratchet
- depends_on: story:second-runbook-with-model-node
- depends_on: story:run-without-model-node-needs-no-caps
- depends_on: story:runbook-toml-carries-defaults
- depends_on: story:runbooks-load-from-configured-paths
- depends_on: story:router-picks-a-runbook-for-an-input
- depends_on: story:runner-records-action-times-and-output
- depends_on: story:loom-run-records-its-claims
scope:
- confidence: inferred
  path: Taskfile.yml
- confidence: cited
  path: docs/bugfix.md
- confidence: cited
  path: docs/cli.md
- confidence: cited
  path: docs/loom-run.md
- confidence: cited
  path: docs/requester.md
- confidence: inferred
  path: runbook/src/declarations.rs
- confidence: cited
  path: runbook/src/lib.rs
- confidence: cited
  path: runbook/src/loom_run/bugfix.rs
- confidence: cited
  path: runbook/src/loom_run/goals.rs
- confidence: cited
  path: runbook/src/loom_run/here.rs
- confidence: cited
  path: runbook/src/loom_run/ports.rs
- confidence: cited
  path: runbook/src/loom_run/run_loop.rs
- confidence: inferred
  path: runbook/src/router.rs
- confidence: cited
  path: runbook/tests/fixtures/bugfix-runbook
- confidence: inferred
  path: runbook/tests/fixtures/model-runbook
- confidence: cited
  path: runbook/tests/loom_run/bugfix.rs
- confidence: inferred
  path: runbook/tests/loom_run/declarations.rs
- confidence: cited
  path: runbook/tests/loom_run/goals.rs
- confidence: cited
  path: runbook/tests/loom_run/mod.rs
- confidence: inferred
  path: runbook/tests/source_names/known/runbook/src/loom_run/bugfix.rs.txt
- confidence: inferred
  path: runbook/tests/source_names/known/runbook/src/loom_run/goals.rs.txt
- confidence: inferred
  path: runbook/tests/source_names/known/runbook/src/loom_run/here.rs.txt
- confidence: inferred
  path: runbook/tests/source_names/known/runbook/src/loom_run/run_loop.rs.txt
- confidence: inferred
  path: scripts
revision: 13
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T09:57:20Z", actor: "human:operator", revision: 13, executor: "agent:runbook"}
---
## Outcome

`runbook run` has no `--bugfix`, no `--bugfix-ticket` and no `--goals`. A runbook declares which artifact takes the intake text, which files a person may give at the start, and a program that prepares the work directory before the tree opens. A person gives a file with `--input <name>=<file>`, where the name is one the runbook declared; the runner copies it to the declared place and starts the declared program. The bug-fix runbook of SDC declares its ticket, its seed directory and its seed program; the SDC runbook declares its goals document. The runner names none of them.

This story also creates the place of these declarations: one declarations file per runbook, in a closed format of this repository, and its one reader, with all four members the open question names (the start inputs, a program before the open and programs after a close, the default id of the root case, the programs the runbook needs). It uses the first two. `story:conformance-and-root-case-from-runbook` and `story:runner-source-names-no-runbook` use the others and write the values of SDC; they do not change the format or the reader.

## Source

- `epic:runner-knows-no-runbook`, item 3 ("`--bugfix`, `--bugfix-ticket`, `--goals`, `--conformance` become inputs and actions a runbook declares").
- Read at `main` `826b09c7` (the cited lines hold at `3c0eb863`, release 0.7.0: of the paths read, only `runbook/src/commands.rs` and tests of the daemon differ):
  - `runbook/src/loom_run/here.rs:91-99`: the three options. `runbook/src/loom_run/ports.rs` (115 lines): `Ports {dry, replay, bugfix, goals, watch, conformance}`.
  - `runbook/src/loom_run/bugfix.rs` (175 lines): the constants `RUNBOOK = "sdc-bugfix"`, `CASE = "BUG-1"`, `HELPER = "sdc-bugfix"` (`:22-27`); `runbook_of()` turns the runbook `sdc` into `sdc-bugfix` when an option is given; `seed()` runs `sdc-bugfix seed` in the work directory before the tree opens, with a cleared environment and the environment of the bound programs. The head of the file states: the text of the ticket reaches the work directory only, the runner never reads it, and on a failure it prints the last three lines the seed printed.
  - `runbook/src/loom_run/goals.rs` (36 lines): copies the file to `work/goals.yaml`; `check_settings()` refuses a requester decider without it.
  - `runbook/src/loom_run/run_loop.rs`: `bugfix::check` (`:79`), `goals::check_settings` (`:92`), the intake is written to `idea.yaml` (`:296`), `bugfix::seed` (`:297`), `goals::copy` (`:299`).
  - The bindings already name the artifacts: `idea: idea.yaml` and `goals: goals.yaml` (`bindings/sdc.yaml:5-6`), `report: bug.yaml` (`bindings/sdc-bugfix.yaml:5`).
  - The options are named in `docs/bugfix.md`, `docs/cli.md`, `docs/requester.md` and `docs/loom-run.md`.
  - The tests that hold a bug-fix run today (`runbook/tests/loom_run/bugfix.rs`): `a_ticket_seeds_the_root_case_before_the_tree_opens` (`:169`), `the_runbook_argument_sdc_names_the_bug_fix_runbook_when_an_option_is_given` (`:194`), `an_explicit_case_is_kept` (`:204`), `a_seed_directory_seeds_with_the_intake_as_the_report` (`:212`), `the_ticket_text_reaches_the_work_directory_and_no_other_file_stream_or_event` (`:222`), `a_failed_seed_ends_the_run_with_the_last_lines_the_seed_printed_and_no_ticket_text` (`:234`), `the_options_are_checked_before_anything_is_written` (`:250`), `a_run_without_a_bug_fix_option_does_not_start_the_helper` (`:280`).
  - The modules of the runner are listed in `runbook/src/lib.rs`.

## Domain relations

- Runbook -> input a person may give at the start: zero to many; the runbook owns the declaration; each input lands on one artifact of the bindings. inferable (inferred from `runbook/src/loom_run/goals.rs` and `runbook/src/loom_run/bugfix.rs:22-27`, one option per input in code; no `ess/1` document declares it).
- Runbook -> program before the open: zero or one. inferable (inferred from `runbook/src/loom_run/run_loop.rs:297`).
- Runbook -> declarations file: zero or one; a runbook without one declares nothing. inferable (inferred from the four places in code that the question of the blocker lists; no `ess/1` document declares it).
- Which file of a runbook holds these declarations: decided by the operator on 2026-11-23, option A of `decision-blocker:runbook-declarations-without-a-format-member`: a TOML file that the runbook owns, beside its bindings, in a closed format of this repository; no change of Canon or Loom.

## Names taken in this draft

The names of the new options are defaults of the coordinator, written here so that the operator may rename them before the build: start inputs are given as `--input <name>=<file>` (repeatable); a step a runbook declares as optional after a close is switched on with `--step <name>[=<mode>]` (`story:conformance-and-root-case-from-runbook`). The name of the declarations file is not fixed before the blocker closes; `declarations.toml` beside the bindings is the working name, and it is not `runbook.toml`, which is the configuration file of `story:runbook-toml-carries-defaults`.

## Scope

- New: the reader, `runbook/src/declarations.rs` with its line in `runbook/src/lib.rs`. The declarations file is one new TOML file beside the bindings of `sdc`, of `sdc-bugfix` and of each fixture runbook that declares something.
- `runbook/src/loom_run/here.rs`, `runbook/src/loom_run/ports.rs`, `runbook/src/loom_run/run_loop.rs`; `runbook/src/loom_run/bugfix.rs` and `runbook/src/loom_run/goals.rs` (both go).
- `runbook/tests/loom_run/bugfix.rs`, `runbook/tests/loom_run/goals.rs`, new `runbook/tests/loom_run/declarations.rs` (with its line in `runbook/tests/loom_run/mod.rs`); `runbook/tests/fixtures/bugfix-runbook/` (its declarations and the two recorded listings); `runbook/tests/fixtures/model-runbook/` (one declared input).
- `docs/cli.md`, `docs/bugfix.md`, `docs/requester.md`, `docs/loom-run.md`; `Taskfile.yml` and `scripts/` where one passes an old option; the shards of `bugfix.rs`, `goals.rs`, `here.rs` and `run_loop.rs`.
- Overlap, by story:
  - `story:second-runbook-with-model-node`: edge (the fixture, `runbook/tests/loom_run/mod.rs`).
  - `story:conformance-and-root-case-from-runbook`: it has an edge to this story (the reader, `ports.rs`, `here.rs`, `run_loop.rs`, the fixture). This story deletes `bugfix.rs` with the constant `BUG-1` and `case_of()` (`:25`, `:65`); until that story is in, every task, script and test that starts a bug-fix run gives `--case BUG-1`, and `docs/bugfix.md` says so.
  - `story:decisions-routed-by-runbook-data`: chosen, and written in both stories: the `git grep` of this story does not read `runbook/src/loom_run/decider/`, and that story owns the lines `decider/inputs.rs:78,97` and `decider/requester.rs:239`. It has an edge to this story as well, for the files both type (the fixture, `runbook/tests/loom_run/mod.rs`) and because it reads the place of the goals document from the input declared here.
  - `story:runner-source-names-no-runbook` (edge to this story) owns the hit in `run_dir.rs`; `story:node-classes-file-names-no-action` follows this story through its edge to `story:decisions-routed-by-runbook-data` (`docs/loom-run.md`).
  - `story:node-program-runs-steps-a-binding-names`: `Taskfile.yml`; no edge, not in one wave with this story because both type the file. `story:node-program-built-ins-removed` and the stories that bind steps have no file in common with this one.
  - `story:run-without-model-node-needs-no-caps` (active; `here.rs`, `run_loop.rs`), `story:runbook-toml-carries-defaults` (active; `here.rs`, `docs/loom-run.md`), `story:runbooks-load-from-configured-paths` (active; `here.rs`): edges; all three are in build now and land first.
  - `story:router-picks-a-runbook-for-an-input` (active): edge; it lands first. It names `--bugfix` as a rule of the router; this story changes that rule to the new spelling where it is code (`runbook/src/router.rs`).
  - `story:runner-records-action-times-and-output`, `story:loom-run-records-its-claims` (active, no typed scope, both change `run_loop.rs`): edges; both are in build now and land first.
  - Drafts with no typed scope, each not built in the same wave as this story: `story:decider-inputs-read-once` and `story:replay-plan-out-of-the-run-copy` (`docs/loom-run.md`, `runbook/tests/loom_run/mod.rs`), `story:replay-lane-budget-and-trigger` (`Taskfile.yml`).

## Acceptance

- The format of the declarations file is closed and written down in `docs/loom-run.md`: a format id with no `sdc` in it; the artifact that takes the intake text; the inputs (each a name, the entry of the `artifacts` map of the bindings it lands on, whether it is required, whether it is a file or a directory); the program before the open; the programs after a close (each a name and its modes); the default id of the root case; the programs the runbook needs. A member the format does not have is refused by name (`an_unknown_member_of_the_declarations_is_refused_by_name`); a place that is absolute or leaves the work directory of the root case is refused when the runbook is read (`a_declared_place_outside_the_work_directory_is_refused`); a runbook without the file declares nothing and runs as before (`a_runbook_without_declarations_declares_nothing`).
- There is one reader: the format id of the declarations file stands in one source file (`git grep -l` of the id over `runbook/src` and `driver/src` prints `runbook/src/declarations.rs` only), and `here.rs` and `run_loop.rs` reach a declaration through that module only.
- `runbook run --help` lists `--input` and none of the three old options. Each old option, when given, is refused as a usage error before anything is written, with one line that names the new spelling (`--input <name>=<file>`) and says that `runbook run <runbook> --help` lists the declared names: the test `the_three_old_options_are_refused_with_the_new_spelling`.
- `runbook run <runbook> --help` lists the inputs that runbook declares. The test `the_help_of_a_run_lists_the_declared_inputs_of_its_runbook` starts it for `sdc-bugfix`, for the bug-fix fixture and for the second runbook: standard output holds each declared name with the word `required` or `optional` and whether it is a file or a directory, and the exit code is 0. For a runbook without declarations the help says that it declares no input (same test).
- `git ls-files runbook/src/loom_run/bugfix.rs runbook/src/loom_run/goals.rs` prints nothing, and `git grep -n "sdc-bugfix\|BUG-1\|goals.yaml\|idea.yaml" -- runbook/src/declarations.rs runbook/src/loom_run/here.rs runbook/src/loom_run/ports.rs runbook/src/loom_run/run_loop.rs` prints nothing outside test code.
- `--input` is repeatable. A name the runbook did not declare is refused by name before anything is written, with the declared names in the message (`an_input_the_runbook_did_not_declare_is_refused_by_name`); a declared required input that is missing is refused by name (`a_required_input_that_is_missing_is_refused_by_name`); the file or directory is copied, never linked (`an_input_is_copied_to_its_declared_place_and_never_linked`).
- A bug-fix run from a ticket and one from a seed directory leave the same work directory as before. The recorded files `runbook/tests/fixtures/bugfix-runbook/expected/work-from-ticket.txt` and `work-from-seed.txt` hold the sorted listing of the work directory of the root case (path and digest of each file) when the tree opens; the tests `a_ticket_run_leaves_the_recorded_work_directory` and `a_seed_run_leaves_the_recorded_work_directory` compare with them. The first commit records them with the options of the base. The two slow tests of `task test:bugfix-slow` pass.
- Recorded files: a file this story calls recorded is written in the first commit of the story, before any other change, by the named test started with `RUNBOOK_BLESS_EXPECTED=1` (the test writes the file when the variable is set and compares with it otherwise, the way `SDC_BLESS_EXPECTED` works in `driver/tests/sdc_node.rs:319-329`). That commit changes no file under `runbook/src` or `driver/src`, and `git log --oneline <base>..HEAD -- <file>` prints one line: no later commit of the story changes the file.
- The program before the open is the one the runbook declares. The runner starts it in the work directory with a cleared environment and what a bound program gets, and never passes its own environment: the test `the_program_before_the_open_gets_no_variable_of_the_runner` starts the run with a sentinel variable set and reads the names the program printed. It prints nothing of the program on success and its last three lines on a failure, and the text of a ticket reaches the work directory only: the tests of `runbook/tests/loom_run/bugfix.rs` at `:169`, `:212`, `:222`, `:234`, `:250` and `:280` pass with nothing changed in their commands but the spelling of the option, the name of the runbook and the added `--case BUG-1`.
- A run names its runbook: the turn of `sdc` into the bug-fix runbook by an option (`runbook_of()`, the test at `:194`) is gone with the option, and `docs/bugfix.md` shows `runbook run sdc-bugfix --input <name>=<file> --case BUG-1`.
- A requester decider without its goals document is refused as today: the test of `check_settings` in `runbook/tests/loom_run/goals.rs` stays, on the declared input.
- The second runbook declares one file and the test `the_second_runbook_gets_its_declared_input` gives it: the file stands at the declared place when the first action runs.
- `--confinement` is required and has no default, and a file value never lowers a confinement an option or the floor of the daemon sets: the tests of `runbook/tests/loom_run/flags.rs` and `settings.rs` pass with their assertions unchanged, and the declarations file has no member for a confinement (`a_confinement_in_the_declarations_is_an_unknown_member`).
- Every task, script and program of the repository that passed one of the three options passes the new one: `git grep -n -- "--bugfix\|--goals" -- Taskfile.yml scripts driver/src runbook/tests` prints only lines of the test of the refusal.
- The prompts are not touched: `git diff --stat <base> -- driver/src/bin/sdc-node.rs driver/src/bin/sdc-node instructions` prints nothing.
- A `review-result` that reviews this story with the role security and has no open finding of severity blocker or warning exists in the store before the story moves to implemented.
- The cycles keep their results: `task test:cycles-loom` (slow lane, not part of `task check`; started in the background and without the `slot` prefix, as its description says; its task names its time with `SLOT_TIMEOUT=30m`) ends with exit 0 and reports each of the 54 cycles of `cycles/tree-suite.yaml` with the result that file commits for it. `git diff --stat <base> -- cycles/` prints nothing: no expected result is edited to get there.
- `task check` is green at the head of the story.
- The last commit of the unit, made after `main` is merged into it, adds one entry under `[Unreleased]` of `CHANGELOG.md`. No other commit of the story changes that file, and the file is not in the typed scope of the story, so two stories of one wave do not meet on it.
- Tests: no test over 60 s and the test step within 120 s; no model call and no network in a test (stub adapter, sh programs); what is expensive (a built binary, a prepared run) is made once per test run, never per test. Each new test is timed before it is added; a test over 5 s is named with its time in the report. Nothing that stays is Python (Rust or POSIX sh).
- A program, variable, file or format id this story creates for a generic thing carries no `sdc` in its name (epic item 6; the names that exist follow stage 2 of `story:project-rename`).
- The shards of `story:runner-source-names-no-runbook-ratchet` (`runbook/tests/source_names/known/<source path>.txt`) for the source files this story types lose every line this story removes from the source and gain none. `git diff --stat <base> -- runbook/tests/source_names/` lists only those shards, and the report names the lines removed.
- No measured claim of `docs/` or `experiments/` changes. Where a number would change (the time of a node, the count of processes it starts), the measurement is recorded under `experiments/` and listed in `experiments/INDEX.md` before the claim is edited.

## Not in this story

- The daemon: `runbook run --detach` and the command `SubmitRun` take no file input today (the three options conflict with `--detach`), and this story adds none.
- Which runbook an input selects: `story:router-picks-a-runbook-for-an-input`.
- `--conformance`, the option `--step` and the use of the default id of the root case: `story:conformance-and-root-case-from-runbook`. The use of the programs a runbook needs: `story:runner-source-names-no-runbook`.
- The names `idea.yaml`, `goals.yaml` and `dissection.yaml` in `runbook/src/loom_run/decider/`: `story:decisions-routed-by-runbook-data`.
- `--watch` (`runbook/src/loom_run/watch.rs`): a generic option whose helper has `sdc` in its name, item 6.
