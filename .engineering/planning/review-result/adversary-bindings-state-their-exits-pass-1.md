---
format: aep.planning-md/3
id: review-result:adversary-bindings-state-their-exits-pass-1
kind: review-result
status: active
title: 'Adversary pass 1 of the unit bindings-state-their-exits at 208ed8a7: red, 1 blocker, 1 warning, 4 notes'
summary: 'Two committed driver tests read lines of bindings/PENDING.md that the unit removed and fail; the guard of sdc-tool.sh accepts a tree that is a file and the gate then ends with a program error. Notes: a not-called exit leaves a feedback copy, the derived-tree renewal counts as a run, the checker takes the pool list from the argv, a text names 130 entries of 131.'
relations:
- reviews: story:bindings-state-their-exits
revision: 1
---
unit: bindings-state-their-exits, branch impl/bindings-state-their-exits, head 208ed8a7 (over main 566a0c3f)
verdict: red (1 blocker, 1 warning, 4 notes)
cases: executed driver filter 198 passed + 3 failed of 201 (before my case: 200 selected, 2 red); runbook exit_classes 5 of 5, red 3 (the 2 committed tests the unit left red, 1 mine)
origin: introduced 4 / pre-existing 1 / undecided 1
wrote-outside-worktree: <scratch>/exits-unit/adv/ (logs, scripts, mutant copies) and <scratch>/exits-unit/adversary-1.md; build dirs driver/target and runbook/target (cargo clean done)
needs-coordinator: none

# 1. git --no-pager diff --stat (worktree, tests only)

    driver/tests/main.rs | 1 +        (one `mod adv_not_called_writes;` line: driver has one test binary)
    ?? driver/tests/adv_not_called_writes.rs   (untracked, new)

No implementation file touched.

# 2. Cases added (written before anything ran), each run alone

- driver/tests/adv_not_called_writes.rs `a_not_called_exit_with_a_retry_feedback_document_leaves_the_workspace_as_it_was`:
  spec.author, a needs_person finding, LOOM_RETRY_FEEDBACK set. Asserts exit 79 (holds), then that the run made no file.
  RED, run alone: `a not-called exit writes nothing; the run made [".sdc/loom-feedback/e2818af3705df73f/feedback.json"]`
  (tests/adv_not_called_writes.rs:98). Summary: 1 test run: 0 passed, 1 failed.
- <scratch>/exits-unit/adv/gate-tree-is-a-file.sh: `ess-gate` under LOOM_CASE with `spec` a regular file.
  RED: `sdc-reply record: spec: no such tree in the workspace` / `exit 126` / `RED: exit 126 is in exits.error of spec.gate`.
  (First run was a false red, exit 127 because yq was missing from PATH; fixed in the case, then the real red above.)
- <scratch>/exits-unit/adv/pending-lines-the-tests-read.sh: the predicate of two committed tests, by grep on the tree's
  bindings/PENDING.md. On the tree: MISSING for all 5 pool-wait bindings, exit 1. On a copy of PENDING.md at 566a0c3f: found for all 5, exit 0.
- <scratch>/exits-unit/adv/checker-mutants.sh: 16 mutants of bindings/*.yaml (scratch copies) through the exits block of
  scripts/check-bindings.sh, extracted verbatim. Result in part 5.

# 3. Suite runs (after the cases existed)

`cd driver && cargo nextest run --locked --test main --no-fail-fast -E 'test(sdc_node) | test(questions_check) | test(the_pending_list_gives) | test(adv_not_called_writes)'`
Summary [77.568s] 201 tests run: 198 passed, 3 failed, 232 skipped; exit 100. Load average 79 at start. Failed:

    bugfix_languages::pool::the_pending_list_gives_exits_69_and_70_for_the_three_bug_fix_bindings
        panicked at tests/bugfix_languages.rs:510:32: no line of PENDING.md for sdc-bugfix repro.run naming exit 69
    impl_gate::the_pending_list_gives_exit_69_for_both_bindings
        panicked at tests/impl_gate.rs:387:32: no line of PENDING.md for sdc-story.bindings.json tests.run naming exit 69
    adv_not_called_writes::a_not_called_exit_with_a_retry_feedback_document_leaves_the_workspace_as_it_was (mine)

Slowest passing tests of the filter: 13.7 s, 13.3 s, 7.5 s (all `questions_check::loom_mode`, not added by the unit). None over 60 s.
An earlier run of the same filter was cut by my own `timeout 580` while the job waited for the disk gate (partial, 35 of 201 done); the numbers above are the complete rerun.

`cd runbook && cargo test --locked --test cli exit_classes`: test result: ok. 5 passed; 0 failed; finished in 18.30s (load 166 at the end).
`scripts/check-bindings.sh` was not run whole (it builds three more driver binaries and runs the experiment and site checks); its exits block passes on the tree (checker-mutants.sh baseline: `pass`).

# 4. Findings

| # | severity | file:line | verdict | origin | what was measured / what reaches it |
|---|---|---|---|---|---|
| 1 | blocker | driver/tests/impl_gate.rs:387 and driver/tests/bugfix_languages.rs:510 | NEEDS-CHANGE | introduced | Two committed tests read `bindings/PENDING.md` for the pool-wait lines (`- \`sdc-story.bindings.json:\` ... \`tests.run\` ... 69`, same for release and the three bugfix actions). The unit's acceptance bullet 4 removes those lines (PENDING.md diff, 27 lines gone) and does not touch the tests. Measured: both panic as quoted in part 3; both pass at 566a0c3f (grep on the base file: found x5). Reaches: `task check` runs the driver tests; the unit's own filter (`sdc_node`, `questions_check`) does not select them, so the unit's gate run could not see them. Fix: delete or rewrite both tests to read `exits.error` of the five bindings (the exit_classes test already holds it). |
| 2 | warning | bindings/tools/sdc-tool.sh:226,232 | NEEDS-CHANGE | introduced | The new guard is `[[ -e $tree ]]`, `sdc-reply record` needs a directory (`tree_digest`: `is_dir`). A tree that exists as a file passes the guard, the record fails, `rc=126`. Measured: `ess-gate` with `spec` a file prints `spec was refused` (ess exit 1: a result) and the tool exits 126; 126 is in `exits.error` of `spec.gate`, so the run ends ProgramError with nothing recorded instead of a failed gate. Also line 232 overrides any command result with 126 whenever the record fails. Reaches: a spec.author or change.implement output that makes `spec` or `impl` a file; no documented path produces it, so low reach. Fix: `-d`, and keep the command's rc when the record fails on a tree that is not a directory. At the base the same 126 gave `failed`; the unit's list makes it end the run. |
| 3 | note | driver/src/bin/sdc-node/setup.rs:131 (retry_feedback, :276-313) | NEEDS-CHANGE | undecided | A not-called exit 79 after a retry feedback document left `work/.sdc/loom-feedback/<digest>/feedback.json`. Test: adv_not_called_writes (red, part 2). The story says a not-called exit "records nothing"; the docs say "writes nothing" (docs/loom-bindings.md, "The exit lists"). The file is only read by the same call, never by the next step, so the harm is none found; setup.rs is not in the diff, so the write also happened at the base with exit 0 (not run there). Reaches: a retried spec.author while a finding waits for a person (retry_when plus needs_person). |
| 4 | note | driver/src/bin/sdc-node/findings.rs:163 | CONFIRMED | pre-existing | The derived-tree renewal prints "llm-node: spec.author not called: ..." and exits 0, so Loom counts it as a run of `spec.author` (`budget.max_runs: 4`, result `authored`). The story acceptance bullet 2 reads "exits with that code where it prints 'not called' today"; the unit deviates for a stated reason (spec-own-default ends Stuck with 79), in code comment, CHANGELOG and docs/loom-bindings.md, but the story's Acceptance text is unchanged and no doc says the renewal costs one of the four runs. Each default-taken finding after the first author run costs one. Not run through Loom (no run of spec-own-default made); reading of Loom 70b0e6e `runs()` filter (class null = counted). |
| 5 | note | scripts/check-bindings.sh:136 (`$pool` from `--pool-wait`) | CONFIRMED | introduced | The pool list is derived from the argv, so a pool-wait binding that loses `--pool-wait` and takes `[75, 126, 127]` passes the checker (mutant `pool-binding-without-pool-wait-short-list`: pass). An unknown member under `exits` also passes (`exits-extra-member`: pass); Loom's loader owns that refusal. The Rust test `every_program_binding_lists_...` holds the five pool bindings by name, so the gap is covered there. |
| 6 | note | runbook/src/check.rs:19,513 | CONFIRMED | introduced | "the 130 entries of the old suite" is now false: the unit's old-suite-names.txt holds 131 names and docs/cycles-suite.md says so (131 entries, 37+18+39+37). The check's refusal text tells the reader to restore a list of 130. Implementation file, not mine to edit. |

False sentences in the documents: docs/progress.md:427 "Under Loom the exits 69 and 70 of the pool wait are recorded as failed until its loader has exits" (generated record of release 0.3.1, true of the measured runs, stale as a statement of today; note only). Nothing else found false in docs/loom-bindings.md, docs/cycles-suite.md, docs/driver.md, docs/loom-run-directory.md, bindings/PENDING.md or CHANGELOG.md: the counts (50 program bindings, 14 model nodes, result keys 0,1,3,5,8, 131 entries) match the tree.

# Acceptance, bullet by bullet

1. Each program binding has `exits.error` `[75,126,127]`, pool five `[69,70,75,126,127]`: met. runbook `exit_classes::every_program_binding_lists_the_exits_that_are_a_program_error` (exact vectors, 50 programs, 5 pool bindings); checker baseline pass.
2. Each model node has one `exits.not_called`, `sdc-node` exits it where it prints "not called": partly met. 14 nodes with `[79]`: `sdc_node::every_model_node_binding_lists_the_one_exit_the_program_ends_not_called_with`; exit 79 on the findings wait and the questions guard: `sdc_node::a_finding_that_waits_for_a_person_stops_the_node`, `sdc_node::questions_write_is_not_called_while_questions_exist`, `questions_check::loom_mode` (all pass). The derived-tree renewal prints "not called" and exits 0 (finding 4).
3. A test binds a program that exits 75 (ProgramError, no result, `max_runs` not reduced) and one for the not-called code: met. `exit_classes::a_bound_program_that_exits_75_ends_the_run_program_error_and_records_nothing_and_no_run_is_counted` and `exit_classes::a_bound_program_that_exits_the_not_called_code_records_nothing_and_no_run_is_counted` (pass, real Loom 70b0e6e, stand-in sdc-node); the exits are asserted as literals (75 / class `error`, 79 / class `not_called`, zero runs without a class). They would fail with `exits` removed from `bug.dissect` of sdc-bugfix.yaml (reasoned, not mutated: the rig links the tree's bindings).
4. D36, D37, pool-wait entries leave PENDING.md; `scripts/check-bindings.sh` exits 0: not met in the tree's gate. The entries are gone, but two committed tests still read them and fail (finding 1). The exits block of the checker passes; the whole script not run.
5. `runbook test cycles/tree-suite.yaml`: no entry changes from expected to another state: not checked (told not to run it whole). The diff adds one `pending` entry and changes no `expected` one.
6. Added bullet (spec-person-wait stub entry, pending with the Loom story): met as far as the file shows; entry is `pending: true`, docs row names the gap and the Loom code lines. Not run.

# 5. Attacked and not broken

- Exit-class collisions in the tools: `sdc-tool.sh`, the three adapters and `sdc-bugfix` return 69/70 only from the pool paths (`pool_run`, `sdc-bugfix::run`), 126/127 only for a missing program or unusable output, 79 nowhere; `ess` exits 1 on a missing or malformed spec (probed); result tables hold only 0,1,3,5,8, none in an error list. The writer-caused `exit 126` of `impl-gate` (cargo test exit 0 with no `test result:` line) is a result-vs-error call that predates the unit and is stated in the tool; not tested.
- Checker mutants refused: drop `exits`, 79 in error, error code as result key (also in a second evidence entry), node without `not_called`, `not_called` 78, 79 in both classes, 79 as result key, pool binding with the short list, non-pool binding with 69/70, string codes, `exits` on a person action, whole file dropped.
- The `sdc-tool.sh` guard on a missing tree hides nothing: ess-gate, conform-run and impl-gate all fail on a missing tree (probed ess-gate: exit 1; impl-gate `[[ -d $tree ]]`).
- Changed goldens and tests: only `exit 0` to `exit 79` in four goldens, `Some(0)` to `Some(79)` in questions_check, added asserts in sdc_node; none loosened.
- Not-called paths: before the 79 returns, `sdc-node` writes nothing but the retry-feedback copy (finding 3); the questions guard and the findings wait run before any call directory.

# 6. Paths written outside the worktree

- <scratch>/exits-unit/adv/ (checker-mutants.sh, mut/, gate-tree-is-a-file.sh, gate-file/, pending-lines-the-tests-read.sh, basecopy/, g1/, e1/, build-driver.log, run-adv-notcalled.log, run-driver-suite.log, run-driver-suite2.log, run-runbook-exit-classes.log)
- <scratch>/exits-unit/adversary-1.md
- inside the tree: driver/target and runbook/target, removed with `cargo clean`.

# 7. Findings block

```findings
- file: driver/tests/impl_gate.rs
  line: 387
  category: contract-drift
  severity: blocker
  verdict: NEEDS-CHANGE
  origin: introduced
  message: the_pending_list_gives_exit_69_for_both_bindings and bugfix_languages.rs:510 read the PENDING.md lines the unit removed and now panic; the unit's test filter does not select them
- file: bindings/tools/sdc-tool.sh
  line: 226
  category: boundary
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: the guard is -e but sdc-reply record needs a directory, so a tree that is a file ends the gate with 126, which exits.error turns into a ProgramError instead of a failed gate
- file: driver/src/bin/sdc-node/setup.rs
  line: 131
  category: acceptance
  severity: note
  verdict: NEEDS-CHANGE
  origin: undecided
  message: a not-called exit 79 after a retry feedback document leaves .sdc/loom-feedback/<digest>/feedback.json in the workspace although the docs say a not-called exit writes nothing
- file: driver/src/bin/sdc-node/findings.rs
  line: 163
  category: acceptance
  severity: note
  verdict: CONFIRMED
  origin: pre-existing
  message: the derived-tree renewal prints "not called" and exits 0, so it counts against spec.author max_runs 4; the story acceptance and the docs do not say so
- file: scripts/check-bindings.sh
  line: 136
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the pool list is derived from --pool-wait in the argv, so a pool binding that drops the flag and the 69/70 codes passes the checker; only the Rust test holds the five by name
- file: runbook/src/check.rs
  line: 513
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the refusal text and the doc comment at line 19 still say 130 entries of the old suite, the list now holds 131
```
