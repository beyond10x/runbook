---
format: aep.planning-md/3
id: story:runbook-loom-run-command
kind: story
status: draft
title: 'runbook loom-run: one case tree through b10x-loom with a spend ledger and caps'
summary: The runner that drives one Runbook case tree through the b10x-loom binary as a child process, with caps and a shared spend ledger
relations:
- decomposes: epic:runbook-one-binary-and-daemon
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: docs/loom-run.md
- confidence: cited
  path: runbook/Cargo.lock
- confidence: cited
  path: runbook/Cargo.toml
- confidence: cited
  path: runbook/src/loom_run
- confidence: cited
  path: runbook/src/main.rs
- confidence: cited
  path: runbook/tests/cli.rs
- confidence: cited
  path: runbook/tests/loom_run
revision: 5
---
# Story: runbook-loom-run-command

## Outcome

A person (or a later experiment runner) starts one case tree of a Runbook protocol set with one
command, `runbook loom-run`, and gets a run directory, an outcome and a spend that never passes the
cap, whatever the number of runs that share the ledger.

## Context

The eval of Runbook on Loom needs a runner that drives `b10x-loom` as a child process with model
calls in the loop. Three units build it in parallel on the files named in the interfaces document of
wave rb3: this story is the core (the loop, the caps, the ledger, the child environment); the decider
and the bindings copy, and the run directory writer, are the stories of the other two units.
`docs/loom-run.md` holds the behaviour; the Loom facts it relies on (exit codes of `tree run`, the
four stable members of `decisions[]`, the cost source under `.sdc/llm/`) are the coordinator's brief
of 2026-11-22.

## Acceptance

- `runbook loom-run` refuses with exit 2 and writes nothing when `--loom` is not the file `--loom-sha256`
  names or when `<runs-dir>/<name>` exists; the run directory holds neither a login file's content nor
  its path (tests in `runbook/tests/loom_run/core.rs`).
- The child of `b10x-loom` gets a built environment of `PATH`, `HOME` and `XDG_DATA_HOME` only, and
  `--confinement` is required with no default and passed unchanged to every `tree run`.
- The loop runs `tree run --max-steps 1`, answers each decision through the decider by running its
  printed `command` as data, and ends `denied` when a denied decision returns, `error` with the exit
  code and the stderr tail when a printed command fails.
- The spend cap holds for two processes that share one ledger (0.60 USD per call against a cap of
  2.00 USD never books more than 2.00 USD); the wall-time, model-call and signal caps end the run with
  their own outcome and exit code.
- The run fails closed on an unreadable or invalid ledger, a decider answer without a cost, an unknown
  `b10x-loom` exit code, `tree run` output that is not JSON and a model step without a cost record.
- The end of a run stores `tree status` and `tree verify`, and a non-zero verify makes the outcome `error`.
- The tests of the crate run against a fake `b10x-loom` the tests generate: no model call, no test over
  60 s and the crate's test step within 120 s.

## Out of Scope

- The decider kinds that decide (rule, model) and the rewrite of the bindings copy by settings (the
  decider story of the same wave).
- The run directory files that `sdc-experiment report` and `runbook parity summarize` read (the run
  directory writer story).
- Any run against a model; the first real run is a separate step with its own cap.

## Ambiguities

- A SIGKILL of the runner leaves a reservation in the ledger. `requires-stakeholder-input`: the design
  owner decides whether a stale reservation expires or is reset by hand; until then it counts against the cap.
- One model call can overshoot its `--max-budget-usd` by what the model tool allows. `inferable`: the
  ledger books the real cost and the overshoot is documented in `docs/loom-run.md`.

## Open Questions

None beyond the ambiguities above.

## The run directory (unit R3, `rb3-loom-run-dir`)

Tests: `runbook/tests/loom_run/run_dir.rs`; page: `docs/loom-run-directory.md`.

- From the events of a small complete run (two model steps with costs, one decision answered by the
  model decider with a cost, outcome delivered), the writer makes a run directory that the contract's
  rules read as finished, closed, delivered and clean, with `run.cost_usd` equal to the sum of the
  costs of the events and `run.llm_calls` 2, `run.decider_calls` 1: test
  `a_small_complete_run_reads_as_closed_delivered_and_clean_with_the_cost_of_the_events`.
- From a run stopped by the spend cap it makes a directory read as finished, not closed, not
  delivered, with `run-stopped.json` and the cost spent before the stop: test
  `a_run_stopped_by_the_spend_cap_is_finished_not_closed_and_not_delivered`.
- A run that is killed leaves whole lines in every append-only file and a case store the reader takes
  as open: tests `a_run_that_is_killed_leaves_a_directory_the_reader_takes_as_open` and
  `the_files_are_appended_line_by_line_as_the_events_arrive`.
- No file the writer makes holds a login path, a marker of a login file's content, a token, a path
  under a home directory or an environment variable: test
  `a_run_dir_holds_no_login_path_no_marker_no_home_path_and_no_environment`.
- `runbook parity summarize --loom-status <run>/loom/status.json --product-tree <run>/work` on a
  directory the writer made from the committed fixture status prints the fixture's counts line: test
  `the_run_directory_names_the_files_of_parity_summarize_and_the_counts_line_is_the_fixtures`.
- `docs/loom-run-directory.md` lists, per member of the contract, its source in a Loom run, the
  members a Loom run does not record with the measure each affects, and, per row of
  `experiments/release-0.1.2/report.md`, whether a Loom run directory gives it.

## What the core gives the run directory writer (requests of unit R3)

- `RunStarted` settings carry `loom{sha256, confinement, cap_micro_usd}`, `max_llm_calls`,
  `max_wall_seconds`, `case` and `root_protocol`, and not the binary's path: test
  `run_started_settings_carry_what_the_command_was_given_and_not_the_binary_path`.
- The protocol files are copied into `<run>/protocols/` before `tree open`: test
  `the_protocol_files_are_copied_into_the_run_directory_before_tree_open`.
- `events.jsonl`, `outcome.json`, `tree-status.json` and `tree-verify.json` name no path of the
  runs directory: test `the_files_the_runner_writes_name_no_path_of_the_runs_directory`.
- `tree-status.json` is refreshed after every step through a temporary file and a rename: test
  `the_status_is_refreshed_after_every_step_and_never_left_half_written`.
- A `tree status` the binary refuses ends the run as `error` with its exit code and stderr (a store is
  only readable with the protocol catalog of its run): test
  `a_status_the_binary_refuses_ends_the_run_with_its_exit_code_and_stderr`.

## Open requests

- `DecisionAnswered` does not yet carry the model name and the duration of the decider call
  (request 5 of unit R3): the writer reads the model from the settings or the cost record.
- `runbook parity summarize --run-dir <dir>` as one option (request 6) is nobody's in this wave: the
  two-option form works.

## The decider and the bindings copy (unit R2, `rb3-loom-run-decider`)

Page: `docs/loom-run-decisions.md`; tests: `runbook/src/loom_run/decider/tests.rs` and
`runbook/src/loom_run/bindings_copy/tests.rs`.

- The page holds the decisions table: per driver decision kind the inputs, the allowed answers,
  whether it is a `decisions[]` entry at Loom `2c0a6f48`, and what a run does; kinds Loom cannot ask
  are listed as "not reachable through `tree` today". Check: `grep -c 'Not reachable through'
  docs/loom-run-decisions.md` is 3 or more.
- `decider::from_settings` starts the bindings copy's `decider.sh` with a self-built environment
  (`PATH`, `HOME`, `SDC_*` only), the per-call budget as `SDC_DECIDER_MAX_BUDGET_USD`, a timeout that
  ends the process group, and answers `CannotDecide` for anything missing, unparsable or with an exit
  other than 0. Check: `cargo test --locked --bin runbook loom_run::decider`.
- Autonomy below A5 ends in `CannotDecide {reason: "reserved for a person: ..."}`; at A5 the decider
  answers as `decider:model`. Check: `only_autonomy_a5_lets_the_decider_answer`.
- The call limit (`decider.max_calls`) ends the run's decisions with `CannotDecide`; `escalate: final`
  takes `best`, `stop` waits for a person. Check: the `call_limit` and `escalate` tests.
- `bindings_copy::materialise` rewrites the copy only (principal of the decider's person steps, model,
  turns and budget per role, adapter, `--claude-credentials`), keeps all 66 bindings, never writes
  the committed `bindings/*.yaml`, refuses an unknown settings key by name, and never holds the login
  file's path in a result. Check: `cargo test --locked --bin runbook loom_run::bindings_copy`.
- The copy loads in the pinned `b10x-loom` (`tree open`): slow-lane check by hand, documented in
  `docs/loom-run-decisions.md` under "Checks".

## The three together (integration, unit R1)

Page: `docs/loom-run-settings.md`; tests: `runbook/tests/loom_run/decider.rs` and `settings.rs`.

- One settings format, `runbook.loom-run-settings/1`: `docs/loom-run-settings.md` lists every key the
  loop, the decider and the bindings copy read, its type and default, and what `sdc-experiment run`
  writes today and must change ("R4 must change"). Check: test
  `the_file_r4_writes_today_is_refused_for_the_decider_kind_it_does_not_name`.
- A settings member that repeats an option (`loom.sha256`, `loom.confinement`, `loom.cap_micro_usd`,
  `max_llm_calls`, `max_wall_seconds`, `run`, `format`) must agree with it, or the run is refused
  naming the member. Check: `a_member_that_repeats_an_option_must_agree_with_it_and_is_refused_by_name`.
- A run with two model steps and an approval answered by the model decider (a fake `decider.sh` in the
  bindings copy) ends delivered, with a run directory the writer made (`run-settings.jsonl`,
  `llm-calls.jsonl`, `decisions.jsonl`, `rounds.jsonl`, `status.json`): test
  `two_model_steps_and_an_approval_by_the_model_decider_end_delivered_with_the_run_directory`.
- A run stopped by the spend cap writes `run-stopped.json` with cause `spend_cap` and no `status.json`:
  test `a_run_with_the_model_decider_is_stopped_by_the_spend_cap_and_the_directory_says_so`.
- A decider that cannot decide stops the run as `stopped: decider`, books nothing for the decision and
  leaves no decision line: test
  `a_decider_that_escalates_stops_the_run_for_a_person_and_books_nothing_for_it`.
- A rejection runs the printed command with `--deny`: test
  `a_rejection_by_the_model_decider_runs_the_printed_command_with_deny`.
