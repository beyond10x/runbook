---
format: aep.planning-md/3
id: story:second-runbook-with-model-node
kind: story
status: proposed
title: A second runbook with a model node runs through runbook run and runbook test with no Rust written for it
summary: A hand-written fixture with invented text (protocol, bindings, instruction, stub reply, suite) runs with the stub adapter and no model call; the proof of the epic.
tags:
- generic-runner
relations:
- decomposes: epic:runner-knows-no-runbook
- depends_on: story:runner-source-names-no-runbook-ratchet
- depends_on: story:binding-states-model-node-inputs-and-output
- depends_on: story:runner-finds-model-node-by-binding
scope:
- confidence: inferred
  path: runbook/tests/fixtures/model-runbook
- confidence: cited
  path: runbook/tests/loom_run/mod.rs
- confidence: inferred
  path: runbook/tests/loom_run/model_runbook.rs
- confidence: cited
  path: runbook/tests/test_suite/mod.rs
- confidence: inferred
  path: runbook/tests/test_suite/model_runbook.rs
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T06:01:19Z", actor: "human:operator", revision: 10, executor: "agent:runbook", correlation: "epic-runner-knows-no-runbook-critics-3"}
---
## Outcome

A second runbook with a model node exists and runs. It is written by hand with invented text: a protocol (`example@1`, or another invented id where that one is taken), bindings with one model node and one check that is an sh program, an instruction text, a stub reply and a suite file. It runs through `runbook run` and through `runbook test` with the stub adapter, with no model call, and no line of Rust was written for it.

This is the proof of the epic. The check of `story:runner-source-names-no-runbook-ratchet` reads the names of this runbook like those of SDC, and no shard ever holds one of them.

## Source

- `epic:runner-knows-no-runbook`, item 5 ("A second runbook with a model node, written by hand with invented text, runs through `runbook run` and `runbook test` with a stub adapter") and the outcome ("a second runbook with model nodes runs without a line of Rust written for it").
- Read at `main` `826b09c7` (the cited lines hold at `3c0eb863`, release 0.7.0: of the paths read, only `runbook/src/commands.rs` and tests of the daemon differ):
  - No protocol other than those of SDC has a model node. The fixtures `runbook/tests/fixtures/test-suite/` (protocols `suite-demo`, `suite-amend`), `runbook/tests/fixtures/engine-runbook/` (`engine-demo`, `engine-draft`) and `runbook/tests/fixtures/bugfix-runbook/` bind sh programs only.
  - A model node of a runbook the node program does not know cannot run: `driver/src/bin/sdc-node/nodes.rs:219` ends with `llm-node: no agent node <action>` (exit 64).
  - `runbook run` takes a runbook from outside the built-in set with `--bindings DIR`, `--protocols DIR`, `--root-protocol NAME@REVISION` and `--case ID` (`runbook/src/loom_run/here.rs`); `runbook test <suite> --confinement <level>` runs a suite through the suite runner of Loom in this process (`runbook/src/test_suite.rs`, 249 lines) and reads the bindings of the suite itself, without the copy the runner makes for `runbook run`.
  - The protocol id `example@1` is used so far only by a hand-written run record of the UI tests (`ui/test/loom-run-fixture.mjs:68`).
  - Under `runbook test` a node reads the tools, the adapters and the instructions of the checkout the node program was built in (`driver/src/bin/sdc-node/setup.rs:148-160`, `driver/src/bin/sdc-node.rs:79-85,149-153,242-246`). `story:binding-states-model-node-inputs-and-output` changes that and holds it with the test `a_node_finds_the_tools_of_its_own_runbook`; this story uses the result and changes no Rust.

## Scope

- New: `runbook/tests/fixtures/model-runbook/` (protocol file, bindings file, the sh launcher beside the bindings that the form of `story:binding-states-model-node-inputs-and-output` asks for, instruction text, stub reply, the sh check, a suite file in `loom.tree-suite/1`).
- New test modules of the one test binary `runbook/tests/cli.rs`: `runbook/tests/loom_run/model_runbook.rs` (with its line in `runbook/tests/loom_run/mod.rs`) and `runbook/tests/test_suite/model_runbook.rs` (with its line in `runbook/tests/test_suite/mod.rs`).
- No file under `runbook/src` or `driver/src`, and no file under `docs/` (`story:runner-source-names-no-runbook` names the fixture in `docs/loom-bindings.md` as the worked example).
- Overlap, by story:
  - `story:binding-states-model-node-inputs-and-output`, `story:runner-finds-model-node-by-binding`: edges; this story needs both.
  - `story:run-inputs-declared-by-runbook`, `story:conformance-and-root-case-from-runbook`, `story:decisions-routed-by-runbook-data`: each adds its part to the fixture and to `runbook/tests/loom_run/mod.rs` and has an edge to this story (the last two through the first).
  - `story:node-program-runs-steps-a-binding-names` and the stories that bind steps: no file in common; the two do not meet on a line, so this story can be built beside them.
  - `story:runbooks-load-from-configured-paths` (active): it starts a runbook by name from a configured directory; this story starts the fixture with `--bindings` and `--protocols` and changes no file of that story.
  - `story:decider-inputs-read-once` and `story:replay-plan-out-of-the-run-copy` (drafts, no typed scope; each names `runbook/tests/loom_run/` as a whole): not built in the same wave as this story, which adds one line to `runbook/tests/loom_run/mod.rs`.

## Acceptance

- The test `a_second_runbook_with_a_model_node_runs` (`runbook/tests/loom_run/model_runbook.rs`) starts `runbook run` on the fixture with `--bindings`, `--protocols`, `--root-protocol`, `--case`, `--confinement local`, the stub adapter and caps that allow one call. The run ends closed; the run directory holds the record of one call of the node; the output artifact holds the text of the stub reply; the check that follows the node passed on it.
- The test `a_second_runbook_with_a_model_node_passes_its_suite` (`runbook/tests/test_suite/model_runbook.rs`) runs `runbook test <suite> --confinement local` on the suite of the fixture and every cycle of the suite ends as expected (the last line reads `N of N as expected`).
- The suite has a cycle named `check-fails-once` in which the sh check refuses the first reply. The test `the_retry_feedback_stands_in_the_second_request` reads the two requests of that cycle: the node was called twice, and the second request holds, byte for byte, the line the sh check printed on its refusal (an invented sentence that stands in the fixture once), which the first request does not hold.
- Both run tests fail at `826b09c7` (the node ends with exit 64) and pass once `story:binding-states-model-node-inputs-and-output` and `story:runner-finds-model-node-by-binding` are in.
- The runner counts the node as a model node. The test `the_runner_counts_the_node_of_the_second_runbook_as_a_model_node` reads three things of the run of the first test: the records of the run state one call for the action, the ledger holds one reservation for the run, and the exit code is 0. The test `the_second_runbook_starts_no_call_when_the_ledger_has_nothing_left` starts the same run on a ledger whose reservations are as large as the cap, the way the last part of `a_reservation_left_by_a_killed_run_reduces_what_is_left_and_never_raises_it` does (`runbook/tests/loom_run/spend.rs:71-78`): the exit code is 4 (`EXIT_SPEND_CAP`, `runbook/src/loom_run/mod.rs:138`), the outcome of the run is `stopped: spend cap`, the records state no call, the call log of the stub adapter is empty and the ledger holds no reservation of this run (the spend cap fails closed).
- `git grep -n` for the action id, the artifact path and the protocol id of the fixture prints nothing under `runbook/src` and `driver/src`. `git diff --stat <base> -- runbook/src driver/src docs` prints nothing: a fault the fixture uncovers in the runner or the node program is fixed in a story of its own and named in the report.
- The sh check of the fixture writes the names of its environment into its output and the test `the_check_of_the_second_runbook_gets_path_and_loom_names_only` reads `PATH` and `LOOM_*` and nothing else: the program contract of Loom is unchanged.
- `--confinement` is given in every command of the tests; it is required and has no default (`git grep -c -- "--confinement" -- runbook/tests/loom_run/model_runbook.rs runbook/tests/test_suite/model_runbook.rs` counts one for each command the two files start).
- All text of the fixture is invented: no name of a person, an organisation or a customer, no home path.
- The names of the fixture are read by the check of `story:runner-source-names-no-runbook-ratchet`; no shard holds one of them.
- The prompts of the 14 model nodes are byte-identical before and after: the test `the_prompt_of_each_agent_node_is_its_golden_byte_for_byte` (`driver/tests/bugfix_prompt.rs:214`) passes, `git diff --stat <base> -- instructions/golden/` prints nothing (25 files under `instructions/golden/llm-node/`), and no step of the story runs with `SDC_BLESS_PROMPTS=1`. Where the test has to start a node in a new way, only its harness changes, never a golden file.
- The cycles keep their results: `task test:cycles-loom` (slow lane, not part of `task check`; started in the background and without the `slot` prefix, as its description says; its task names its time with `SLOT_TIMEOUT=30m`) ends with exit 0 and reports each of the 54 cycles of `cycles/tree-suite.yaml` with the result that file commits for it. `git diff --stat <base> -- cycles/` prints nothing: no expected result is edited to get there.
- `task check` is green at the head of the story.
- The last commit of the unit, made after `main` is merged into it, adds one entry under `[Unreleased]` of `CHANGELOG.md`. No other commit of the story changes that file, and the file is not in the typed scope of the story, so two stories of one wave do not meet on it.
- Tests: no test over 60 s and the test step within 120 s; no model call and no network in a test (stub adapter, sh programs); what is expensive (a built binary, a prepared run) is made once per test run, never per test. Each new test is timed before it is added; a test over 5 s is named with its time in the report. Nothing that stays is Python (Rust or POSIX sh).
- A program, variable, file or format id this story creates for a generic thing carries no `sdc` in its name (epic item 6; the names that exist follow stage 2 of `story:project-rename`).
- No shard of `story:runner-source-names-no-runbook-ratchet` changes: `git diff --stat <base> -- runbook/tests/source_names/` prints nothing.
- No measured claim of `docs/` or `experiments/` changes. Where a number would change (the time of a node, the count of processes it starts), the measurement is recorded under `experiments/` and listed in `experiments/INDEX.md` before the claim is edited.

## Not in this story

- A runbook of a consumer (not in the epic).
- An input given at the start, a program after the close and a decision class of this runbook: `story:run-inputs-declared-by-runbook`, `story:conformance-and-root-case-from-runbook` and `story:decisions-routed-by-runbook-data` each add their part to this fixture.
- Starting the runbook by name from a configured directory: `story:runbooks-load-from-configured-paths`.
