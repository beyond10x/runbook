---
format: aep.planning-md/3
id: review-result:adversary-line2-fix-rounds-1-2-pass-3
kind: review-result
status: active
title: 'Narrow check of line 2 fix rounds 1 and 2 (units 3, 4, 6, 7) at 16cbc1d: red, 1 note'
summary: 'Notes 1, 3 and 4 of pass 2 fixed; note 2 in part: the divergence message advises to pack the recording again at every change of in/tests.txt, and the doc and the change log say it names the label as the cause.'
tags:
- wave-2026-11-22-w1
relations:
- reviews: story:driver-runtime-removed
revision: 1
---
```
unit: line 2 fix rounds 1 and 2 (units 3, 4, 6, 7) at 16cbc1d, narrow check
verdict: red
cases: executed 29 in the named modules (6 added in adversary_pass3), red 1
origin: introduced 1 / pre-existing 0 / undecided 0
wrote-outside-worktree: <scratch>/removal/line2/adversary-2/pass-3/ (cases.patch, run1.log); nextest tmp dirs under runbook/target/tmp (ignored by git)
needs-coordinator: no
```

1. **Note 1 holds.**
   - `mask_ess_label` is called only for `in/tests.txt` (`hash_file(.., rel == "tests.txt")`) and replaces only the first match.
   - `replay_boundaries::a_version_in_another_input_file_that_looks_like_the_label_is_still_an_input` is green.
   - A second label-shaped line below the header and two preamble lines, changed from `1.0.0` to `9.9.9`, diverges.
   - A `sub/tests.txt` is not masked and diverges.
   - A recording at `0.97.3` replays with the header printing `0.98.0` (mode `recorded`).
   - The three unit tests in `sdc-replay` pass.
2. **Note 2 is half true.**
   - Form 1 is true. A recorded digest of the raw text at `0.97.4` against a current `0.97.4` gives "only the ESS version label … pack the recording again".
   - Form 2 is true when a stale label is the cause. A recording at `0.97.1` against `0.97.4` gives "if that is the only difference, pack the recording again".
   - Form 2 is also given for a changed test name at the committed label `0.97.3`, where the label is not a cause. That case, `adversary_pass3::a_changed_test_at_the_committed_label_is_not_told_to_be_packed_again`, is RED (0.05 s): the message is "in/tests.txt changed: its header line carries an ESS version label, and a recording packed before the label was masked holds the digest of its own label: if that is the only difference, pack the recording again". Every real log has a label, so every `tests.txt` change gets this advice.
   - `docs/loom-run.md:386` and `CHANGELOG.md:32` say the message "names the label as the cause". Form 2 names it only as a possible cause, so the doc and CHANGELOG do not say the same.
3. **Note 3 holds.** `docs/cycles-suite.md` and the `Taskfile.yml` description give 17.1 s at load 44, 26.1 s and 27.8 s, 30.4 s at load 45, and 49 s and 61.5 s at load 130. Both say the runner ends a test at 60 s and that the timeout is not raised. `git diff c413209 HEAD -- runbook/.config/nextest.toml` is empty.
4. **Note 4 holds.** `git diff c413209 HEAD -- ess/domains/decision.yaml` is empty. The CHANGELOG states the order of `empty-record-id`, `already-recorded` and `no-request`, says no input can reach the `already-recorded` predicate after that guard, and no longer calls it a fix.
5. **Diff scope holds.**
   - The diff `c413209..16cbc1d` touches only `CHANGELOG.md`, `Taskfile.yml`, `docs/cycles-suite.md`, `docs/loom-run.md`, `driver/src/bin/sdc-replay.rs` and two test files.
   - A search of the added lines finds no `/home/`, `file://`, `[patch]` or name.
   - All 5 commits are `b10x-bot[bot]|b10x-bot[bot]`.

I added six cases in `adversary_pass3.rs` and changed no implementation file.

The tree is clean after the revert: `git status --porcelain` shows 0 lines, and `git apply --check` accepts `<scratch>/removal/line2/adversary-2/pass-3/cases.patch`.

```findings
[
  {"file": "driver/src/bin/sdc-replay.rs", "line": 313, "category": "contract-drift", "severity": "note", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "explain_tests_changed adds \"if that is the only difference, pack the recording again\" to every in/tests.txt change whose text has a label, including a changed test name at the committed label 0.97.3 where the label cannot be the cause (runbook/tests/loom_run/adversary_pass3.rs a_changed_test_at_the_committed_label_is_not_told_to_be_packed_again is red); docs/loom-run.md:386 and CHANGELOG.md:32 say the message names the label as the cause, which only the proven form does. Fix: add the advice only when the current label differs from the mask, and say 'possible cause' in the doc and the CHANGELOG."}
]
```
