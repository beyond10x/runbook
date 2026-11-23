---
format: aep.planning-md/3
id: review-result:adversary-bindings-state-their-exits-pass-2
kind: review-result
status: active
title: 'Adversary pass 2 of the unit bindings-state-their-exits at a4cfcd6e: red, 1 note'
summary: 'The six findings of pass 1 are fixed (finding 4 with the story text). One new note: the cleanup of a not-called exit removes a dangling link that was there before the run. 21 of 24 checker mutants refused; the three that pass are outside the story.'
relations:
- reviews: story:bindings-state-their-exits
revision: 1
---
unit: bindings-state-their-exits, pass 2 on fix round 1 (208ed8a7..a4cfcd6e: bb8abada, 115de4c8, a4cfcd6e), branch impl/bindings-state-their-exits, head a4cfcd6e
verdict: red (1 note open: a pre-existing link removed by the 79 cleanup; nothing above note level)
cases: executed driver filter 245 passed + 1 failed of 246 (the one red is my link case); my 5 new cases ran alone first: 4 passed, 1 failed
origin: introduced 1 / pre-existing 0 / undecided 0
wrote-outside-worktree: <scratch>/exits-unit/adv/ (scripts, logs; the scratch copy of the tree is deleted) and <scratch>/exits-unit/adversary-2.md; driver/target removed with `cargo clean` (runbook/ was not built)
needs-coordinator: none (the story's Acceptance text under .engineering/ is the coordinator's; see finding 4 of pass 1)

# 1. git --no-pager diff --stat (worktree, tests only)

    driver/tests/main.rs | 1 +        (one `mod adv2_not_called_removes;` line, on top of the committed `mod not_called_writes;`)
    ?? driver/tests/adv2_not_called_removes.rs   (untracked, new)
    ?? .engineering/planning/review-result/adversary-bindings-state-their-exits-pass-1.md   (not mine; the coordinator's record of pass 1)

No implementation file touched.

# 2. The six findings of pass 1

| # | pass 1 | now | shown by |
|---|---|---|---|
| 1 | blocker: two tests read the removed PENDING.md lines | fixed | driver/tests/impl_gate.rs `the_story_and_release_yaml_list_the_pool_wait_exits_69_and_70_as_a_program_error` and bugfix_languages.rs `the_bug_fix_yaml_lists_the_pool_wait_exits_69_and_70_as_a_program_error_for_its_three_bindings` pass (suite below); no driver test reads PENDING.md any more (grep). See 5(d) for the mutants. |
| 2 | warning: `-e` guard, a file tree ended 126 | fixed | `-d` at sdc-tool.sh:227. My pass-1 case gate-tree-is-a-file.sh was red (exit 126), now `exit 1` / GREEN. The committed `impl_gate::a_gate_on_a_tree_that_is_a_file_ends_with_its_own_result_and_not_126` passes. |
| 3 | note: 79 with a retry feedback left `.sdc/loom-feedback/...` | fixed | committed `not_called_writes::a_not_called_exit_with_a_retry_feedback_document_leaves_the_workspace_as_it_was` passes (my pass-1 case, renamed). |
| 4 | note: the renewal counts against max_runs, unsaid | fixed in part | docs/loom-bindings.md now says "That renewal of `spec.author` exits 0 and so counts as one of its `budget.max_runs`". The story file's Acceptance bullet 2 ("exits with that code where it prints 'not called'") is unchanged (coordinator's). |
| 5 | note: checker derived the pool list from the argv | fixed | pass-1 mutant `pool-binding-without-pool-wait-short-list` is now refused: `a binding that waits for the build pool has no --pool-wait in its argv`. |
| 6 | note: "130 entries" in runbook/src/check.rs | fixed | lines 19 and 513 no longer carry a count. |

# 3. Cases added (written before anything ran), each run alone

driver/tests/adv2_not_called_removes.rs, run alone (`cargo nextest run --locked --test main -E 'test(adv2_not_called_removes)'`): `Summary 5 tests run: 4 passed, 1 failed`.
- `a_not_called_exit_with_no_feedback_document_writes_no_file_and_no_directory`: exit 79, entries (files, directories, links) before and after equal. PASS.
- `a_feedback_directory_that_existed_before_keeps_its_other_content`: `.sdc/loom-feedback/keep/marker` survives, only `keep` is left. PASS.
- `an_empty_sdc_directory_that_existed_before_is_still_there`: PASS.
- `a_link_at_sdc_to_a_directory_outside_keeps_what_was_in_it`: `.sdc` a link to a directory outside the work directory holding `precious`; `precious` and the link survive. PASS.
- `a_link_at_loom_feedback_that_existed_before_is_not_removed`: `.sdc/loom-feedback` a dangling link made before the run. RED:
  `panicked at tests/adv2_not_called_removes.rs:128:5: the link was there before the run and is gone after it`.

Scratch cases (POSIX sh): checker-mutants2.sh (22 mutants of bindings/*.yaml in scratch copies through the exits block of scripts/check-bindings.sh, extracted verbatim), gate-record-fails-on-dir.sh, pool-exit-mutants.sh (run in a scratch copy of the tree with its own build; copy deleted).

# 4. Suite run (after the cases existed)

`cd driver && cargo nextest run --locked --test main --no-fail-fast -E 'test(adv2_not_called_removes) | test(not_called_writes) | test(impl_gate) | test(bugfix_languages) | test(sdc_node) | test(questions_check)'`
Summary [35.406s] 246 tests run: 245 passed, 1 failed, 193 skipped; exit 100. Load average about 80 earlier in the session. The failure is my dangling-link case. Slowest passing: 8.3 s, 7.7 s, 6.1 s (bugfix_languages::pool, committed before this round). None over 60 s. The conformance tests that need `--examples` are not in this filter.
runbook exit_classes was not rerun (the round touched no runbook source but a comment and a message in check.rs).

# 5. The four attacks on the round

(a) The 79 cleanup (setup.rs `made`, sdc-node.rs:260). It picks the outermost of `.sdc`, `.sdc/loom-feedback`, `.sdc/loom-feedback/<digest>` that does not exist (`!exists()`, which follows links) and removes it on exit 79.
- Directory that existed before: not removed (marker case, empty `.sdc` case: PASS).
- Link to outside: a link at `.sdc` that resolves is not "made"; what the run made inside the target is removed, what was there stays (`precious` case: PASS). The path is always `work/.sdc/...` plus a hex digest, so nothing outside the work directory is named.
- 79 with no feedback document: writes nothing and removes nothing (PASS).
- Hole: a link that exists but dangles passes `!exists()`, so it counts as "made by the run" and is removed. Red case above. Reaches: nothing found (a dangling `.sdc/loom-feedback` link is not produced by any step); note.
- Not checked: a second process creating `.sdc` between the existence test and the removal (Loom runs the steps of one case one after another).

(b) The `-d` guard (sdc-tool.sh:227-233) on a real directory. A record that fails on a real directory is not hidden: with `.sdc/records` a regular file and a failing gate, the tool prints the gate's log (`stub ess refuses`), then `sdc-reply record: ./.sdc/records/ess_gate: Not a directory`, and exits 126 (`|| rc=126` is unchanged at line 233), so the run ends ProgramError loudly. Nothing passes silently: a tree that is a file makes ess-gate, conform-run and impl-gate fail on their own checks. Note for the commit text: the command's own exit is kept only when the tree is not a directory; on a directory with a failed record the 126 replaces it, as before.

(c) The checker with five pool bindings by name. Rerun of my 16 pass-1 mutants: 13 refused as before, `pool-binding-without-pool-wait-short-list` now refused; 3 still pass, all outside the story (`exits-extra-member`, `node-by-path-without-not_called`, `kind-spelled-differently`; Loom's loader refuses the first, the other two name programs and kinds nothing binds). Reverse case and removals, all refused: a sixth binding with `--pool-wait` and the short list; the same with the full list (`--pool-wait in the argv of a binding the checker does not list`); a model node with `--pool-wait`; a pool binding deleted (`bindings/sdc-story.yaml:tests.run: a binding that waits for the build pool is not in bindings/`); `gate.run` losing 70; `acceptance.run` losing the flag and keeping the full list.

(d) The two rewritten tests, run in a scratch copy of the tree (test binary built there; the copy's bindings mutated, the tree's not). Baseline both exit 0. Mutants, each caught (exit 101): story `tests.run` loses 69; release `acceptance.run` loses 70; bugfix `repro.run` loses 69; bugfix `tests.run` loses 70; bugfix `gate.run` loses 69 and 70. Both tests assert the exact list `[69, 70, 75, 126, 127]` and panic when `exits.error` or the action is absent.

# 6. Findings

| # | severity | file:line | verdict | origin | what was measured / what reaches it |
|---|---|---|---|---|---|
| 1 | note | driver/src/bin/sdc-node/setup.rs:289 (`made`, rfind on `!d.exists()`) | NEEDS-CHANGE | introduced | A dangling link at `.sdc/loom-feedback` that existed before the run is removed by the 79 cleanup; test `adv2_not_called_removes::a_link_at_loom_feedback_that_existed_before_is_not_removed` red as quoted. Reaches: nothing found (no step makes such a link). Fix: test `symlink_metadata` instead of `exists()` for the three directories. |

Nothing else above note level is open from the round.

# 7. Attacked and not broken

- The cleanup on a directory, an empty directory and a link to outside (3 green cases); 79 without feedback.
- Failed record on a real directory: loud (126), not hidden.
- Checker: 21 of 24 mutants refused including the reverse case; the 3 passes are not reachable bindings.
- Rewritten pool-exit tests: 5 of 5 mutants caught.
- The fix round's docs: the three changed sentences match the code (renewal counts as a run; cleanup; no count in check.rs).

# 8. Paths written outside the worktree

<scratch>/exits-unit/adv/ (checker-mutants2.sh, mutants2-tail.txt, gate-record-fails-on-dir.sh, gate-rec/, pool-exit-mutants.sh, run2-adv.log, run2-copy-base.log, run2-suite.log, and the pass-1 files); <scratch>/exits-unit/adversary-2.md. In the tree: driver/target, removed with `cargo clean`.

```findings
- file: driver/src/bin/sdc-node/setup.rs
  line: 289
  category: boundary
  severity: note
  verdict: NEEDS-CHANGE
  origin: introduced
  message: the 79 cleanup tests existence with exists(), so a dangling link at .sdc/loom-feedback that existed before the run counts as made by it and is removed
```
