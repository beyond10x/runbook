---
format: aep.planning-md/3
id: story:runner-source-names-no-runbook
kind: story
status: proposed
title: No shard of runbook names is left, and the check refuses every hit
summary: The names left in doctor, assets, check, replay and the run directory writer go; the last shards of the ratchet are deleted and the check gets strict.
tags:
- generic-runner
relations:
- decomposes: epic:runner-knows-no-runbook
- depends_on: story:runner-source-names-no-runbook-ratchet
- depends_on: story:second-runbook-with-model-node
- depends_on: story:runner-finds-model-node-by-binding
- depends_on: story:implement-reply-leaves-node-program
- depends_on: story:node-classes-file-names-no-action
- depends_on: story:run-inputs-declared-by-runbook
- depends_on: story:conformance-and-root-case-from-runbook
- depends_on: story:decisions-routed-by-runbook-data
- depends_on: story:runbooks-load-from-configured-paths
- depends_on: story:node-program-built-ins-removed
- depends_on: story:runner-records-action-times-and-output
- depends_on: story:loom-run-records-its-claims
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: docs/cli.md
- confidence: cited
  path: docs/loom-bindings.md
- confidence: cited
  path: runbook/src/assets.rs
- confidence: cited
  path: runbook/src/check.rs
- confidence: cited
  path: runbook/src/doctor.rs
- confidence: cited
  path: runbook/src/loom_run/replay.rs
- confidence: cited
  path: runbook/src/loom_run/run_dir.rs
- confidence: cited
  path: runbook/tests/cli.rs
- confidence: inferred
  path: runbook/tests/fixtures/doctor
- confidence: inferred
  path: runbook/tests/source_names
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T09:57:21Z", actor: "human:operator", revision: 12, executor: "agent:runbook"}
---
## Outcome

The source of the runner and of the node program names no action, no artifact and no protocol of a runbook. The last shards of the ratchet are deleted, and the check refuses every hit. What the stories before this one did not remove is removed here: the names in `runbook doctor`, in the detection of a checkout, in `runbook check`, in the replay and in the run directory writer.

This closes the ratchet that `story:runner-source-names-no-runbook-ratchet` opened and is the last story of `epic:runner-knows-no-runbook`.

## Source

- `epic:runner-knows-no-runbook`, item 5 and acceptance line 1 ("The check of item 5 is in `task check` and green").
- Read at `main` `826b09c7` (the cited lines hold at `3c0eb863`, release 0.7.0: of the paths read, only `runbook/src/commands.rs` and tests of the daemon differ), the names no other story of the epic removes:
  - `runbook/src/doctor.rs` (381 lines): the scopes `Sdc` and `Bugfix`, chosen by the names `sdc` and `sdc-bugfix` (`:23-24`); the table `SPECS`, whose rows say which runbook needs a program (`sdc:`, `sdc-bugfix:`, `:79-109`).
  - `runbook/src/assets.rs` (342 lines): a directory is a checkout when it holds `protocol/` and `driver/examples/sdc/` (`:182`); the layout parts `driver/examples/sdc/` (`:73`) and `bindings/tools/sdc-tool.sh` (`:81`).
  - `runbook/src/check.rs` (627 lines): the JSON copies of the bindings under `driver/examples/sdc` (`:204`, `:232`, `:285`, `:294`, `:376`).
  - `runbook/src/loom_run/replay.rs:45`: the tool of SDC by name (`PROGRAM_TOOL = "sdc-tool.sh"`). `runbook/src/loom_run/run_dir.rs:782-784`: `work/findings.yaml`.
  - `runbook/src/list.rs:7`: `ROOT_RUNBOOKS = ["sdc", "sdc-bugfix"]`, which `story:runbooks-load-from-configured-paths` (active) removes.
  - `runbook/Cargo.toml:22`: the runner links the SDC library (`sdc_tools`); `git grep -c "sdc_tools::" -- runbook/src` counts 17 lines in five files of `loom_run/`.
  - The tests of `runbook doctor` today: `runbook/tests/cli.rs:140-334`, among them `doctor_for_one_runbook_checks_only_what_it_needs` (`:207`).

## Domain relations

- Runbook -> program it needs: zero to many; the programs its bindings start and the programs its bindings name as steps can be read from the bindings, the ones its own tools call cannot. inferable for the first half (inferred from `runbook/src/doctor.rs:79-109`, the column `by` of the table; no `ess/1` document declares it). The second half is the member for the programs a runbook needs in the declarations file that `story:run-inputs-declared-by-runbook` creates (the working answer to `decision-blocker:runbook-declarations-without-a-format-member`, which is open and blocks this story).

## Scope

- `runbook/src/doctor.rs`, `runbook/src/assets.rs`, `runbook/src/check.rs`, `runbook/src/loom_run/replay.rs`, `runbook/src/loom_run/run_dir.rs`. The reader `runbook/src/declarations.rs` is called and not changed; no file of the node program (its shards are gone since `story:node-program-built-ins-removed`).
- The member for the programs a runbook needs in the declarations files of the two SDC runbooks (typed when the blocker closes).
- `runbook/tests/source_names/` (the last shards go, the check gets strict), `runbook/tests/cli.rs`, the recorded files under `runbook/tests/fixtures/doctor/`.
- `docs/cli.md`, `docs/loom-bindings.md` (the fixture of `story:second-runbook-with-model-node` as the worked example), `AGENTS.md`.
- Overlap, by story:
  - Every other story of the set has an edge from this one, direct or through another: `story:second-runbook-with-model-node`, `story:runner-finds-model-node-by-binding` (`replay.rs`, `run_dir.rs`), `story:node-program-built-ins-removed` (the shards of the node program, `docs/loom-bindings.md`), `story:node-classes-file-names-no-action`, `story:run-inputs-declared-by-runbook` (the reader), `story:conformance-and-root-case-from-runbook`, `story:decisions-routed-by-runbook-data`. It is built last and alone.
  - `story:runbooks-load-from-configured-paths` (active; `assets.rs`, `list.rs`): edge; it lands first and removes `ROOT_RUNBOOKS`. Its acceptance has `runbook doctor --for <name>` and `runbook check` on a runbook of a configured directory; this story keeps both.
  - `story:one-copy-of-the-bindings` (draft, no typed scope): it removes the JSON copies that `check.rs` and `assets.rs` name. Precondition: not in one wave with this story; when it is in first, the lines `check.rs:204,232,285,294,376` and `assets.rs:73` are gone already and this story has less to do.
  - `story:runner-records-action-times-and-output`, `story:loom-run-records-its-claims` (active, no typed scope, both change `run_dir.rs`): edges; both are in build now and land first.
  - `story:node-program-runs-steps-a-binding-names` (`runbook/src/doctor.rs`, the row of `sdc-step`; `docs/loom-bindings.md`): ordered through the edge to `story:node-program-built-ins-removed`.
  - Drafts with no typed scope, each not built in the same wave as this story: `story:decider-inputs-read-once` (`runbook/src/loom_run/run_dir.rs`), `story:replay-plan-out-of-the-run-copy` (`runbook/src/loom_run/replay.rs`), `story:held-rows-name-existing-tests` (`runbook/src/check.rs`, `runbook/tests/cli.rs`), `story:replay-lane-budget-and-trigger` (`AGENTS.md`).
  - `story:project-rename` (active): stage 2 renames what is left with `sdc` in a generic name, after this story.

## Acceptance

- `git ls-files runbook/tests/source_names/known` prints nothing. The test `no_runbook_name_in_runner_or_node_source` fails on any hit; `a_planted_name_is_reported_as_new` still proves that it does; the comparison with the merge base is gone with the shards.
- `runbook doctor --for <name>` reads what a runbook needs from the runbook: the programs its bindings start, the programs its bindings name as steps, and the programs its declarations name. `git grep -n "\"sdc\"\|\"sdc-bugfix\"\|sdc:\|sdc-bugfix:" -- runbook/src/doctor.rs` prints nothing outside test code.
- For `sdc` and `sdc-bugfix` it prints the rows it printed. The recorded files `runbook/tests/fixtures/doctor/rows-sdc.txt` and `rows-sdc-bugfix.txt` hold the rows of `runbook doctor --for sdc` and `--for sdc-bugfix` on one prepared `PATH`; the test `doctor_prints_the_recorded_rows_for_the_two_sdc_runbooks` (`runbook/tests/cli.rs`) compares, and the rows are exactly the recorded ones. The recorded files hold the row of `sdc-step` already: `story:node-program-runs-steps-a-binding-names` adds it to the table of `runbook/src/doctor.rs` (the test at `:352` asks for a row per program of the driver crate), and the files are recorded at the base of this story, which lies after that story. This story changes where the row comes from (the steps the bindings name), not the row. The tests of `runbook/tests/cli.rs:140-334` pass with their assertions unchanged.
- Recorded files: a file this story calls recorded is written in the first commit of the story, before any other change, by the named test started with `RUNBOOK_BLESS_EXPECTED=1` (the test writes the file when the variable is set and compares with it otherwise, the way `SDC_BLESS_EXPECTED` works in `driver/tests/sdc_node.rs:319-329`). That commit changes no file under `runbook/src` or `driver/src`, and `git log --oneline <base>..HEAD -- <file>` prints one line: no later commit of the story changes the file.
- For the second runbook `runbook doctor --for` names the sh check of the fixture (`doctor_for_the_second_runbook_names_its_check_program`).
- `runbook check`, the detection of a checkout and the replay hold no path and no tool with the name of a runbook: `git grep -n "driver/examples/sdc\|sdc-tool" -- runbook/src/assets.rs runbook/src/check.rs runbook/src/loom_run/replay.rs` prints nothing outside test code (`PROGRAM_TOOL`, `replay.rs:45`, and the layout parts `assets.rs:73,81` among them). The parts are found through the runbook: the test `the_layout_parts_of_a_runbook_are_found_in_a_checkout_with_other_names` arranges the fixture of the second runbook as a checkout, and `runbook doctor` reports each part of its layout as present and the one the test removes as missing.
- `runbook check` on this checkout prints the same findings as before (none), and the replay wraps a program action by the classification of `story:runner-finds-model-node-by-binding`: the tests of `runbook check` and of `runbook/tests/loom_run/replay.rs` and `replay_boundaries.rs` pass with their assertions unchanged.
- `git grep -n "findings.yaml" -- runbook/src/loom_run/run_dir.rs` prints nothing outside test code.
- The runner links no part of the SDC library that is SDC only: `git grep -n "sdc_tools::conform\|sdc_tools::reply::review\|sdc_tools::reply::implement" -- runbook/src` prints nothing, and `git grep -c "sdc_tools::" -- runbook/src` sums to no more than 15 (17 at `826b09c7`, less the 2 of `conformance.rs`).
- `git grep -n "model-runbook" -- docs/loom-bindings.md` prints at least one line: the fixture is the worked example of a runbook with a model node.
- The prompts of the 14 model nodes are byte-identical before and after: the test `the_prompt_of_each_agent_node_is_its_golden_byte_for_byte` (`driver/tests/bugfix_prompt.rs:214`) passes, `git diff --stat <base> -- instructions/golden/` prints nothing (25 files under `instructions/golden/llm-node/`), and no step of the story runs with `SDC_BLESS_PROMPTS=1`. Where the test has to start a node in a new way, only its harness changes, never a golden file.
- The cycles keep their results: `task test:cycles-loom` (slow lane, not part of `task check`; started in the background and without the `slot` prefix, as its description says; its task names its time with `SLOT_TIMEOUT=30m`) ends with exit 0 and reports each of the 54 cycles of `cycles/tree-suite.yaml` with the result that file commits for it. `git diff --stat <base> -- cycles/` prints nothing: no expected result is edited to get there.
- `task check` is green at the head of the story.
- The last commit of the unit, made after `main` is merged into it, adds one entry under `[Unreleased]` of `CHANGELOG.md`. No other commit of the story changes that file, and the file is not in the typed scope of the story, so two stories of one wave do not meet on it.
- Tests: no test over 60 s and the test step within 120 s; no model call and no network in a test (stub adapter, sh programs); what is expensive (a built binary, a prepared run) is made once per test run, never per test. Each new test is timed before it is added; a test over 5 s is named with its time in the report. Nothing that stays is Python (Rust or POSIX sh).
- A program, variable, file or format id this story creates for a generic thing carries no `sdc` in its name (epic item 6; the names that exist follow stage 2 of `story:project-rename`).
- No measured claim of `docs/` or `experiments/` changes. Where a number would change (the time of a node, the count of processes it starts), the measurement is recorded under `experiments/` and listed in `experiments/INDEX.md` before the claim is edited.

## Not in this story

- Names that say `sdc` for a generic thing (`sdc-node`, `sdc-watch`, `sdc-replay`, `SDC_*`, `sdc.*` format ids, the directory `.sdc/`): item 6, stage 2 of `story:project-rename`. The check of this epic does not count them.
- The library `driver/src/reply/` and the other programs of the driver crate: they are SDC and stay.
- The fixed list of startable names: `story:runbooks-load-from-configured-paths`.
