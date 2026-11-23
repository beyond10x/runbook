---
format: aep.planning-md/3
id: story:conformance-and-root-case-from-runbook
kind: story
status: proposed
title: 'runbook run loses --conformance and its fixed root case id: a runbook declares both'
summary: A runbook declares programs after a close, switched on with --step <name>[=<mode>], and its default root case id; conformance.rs and the constants IDEA-1 and BUG-1 leave the runner.
tags:
- generic-runner
relations:
- decomposes: epic:runner-knows-no-runbook
- depends_on: story:runner-source-names-no-runbook-ratchet
- depends_on: story:second-runbook-with-model-node
- depends_on: story:run-inputs-declared-by-runbook
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
  path: docs/conformance.md
- confidence: cited
  path: docs/loom-run.md
- confidence: cited
  path: runbook/src/loom_run/conformance.rs
- confidence: cited
  path: runbook/src/loom_run/here.rs
- confidence: cited
  path: runbook/src/loom_run/ports.rs
- confidence: cited
  path: runbook/src/loom_run/run_loop.rs
- confidence: inferred
  path: runbook/tests/fixtures/model-runbook
- confidence: inferred
  path: runbook/tests/loom_run/after_close.rs
- confidence: cited
  path: runbook/tests/loom_run/bugfix.rs
- confidence: cited
  path: runbook/tests/loom_run/conformance.rs
- confidence: cited
  path: runbook/tests/loom_run/mod.rs
- confidence: inferred
  path: runbook/tests/source_names/known/runbook/src/loom_run/here.rs.txt
- confidence: inferred
  path: runbook/tests/source_names/known/runbook/src/loom_run/run_loop.rs.txt
- confidence: inferred
  path: scripts
revision: 13
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T09:57:21Z", actor: "human:operator", revision: 13, executor: "agent:runbook"}
---
## Outcome

`runbook run` has no `--conformance`, and the runner holds no id of a root case. A runbook declares the programs the runner may start after a run closed, and the default id of its root case, in the declarations file that `story:run-inputs-declared-by-runbook` created. A person switches a declared program on with `--step <name>[=<mode>]`. The SDC runbook declares the conformance observation as such a program and `IDEA-1` as its root case; the bug-fix runbook declares `BUG-1`. The runner no longer links the conformance code of SDC.

This story uses two members of the declarations and writes the values of SDC; it does not change the format or the reader.

## Source

- `epic:runner-knows-no-runbook`, item 3 ("`--conformance` become inputs and actions a runbook declares; the root case id and whether a protocol can be started come from the protocol").
- Read at `main` `826b09c7` (the cited lines hold at `7a9efbc0`: of the paths read, `runbook/src/commands.rs` and tests of the daemon differ, and `runbook/src/check.rs` and `runbook/tests/cli.rs` changed from line 397 and from line 665 on, below every line cited here):
  - `runbook/src/loom_run/here.rs:103-105`: `--conformance` with the modes `off` (default) and `observe`. `runbook/src/loom_run/conformance.rs` (61 lines): `after_close()` (`:38-61`) calls `sdc_tools::conform::observe` with the pin `driver/ess-pin.json` (`:54`), the tool `bindings/tools/sdc-tool.sh` (`:28`) and the helper `sdc-conform-adapter` (`:31`). The head of the file states: the observation decides nothing, the outcome and the exit of the run are as they were. It is called at `runbook/src/loom_run/run_loop.rs:164`. The same observation has a command line of its own, the program `sdc-conform`.
  - `runbook/src/loom_run/here.rs:18`: `DEFAULT_CASE = "IDEA-1"`, the default of `--case`. `runbook/src/loom_run/bugfix.rs:25`: `CASE = "BUG-1"`, taken when a bug-fix option is given and `--case` was left at its default (`case_of()`).
  - `runbook/Cargo.toml:22` links the SDC library (`sdc_tools`); `git grep -c "sdc_tools::" -- runbook/src/loom_run/conformance.rs` prints 2.

## Domain relations

- Runbook -> program after a close: zero to many, each with a name a person asks for; the runbook owns the declaration; the program never changes the outcome of the run. inferable (inferred from `runbook/src/loom_run/conformance.rs:38-61`; no `ess/1` document declares it).
- Runbook -> default id of the root case: zero or one. inferable (inferred from `runbook/src/loom_run/here.rs:18` and `runbook/src/loom_run/bugfix.rs:25`, one constant per runbook).
- Where a runbook states both: the declarations file of `story:run-inputs-declared-by-runbook` (the working answer to `decision-blocker:runbook-declarations-without-a-format-member`, which is open and blocks this story).

## Names taken in this draft

The option `--step <name>[=<mode>]` is a default of the coordinator, written here so that the operator may rename it before the build. For SDC the declared name is `conformance` and its mode `observe`, so `--conformance observe` becomes `--step conformance=observe`.

## Scope

- `runbook/src/loom_run/here.rs`, `runbook/src/loom_run/ports.rs`, `runbook/src/loom_run/run_loop.rs`; `runbook/src/loom_run/conformance.rs` (goes). The reader `runbook/src/declarations.rs` is called and not changed.
- The values in the declarations files of the two SDC runbooks (typed when the blocker closes); `runbook/tests/fixtures/model-runbook/` (a root case id and one program after a close).
- `runbook/tests/loom_run/conformance.rs`, new `runbook/tests/loom_run/after_close.rs` (with its line in `runbook/tests/loom_run/mod.rs`), `runbook/tests/loom_run/bugfix.rs` (the commands lose `--case BUG-1`).
- `docs/conformance.md`, `docs/cli.md`, `docs/bugfix.md`, `docs/loom-run.md`; `Taskfile.yml` and `scripts/` where one passes `--conformance` or `--case BUG-1`; the shards of `here.rs` and `run_loop.rs`.
- Overlap, by story:
  - `story:run-inputs-declared-by-runbook`: edge. It creates the format and the reader, deletes `bugfix.rs` with `BUG-1` (`:25`) and `case_of()` (`:65`), and changes `ports.rs`, `here.rs`, `run_loop.rs`, the fixture and `runbook/tests/loom_run/mod.rs` first. Until this story is in, a bug-fix run is started with `--case BUG-1`; this story takes that away again.
  - `story:decisions-routed-by-runbook-data`: it has an edge to this story (the fixture, `runbook/tests/loom_run/mod.rs`).
  - `story:runner-source-names-no-runbook`: it has an edge to this story. `story:node-classes-file-names-no-action` follows this story through its edge to `story:decisions-routed-by-runbook-data` (`docs/loom-run.md`).
  - `story:node-program-runs-steps-a-binding-names`: `Taskfile.yml`; no edge, not in one wave with this story because both type the file. `story:node-program-built-ins-removed`: no file in common.
  - `story:run-without-model-node-needs-no-caps`, `story:runbook-toml-carries-defaults`, `story:runbooks-load-from-configured-paths` (active; `here.rs`, `run_loop.rs`): ordered through the edge to `story:run-inputs-declared-by-runbook`, which has an edge to each of the three; all three land first.
  - `story:runbooks-load-from-configured-paths` already owns the other half of item 3 of the epic: its acceptance says "The fixed list of startable names in `list.rs` is gone: a protocol says itself whether it can be started". This story does not claim that half.
  - `story:runner-records-action-times-and-output`, `story:loom-run-records-its-claims` (active, no typed scope, both change `run_loop.rs`): edges; both are in build now and land first.
  - Drafts with no typed scope, each not built in the same wave as this story: `story:decider-inputs-read-once` and `story:replay-plan-out-of-the-run-copy` (`docs/loom-run.md`, `runbook/tests/loom_run/mod.rs`), `story:replay-lane-budget-and-trigger` (`Taskfile.yml`).

## Acceptance

- `runbook run --help` lists `--step` and not `--conformance`. `--conformance`, when given, is refused as a usage error before anything is written, with one line that names the new spelling (`--step <name>[=<mode>]`): the test `the_old_conformance_option_is_refused_with_the_new_spelling`.
- `git ls-files runbook/src/loom_run/conformance.rs` prints nothing, and `git grep -n "sdc_tools::conform\|ess-pin\|IDEA-1\|BUG-1" -- runbook/src` prints nothing outside test code (`bugfix.rs`, the other place of `BUG-1`, is gone since `story:run-inputs-declared-by-runbook`).
- A program after a close runs only when the person asks for it by its declared name, and only after a run that ended closed; with no `--step` nothing runs, as with `off` before. A name or a mode the runbook did not declare is refused by name before anything is written. Its exit never changes the outcome or the exit code of the run. Tests in `runbook/tests/loom_run/after_close.rs`: `no_step_runs_unless_it_is_asked_for`, `a_step_the_runbook_did_not_declare_is_refused_by_name`, `a_step_does_not_run_after_a_run_that_did_not_close`, `a_failing_step_changes_neither_outcome_nor_exit_code` (an sh program that ends with 1; the test reads the same exit code of `runbook run` and one line on standard error).
- The conformance observation of SDC, asked for with `--step conformance=observe`, writes the same `oracle.json` into the run directory as `--conformance observe` did: the tests of `runbook/tests/loom_run/conformance.rs` pass with nothing changed but the spelling of the option in their commands.
- `runbook run sdc` without `--case` opens `IDEA-1` and a bug-fix run opens `BUG-1`, from the declarations (`the_root_case_of_a_run_is_the_one_its_runbook_declares`). A runbook that declares no root case id needs `--case`: without it the run is refused as a usage error before anything is written (`a_runbook_without_a_root_case_needs_the_case_option`). An explicit `--case` wins (`an_explicit_case_is_kept`, `runbook/tests/loom_run/bugfix.rs:204`, passes unchanged). The second runbook declares one: the test `the_second_runbook_opens_its_declared_root_case` (`runbook/tests/loom_run/after_close.rs`) starts `runbook run` on the fixture without `--case` and reads the id of the root case the run directory records (the id the fixture declares) and exit code 0.
- The commands that `story:run-inputs-declared-by-runbook` gave `--case BUG-1` lose it: `git grep -n -- "--case BUG-1\|\"BUG-1\"" -- Taskfile.yml scripts docs/bugfix.md runbook/tests/loom_run/bugfix.rs` prints only the lines of the test of an explicit case.
- The program after a close gets a cleared environment and what a bound program gets; the runner never passes its own environment: the test `a_step_after_the_close_gets_no_variable_of_the_runner` starts the run with a sentinel variable set and reads the names the program printed.
- `--confinement` is required and has no default, and a file value never lowers a confinement an option or the floor of the daemon sets: the tests of `runbook/tests/loom_run/flags.rs` and `settings.rs` pass with their assertions unchanged.
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

- Whether a protocol can be started: `story:runbooks-load-from-configured-paths`.
- The observation itself (`driver/src/conform.rs`) and its tables.
- The daemon: a run of the daemon takes no program after a close in this story.
