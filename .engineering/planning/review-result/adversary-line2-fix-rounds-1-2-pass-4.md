---
format: aep.planning-md/3
id: review-result:adversary-line2-fix-rounds-1-2-pass-4
kind: review-result
status: active
title: Adversary confirmation of line 2 fix rounds 1 and 2 at bb44b12, with the live-cap fix
summary: 'Red on two pre-existing notes: a call log with a byte that is not UTF-8 reads as empty and lifts the live cap; a cut-off last line makes the next started line lost. The wording note of pass 3 is confirmed fixed; six of eight cap attacks hold.'
tags:
- wave-2026-11-22-w1
relations:
- reviews: story:driver-runtime-removed
revision: 1
---
```
unit: line 2 fix rounds 1 and 2 at bb44b12, confirmation
verdict: red
cases: executed 36 in the named modules (8 added in adversary_pass4), red 2
origin: introduced 0 / pre-existing 2 / undecided 0
wrote-outside-worktree: <scratch>/removal/line2/adversary-2/pass-4/ (cases.patch, run1.log); nextest tmp dirs under runbook/target/tmp (ignored by git)
needs-coordinator: no
```

**Point 1: confirmed.**
- `replay_boundaries::a_changed_test_at_the_committed_label_is_not_told_to_be_packed_again` is green in the tree.
- `docs/loom-run.md:392-396` and `CHANGELOG.md:32` match the two forms. The label is named as the cause, with the repack advice, only when proven (the unmasked digest is the recorded one). Otherwise the message says the file changed and names the old-label case as one possible reason, with no advice.

**Point 2: the cap holds in 6 of 8 cases. The two reds are log-content cases.**
- **Cap 1, first call killed after its start:** the second call exits 75 and there is 1 live start. Green.
- **Races:** 2 and 4 processes at cap 1, and 4 and 2 at cap 2, 4 rounds each. Live starts were never above the cap, and always equal to `min(cap, processes)`. Green, 6.0 s.
- **Counted once:** at cap 2, two calls finish and the third gets 75, with 2 starts. A log without phases holding one finished live call at cap 1 refuses the call, with 0 starts. Green.
- **Empty log:** at cap 1 the first call starts and the second gets 75. Green.
- **Log is a directory, or mode 000:** the adapter exits non-zero and no live program starts. Green.
- **No readable cap** (absent, `"2"`, `-1`): exit 75 and 0 starts. Green.
- **Cut-off last line, RED (0.40 s):** the log holds `{"mode": "live", "phase": "star` with no newline, at cap 1. Three calls gave exit codes `[0, 0, 75]` and 2 live starts.
  - `append` writes the next started line onto the fragment, so that line is lost.
  - The next count sees 0.
- **Byte that is not text, RED (0.18 s):** the log has one started live line and then `\xff\xfe`, at cap 1.
  - The call exits 0 and starts a live program.
  - `jsonl` (`sdc-replay.rs:148`) reads with `read_to_string(..).unwrap_or_default()`, so a log that is not valid UTF-8 reads as empty and the count is 0. That lifts the cap.
- **Both reds reproduce on `main`.** `199f311ffb` has the same `jsonl` and the same count of live lines, and a cut-off finished line glues the same way. So both are pre-existing, and the fix did not make them worse.
- **Other readers of the log, by reading the code and the script (I did not run the script):**
  - `summary` counts live as `live_calls` (started lines and phase-less lines, not finished lines), so a live call counts once.
  - `paid`, `faults` and `prompt_changed` count only on the finished line, because the started line carries a null fault, no `paid` and `prompt_changed` false.
  - The `rounds.jsonl` share filters `clock == "virtual"`, and the started line says `real`, so it is not summed.
  - `seal` reads `calls.jsonl` only for that share, and takes modes from each record's `replay.json`.
  - `scripts/run-finding-cycles.sh:2852` checks `.replay.calls.live == 1` from `run-stopped.json`, which comes from `summary`, so it still gives 1.
  - The checks at `:2711` and `:2738` expect `live == 0`.
  - `:2739` reads only recorded and reused lines. None of the three is affected.

The new cases are in `<scratch>/removal/line2/adversary-2/pass-4/cases.patch`. After `git apply -R` and `git reset -q`, `git status --porcelain` is empty, and `git apply --check` accepts the patch. Slowest new test is 6.0 s (the race case).

```findings
[
  {"file": "driver/src/bin/sdc-replay.rs", "line": 148, "category": "acceptance", "severity": "note", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "jsonl() reads the call log with read_to_string(..).unwrap_or_default(), so a log with a byte that is not UTF-8 reads as empty, the live count is 0 and the live cap is lifted: one started live line plus \\xff\\xfe at cap 1 still starts a live program (runbook/tests/loom_run/adversary_pass4.rs a_log_with_a_byte_that_is_not_text_does_not_lift_the_cap is red); same function and same count on main 199f311ffb; read the log as bytes and parse line by line, or refuse a live start when the log cannot be read as text."},
  {"file": "driver/src/bin/sdc-replay.rs", "line": 184, "category": "acceptance", "severity": "note", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "append() adds its line to a log whose last line was cut off without a newline, so the new line is glued to the fragment and lost: with a cut-off line at cap 1 three calls gave exit codes 0, 0, 75 and two live starts, because the started line of the first call was lost (adversary_pass4.rs a_cut_off_last_line_does_not_make_a_call_free is red); the same glue happens on main 199f311ffb for a finished line; under the lock, write a newline first when the file does not end in one."}
]
```
