---
format: aep.planning-md/3
id: review-result:adversary-loom-pin-db2726ff-pass-1
kind: review-result
status: active
title: 'Adversary pass 1 of the unit loom-pin-db2726ff at c477a7ca: red, 2 warnings, 7 notes'
summary: 'With --loom a refused tree approve ends exit 1 without a cause while the page says exit 12; the run-directory page and the member list of outcome.json lack the new causes. Notes: a false sentence on the refusal before a run, an inspector case that cannot fail, program errors that end stuck unnamed, a cycle at the 60 s bound under load, the reach of exit 12, a word of the story, two causes no test drives through the runner.'
relations:
- reviews: story:loom-pin-db2726ff
revision: 1
---
verdict: red

unit: runbook, `a5033277` `c70efbdd` `c477a7ca` over `15e58ec` (Loom pin db2726ff), tree `<trees>/b10x/runbook/pin-adv` detached at `c477a7ca`
verdict: NEEDS-CHANGE (2 warnings, 7 notes; no blocker)
cases: executed 124 -> 133 in the filtered run (the unit's tests by name, plus 9 of mine and 4 `loom_run::core::` tests that my `adv_child` binary pulls in by `mod`), red 3 (mine)
origin: introduced 7 / pre-existing 0 / undecided 2
wrote-outside-worktree: logs and scratch only, listed in part 6
needs-coordinator: none

## 1. `git --no-pager diff --stat`

Empty: no tracked file changed. `git status --short`:

```
?? runbook/tests/adv_child.rs
?? runbook/tests/adv_pin.rs
```

Both are test files. No non-test path touched.

## 2. Cases added (written before anything ran), red output

`runbook/tests/adv_pin.rs` (binary `adv_pin`, `CARGO_BIN_EXE_runbook`, sh fixtures of the unit):

| case | asserts | now |
|---|---|---|
| `adv_workers_above_one_under_substrate_writes_nothing_in_the_default_places` | `--workers 2 --confinement substrate`, default scratch: exit 2, `SubstrateWorkersUnserved`, nothing under runtime, state or home dir | green |
| `adv_an_unknown_only_name_beside_a_known_one_refuses_before_any_cycle_and_writes_nothing` | `--only a,no-such --workers 2`: exit 2, `CycleUnknown`, no stdout, nothing written | green |
| `adv_only_a_pending_cycle_runs_it_and_reports_it_pending` | `--only later` (pending): exit 0, `later: pending, still failing`, `selected 1 of 4 cycles`, `0 of 0 as expected, 1 pending` | green |
| `adv_only_names_given_twice_select_a_cycle_once` | duplicate and repeated `--only`: one line, `selected 1 of 4` | green |
| `adv_the_summary_of_two_workers_has_the_lines_and_counts_of_one_worker` | 1 worker and 2 workers: same exit, same line order, same closing line | green (18 s at load 70 to 120) |
| `adv_the_run_directory_page_lists_the_causes_a_refused_run_writes` | row `run-stopped.json` of `docs/loom-run-directory.md` names `program_signalled` or `refused` | RED |
| `adv_the_loom_run_page_lists_cause_among_the_members_of_outcome_json` | member list of `outcome.json` in `docs/loom-run.md` holds `cause` | RED |

`runbook/tests/adv_child.rs` (binary `adv_child`, the unit's fake `b10x-loom`, `--loom` path):

| case | asserts | now |
|---|---|---|
| `adv_child_a_refused_tree_run_ends_the_run_refused_with_its_reason_as_the_cause` | child `tree run` prints `b10x-loom: tree refused: TreeStopped: ...`: exit 12, outcome `refused`, cause `tree_stopped` in `outcome.json` and `run-stopped.json` | green |
| `adv_child_a_refused_tree_approve_ends_the_run_refused_with_its_reason_as_the_cause` | child `tree approve` prints `b10x-loom: tree refused: PersonProgramFailed: ...`: exit 12, cause `person_program_failed` | RED |

Red output, each case run alone first (`cargo nextest run --locked -E 'binary(adv_pin)'`, then `binary(adv_child)`), verbatim:

```
FAIL adv_the_run_directory_page_lists_the_causes_a_refused_run_writes
  tests/adv_pin.rs:151:5: the cause column does not name the refusals: | `run-stopped.json` | `at`, `by`, `cause`, `case`, `reason` | `CapReached` or an `Ended` that is not `ended`; `by` is `rule:loom-run`; `cause` is `spend_cap`, `llm_calls_spent`, `wall_time_spent`, `signal`, `decider_cannot_decide`, `stuck`, `ended_without_outcome` or `error`; ...
FAIL adv_the_loom_run_page_lists_cause_among_the_members_of_outcome_json
  tests/adv_pin.rs:160:5: runbook.loom-run-outcome/1: outcome, reason, loom_exit_code,
                                       stderr_tail, exit_code, steps, spent_micro_usd, llm_calls
FAIL adv_child_a_refused_tree_approve_ends_the_run_refused_with_its_reason_as_the_cause
  tests/adv_child.rs:42:5: assertion `left == right` failed: a person's program that fails is exit 12 in the library; with --loom: runbook run: error: b10x-loom the printed command for a decision of kind ApproveAction exited Some(2)
    left: 1
   right: 12
```

(Line numbers of the first two moved by 2 after I shortened the slow case; the final run prints 149 and 158.)

## 3. Suite run, after the cases existed

Unit's own tests plus mine, by name (`cd runbook && SLOT_TIMEOUT=30m cargo nextest run --locked --no-fail-fast -E 'test(test_suite::) | test(held) | test(refus) | test(a_bound_program_ended_by_a_signal) | test(a_person_program_that_fails) | test(an_answer_under_a_model_principal) | test(a_hold_and_a_reopen) | test(outcome_tests) | test(tail::tests) | test(cycles_run_one_after) | test(a_worker_count) | test(the_selection_line) | test(the_exit_codes_are_all_different) | test(adv_)'`):

```
Summary [53.373s] 133 tests run: 128 passed (1 slow), 5 failed, 621 skipped
FAIL adv_child ... adv_child_a_refused_tree_approve_ends_the_run_refused_...      (mine)
FAIL adv_pin ... adv_the_loom_run_page_lists_cause_among_the_members_of_outcome_json   (mine)
FAIL adv_pin ... adv_the_run_directory_page_lists_the_causes_a_refused_run_writes      (mine)
FAIL cli loom_run::replay_boundaries::a_settings_file_with_a_replay_key_and_the_recorded_option_is_refused
FAIL cli loom_run::replay_boundaries::an_empty_log_starts_one_call_at_cap_1_and_refuses_the_next
exit 100
```

The two `replay_boundaries` reds are not the unit's: `runbook run: refused: sdc-replay is not found: build the helper binaries` and an `Os NotFound` for the same binary; the helper binaries were not built in my tree at that time. Not investigated further. My slow case took 51.6 s in this run (3 runs of a 4-cycle suite at load about 120); I cut it to 2 runs of 2 cycles and the last run of the `adv_` cases printed it at 18.1 s (`9 tests run: 6 passed, 3 failed`, exit 100). Driver: `cd driver && cargo nextest run --locked -E 'test(the_lines_of_a_hold_and_its_reopen)'`: `1 test run: 1 passed`.

Other times over 5 s from the unit's tests in the run above (load average 70 to 120, shared host): `test_suite::a_suite_prints_a_line_per_cycle...` 19.1 s, `inproc::a_person_program_that_fails...` 14.4 s, `inproc::an_answer_under_a_model_principal...` 14.4 s, `a_wrong_cycle_exits_1...` 10.3 s, `only_runs_the_named_cycles...` 10.3 s, `inproc::a_bound_program_ended_by_a_signal...` 9.0 s, `amend::a_check_that_fails_after_a_later_write...` 8.6 s, `held::a_held_tree_is_refused_tree_stopped...` 8.2 s, `engine::tail::tests::the_lines_of_a_hold...` 5.9 s, `workers::two_workers_run_two_cycles...` 5.4 s. None over 60 s.

Old-pin check of the new state test (item 4): `git archive 15e58ec` of `runbook driver ui ess protocol bindings cycles views .cargo` into `<scratch>/adv/base` (deleted afterwards), plus the unit's `amend.rs` and the `test-suite` fixtures and one `mod amend;` line, Loom `70b0e6e0` from that tree's own lock, target dir in scratch:

```
FAIL [5.810s] runbook::cli test_suite::amend::a_check_that_fails_after_a_later_write_takes_the_amend_step_again_and_the_run_does_not_end_stuck
  amend-again-after-a-later-write: FAILED: steps[1].exit (exit, ExitDiffers): expected 0, found 4 (5.4 s)
  Summary: 1 test run: 0 passed, 1 failed
```

Exit 4 is `Stuck`: the test fails on the old pin for the stated reason only. On the new pin it passes in 8.6 s and asserts the order amend, normalise, check, amend in `order.log` plus the count 2 of `amend.upstream`, so a check that never ran would fail it.

## 4. Findings

| # | severity | verdict | origin | file:line | what |
|---|---|---|---|---|---|
| 1 | warning | NEEDS-CHANGE | introduced | `runbook/src/loom_run/child_host.rs:155` with `runbook/src/loom_run/run_loop.rs:722` | With `--loom` only `tree run` reads `b10x-loom: tree refused: <Reason>`. A refused `tree approve` goes through `expect_exit0` and ends `LoomFailed`, outcome `error`, exit 1, no `cause`: `PersonProgramFailed`, `ConfinementRefused` and `NotAPersonAction` (the three the unit says moved to exit 12) arise only at `tree approve`. `docs/loom-run.md:281` says "the same cause and exit 12 arise with `--loom`". Measured: `adv_child_a_refused_tree_approve_...` red, `left: 1 right: 12`. What reaches it: `runbook run --loom` with the child binary (the path `sdc-experiment` takes for a variant without `loom.in_process`; slow lane `task test:loom-in-process`). Fix: read `refusal::in_stderr` in `answer`/`handoff`/`open` of `ChildHost` as `parse_run` does, or say in the doc that only `tree run` is read. |
| 2 | warning | CONFIRMED | introduced | `docs/loom-run-directory.md:76`, `docs/loom-run.md:495` | The run-directory page lists the causes of `run-stopped.json` as 8 names and none of the 29 reason causes or `program_signalled`; the `outcome.json` member list in `docs/loom-run.md` lacks the new `cause`. Both are the files readers (`run_metrics`, `sdc-run-compare`, the inspector) are written against. Two red doc-lint cases above. What reaches it: any reader of those pages. Fix: add the rows. |
| 3 | note | CONFIRMED (by reading, not run) | introduced | `docs/loom-run.md:281`, `runbook/src/loom_run/library.rs:42` | "A refusal before the run starts (the confinement level cannot be given) is exit 2 and its text leads with `ConfinementRefused:`": in a library run `check_level` refuses first with `--confinement substrate is not a level the library can give here ...` (no prefix); the prefix exists only on `calls::runner` and `runbook test`. The existing test only asserts the word `substrate`. |
| 4 | note | CONFIRMED | introduced | `ui/test/event-log-lines.spec.mjs:41` | The inspector never reads `runlog-case-*.jsonl` (`grep runlog ui/`: only a comment at `server.mjs:684`), and the runner's projection never writes a `stop_recorded` event (`run_dir.rs:read_runlog` skips those kinds); the new case feeds `events.jsonl` an invented event name and writes a log file nobody reads, so it stays green with the file deleted. Not run (playwright). What reaches it: nothing found; the claim "every reader reads the kinds" is true for the UI because it reads none. |
| 5 | note | CONFIRMED (by reading) | undecided | `runbook/src/engine/refusal.rs:157` | Only `ProgramSignalled` is lifted out of `ActionFinished.refusal`. Other refusals that end a run in the same line (`ProgramError`, when a program exits a code of `exits.error`, Loom `program.rs:1285`; a program its runner could not start, seen in my entry run: `sdc-node was refused by its runner: CapabilityUnserved ... exec.spawn-failed`) still end `Stuck`, exit 3, cause `stuck`, with the generic text. Both exist at `70b0e6e0` too, so not new in the pin; the acceptance says "never as a catch-all text" for the new ones only. |
| 6 | note | CONFIRMED | introduced | `docs/cycles-suite.md:109`, `cycles/tree-suite.yaml` | Three entries lost `pending` on one quiet-host run each; here (load 72 to 87) `run-stop` ended as expected in 15.8 s, `decider-rework` as expected in 53.6 s, `replay-sibling` failed `steps[1].exit expected 3, found 5` (Loom's 60 s wall bound, 61.1 s). The doc names the load sensitivity ("11 entries ended on the bound at 100 to 177"); the entries are slow lane only. |
| 7 | note | CONFIRMED (by reading) | introduced | `runbook/src/loom_run/library.rs:123`, `runbook/src/loom_run/run_loop.rs:371` | Exit 12 reaches further than the list "three moved": every `tree open` refusal that leads with a reason name (`BindingsRefused`, `ProtocolRefused`, `StoreNotEmpty`, `UnknownProtocol`, ...) in a library run now ends `refused`, exit 12, where it was exit 1; in the other direction `tree status` and `tree verify` refusals after the run (`StoreUnreadable`, ...) stay `error`, exit 1, unnamed. `CHANGELOG.md` and the story name three moved. Exit 2 and 12 are both called "refused" in the docs. Not run. |
| 8 | note | CONFIRMED | introduced | `.engineering/planning/story/loom-pin-db2726ff.md` (acceptance 3) with `runbook/tests/fixtures/test-suite/suites/bindings/tools/amend-wait.sh` | The acceptance says "the first action's handoff check passes"; the fixture's handoff check exits 1. In Loom (`loom-bindings/src/handoff.rs`) exit 0 offers the handoff and any other exit skips it, so exit 1 is what lets the run take the action again: the fixture is consistent with the outcome, the story's word is not. Wording only. |
| 9 | note | INFEASIBLE | introduced | `runbook/src/loom_run/mod.rs` (`outcome_tests`), `runbook/tests/loom_run/run_dir.rs:677` | `tree_stopped` and `confinement_refused` as a run's end are held only by a hand-built `Event::Ended` (run_dir test) and by `calls::run` returning the reason (`held.rs`); no test drives a held tree or a held cgroup root through the runner to `outcome.json`. No CLI path holds a tree or two runs in one process, so nothing reaches it today (`TreeStopped` through the daemon engine ends as a plain failure text, `engine/mod.rs:200`). |

Exit codes and causes (item 1): the match in `refusal.rs` equals Loom's `TreeRefusalReason` at `db2726ff` variant for variant (29, read from `generated/rust/loom/src/tree.rs:1119`); no catch-all arm; causes unique and snake-case of the name (unit test, passes); `named` reads the head before the first `:`, and Loom's `Refused` display starts with the debug name (`loom-tree/src/lib.rs:150`). `outcome.json`, `ended` and `run-stopped.json` take `cause` from one `Outcome::cause()`. No script under `scripts/`, `bindings/tools/`, `driver/src/bin/sdc-experiment*`, `ui/` branches on the exit code of `runbook run` beyond success; no reader branches on the outcome label `refused`.

## 5. Attacked and held

- `runbook test --workers 0`, `65`, `-1`, `many`: exit 64, no output (unit's test, green). `--workers 2` under substrate with default scratch: exit 2, nothing under the runtime, state or home directory (mine). `--only` with an unknown name, with a pending entry, with duplicates: as stated (mine). Summary order with 2 workers equals 1 worker (mine). Per-cycle directories are `<scratch>/<n>/`; I found no shared scratch path, port or runtime-dir file beyond the cycle-to-cycle readability the doc states under `local`.
- Readers: runner projection (`run_dir.rs:read_runlog`, unknown kinds skipped, test `a_hold_and_a_reopen_...` green), `engine/tail.rs` (green), `sdc-replay` (`events_of_log`, green); `run_metrics`, `run_timeline`, `sdc-run-compare`, `sdc-finding-kpis` read `run-stopped.json` and projected `events.jsonl`, never the run log (grep), so they cannot meet the kinds; the daemon's `record_run_event` takes any kind string. A run that ends on a hold ends `refused` with `run-stopped.json`, so no reader shows it as running.
- Suite file: the 14 `pending: true` entries equal the 14 `pending` rows of `docs/cycles-suite.md`; row counts 40 converted, 14 pending, 39 held, 37 dropped equal the headline. The five moved entries differ from the base only by the removed `pending` lines and the `run-stop` check (now `StopRecorded` in the log plus `refusal: TreeStopped` on the resume step: stronger, not weaker).
- Security: the diff touches no environment, login file, argv, spend cap or confinement default; `--confinement` stays `required = true`; `the_process_tree_holds_runbook_and_the_bound_programs_only_and_they_get_path_and_loom_alone` and `the_confinement_level_is_required_and_substrate_is_refused` pass.
- Lock: only the 14 Loom packages move (14 source lines, 11 versions 0.58.0 to 0.59.0); no `[patch]`, no `path`, no `file://`.

## 6. Paths written outside the worktree

- `<scratch>/adv/` logs: `adv-run.log`, `child-run.log`, `suite-run.log`, `final-run.log`, `driver-run.log`, `driver-build.log`, `base-run.log`, `entry-run-stop.log`, `entry-decider-rework.log`, `entry-replay-sibling.log`, `doc-pending.txt`, `yaml-pending.txt`
- `<scratch>/pin-db2726ff/adversary-1.md` (this report)
- Removed again: `<scratch>/adv/base` (base copy), `<scratch>/adv/base-target`, `<scratch>/adv/xdg` (kept scratch dirs of the entry runs)
- Inside the worktree: `runbook/target` and `driver/target` removed with `cargo clean`; the two untracked test files stay.

## Acceptance

1. Cargo.toml rev, lock, no patch/path/file: met (diff; no test).
2. Builds, clippy, new kinds and refusals handled by name: build met (tests compiled); clippy not run; library path met (`outcome_tests`, `refusal::tests`, `inproc` signal/person tests, `held.rs`); `--loom` path not met for `tree approve` (finding 1); StopRecorded/StopLifted readers met (`tail::tests`, `run_dir`, `sdc-replay`), UI case vacuous (finding 4).
3. State of run 2 with no model: met, `test_suite::amend::a_check_that_fails_after_a_later_write_...` passes on the new pin and fails with exit 4 `Stuck` on `70b0e6e0`.
4. `--workers` default 1, refused under substrate, `--only`, `--help`: met (`workers::*`, `test_suite::only_runs_...`, `adv_` cases).
5. `cycles/tree-suite.yaml`: met as to the diff and the doc; reproduced in part (finding 6); `decider-rework-spent` and `rework-fresh-allowance` not run by me.
6. Docs and CHANGELOG: partly met; pin text and suite page yes, run-directory page and `outcome.json` members no (finding 2).
7. Tests of the touched code pass, none over 60 s: met for the unit's tests (the only reds in that run are mine and the two helper-binary ones above).

## Not checked

`cargo clippy -- -D warnings`; the playwright case in `ui/test/event-log-lines.spec.mjs`; the whole test step and the whole `runbook test cycles/tree-suite.yaml`; the other 11 entries, `decider-rework-spent`, `rework-fresh-allowance`; the slow lane against a real child `b10x-loom`; the daemon's engine path with a held tree; a library run on a bad bindings file (finding 7 is by reading); two `replay_boundaries` tests (helper binary missing in my tree); Stuck re-attribution to `program_signalled` for a tree with several cases; a real Substrate level (cgroup not delegated on this host).

```findings
- file: runbook/src/loom_run/child_host.rs
  line: 155
  category: acceptance
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: with --loom a refused tree approve (PersonProgramFailed, ConfinementRefused, NotAPersonAction) ends exit 1 error with no cause, while docs/loom-run.md:281 says the same cause and exit 12 arise with --loom; only tree run is read (run_loop.rs:722); adv_child_a_refused_tree_approve_... is red
- file: docs/loom-run-directory.md
  line: 76
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: the causes listed for run-stopped.json and the members of outcome.json (docs/loom-run.md:495) omit the 29 reason causes, program_signalled and the new cause member; two doc-lint cases red
- file: docs/loom-run.md
  line: 281
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: says a refusal before the run starts leads with ConfinementRefused:, but library.rs:42 check_level refuses first with a text without that prefix
- file: ui/test/event-log-lines.spec.mjs
  line: 41
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the case writes a runlog-case file the inspector never reads and an event name the runner never writes, so it cannot fail for the claim about StopRecorded and StopLifted
- file: runbook/src/engine/refusal.rs
  line: 157
  category: acceptance
  severity: note
  verdict: CONFIRMED
  origin: undecided
  message: only ProgramSignalled is lifted from ActionFinished.refusal; ProgramError and a program its runner could not start still end Stuck with cause stuck and the generic text
- file: cycles/tree-suite.yaml
  line: 1
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: replay-sibling failed at Loom's 60 s bound (61.1 s, exit 5) and decider-rework took 53.6 s at load 72 to 87 after both lost pending; the doc names the load sensitivity
- file: runbook/src/loom_run/library.rs
  line: 123
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: every tree open refusal that leads with a reason name now ends exit 12 (was 1) and post-run status/verify refusals stay exit 1 unnamed, while CHANGELOG and story name three moved refusals; read, not run
- file: .engineering/planning/story/loom-pin-db2726ff.md
  line: 59
  category: acceptance
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: acceptance says the handoff check passes; the fixture's handoff_when exits 1 (skip), which is what Loom needs for the action to be taken again, so the story wording and the fixture differ
- file: runbook/tests/loom_run/run_dir.rs
  line: 677
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: tree_stopped and confinement_refused as a run end are held only by a hand-built Ended event and by calls::run; no test drives them through the runner and no CLI path reaches them today
```
