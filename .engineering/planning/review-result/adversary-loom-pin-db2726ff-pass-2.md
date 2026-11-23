---
format: aep.planning-md/3
id: review-result:adversary-loom-pin-db2726ff-pass-2
kind: review-result
status: active
title: 'Adversary pass 2 of the unit loom-pin-db2726ff at 36c625f3: red, 1 warning, 4 notes'
summary: 'Findings 1, 2 and 4 of pass 1 are fixed, 3 and 7 in part. Two pages say a library run ends exit 12 on a refused handoff; the library runs no handoff. Notes: the page test is one-way, the exact text of the level refusal is asserted only outside the pool, exit 2 is called a usage error in general, a false exit 12 needs a forged child.'
relations:
- reviews: story:loom-pin-db2726ff
revision: 1
---
verdict: red

unit: runbook, fix round 1 (`d583018f` `36c625f3` over `0aacfd3f`, Loom pin db2726ff), tree `<trees>/b10x/runbook/pin-loom` at `36c625f3`
verdict: NEEDS-CHANGE (1 warning, 4 notes; no blocker)
cases: 10 new in 2 untracked files (6 in `adv_child2.rs` green, 2 in `adv_pages.rs` green now and red under two page mutations, 2 library probes run from a copy of the Rig and deleted: green); the round's own 12 touched tests green
origin: introduced 5 / pre-existing 0 / undecided 0
wrote-outside-worktree: logs and scratch only, listed in part 6
needs-coordinator: none

## 1. `git --no-pager diff --stat`

Tracked files: only the coordinator's record, `.engineering/planning/story/loom-pin-db2726ff.md` (3 insertions, 2 deletions). Untracked: the coordinator's `.engineering/planning/review-result/adversary-loom-pin-db2726ff-pass-1.md`, and mine `runbook/tests/adv_child2.rs`, `runbook/tests/adv_pages.rs`. No implementation file touched.

## 2. Pass-1 findings 1, 2, 3, 4, 7

| # | state | shown by |
|---|---|---|
| 1 `--loom` approve gap | fixed | `child_refusal.rs` (4 tests: run, approve, open, no-reason stays exit 1) green; my `adv2_a_refused_handoff_...` (a refused handoff, exit 12, cause `tree_stopped` in `outcome.json` and `run-stopped.json`) green. The handoff goes through `parse_run`, which reads the line for exit codes outside 0, 3, 4, 5. |
| 2 doc drift (run-directory page, `outcome.json` members) | fixed | `loom-run-directory.md:76` lists the 29 and `program_signalled`; the `outcome.json` members line has `cause`; `pages.rs` 5 tests green; mutation M1 (a cause removed from the page) and M4 (the list pointer removed) turn it red. Remaining gaps in the tests, finding C below. |
| 3 `check_level` text | fixed in part | The page and CHANGELOG now give the exact text, and say it has no `cause` and no `ConfinementRefused:` prefix. True: `a_level_the_library_cannot_give_is_refused_before_anything_is_written` run outside the pool (this shell's cgroup is not delegated) passes with the text asserted. Inside the pool it prints "this host delegates a cgroup" and returns: it asserts nothing there (finding D). |
| 4 vacuous UI case | fixed | `git diff --quiet 566a0c3f 36c625f3 -- ui/test/event-log-lines.spec.mjs` exits 0: the file is as before the unit. Page and CHANGELOG say the inspector reads the projected `events.jsonl` and has no test of its own for the two kinds. |
| 7 reach of exit 12 / status stays 1 | fixed in part | Sentences added to the page and CHANGELOG; new `a_bindings_file_loom_refuses_at_the_opening_...` green (`loom.bindings/99`: exit 12, `bindings_refused`, `run-stopped.json` cause, no program ran). One clause is wrong for the library: finding B. |

Pass 1 finding 8 (story text) and 5, 6, 9 are not part of this pass.

## 3. Cases added

`runbook/tests/adv_child2.rs` (child path, fake `b10x-loom`), all green:

- `adv2_a_refused_handoff_ends_the_run_refused_with_its_reason_as_the_cause`: guard for the handoff, which no test of the round covers.
- `adv2_a_reason_inside_another_message_stays_a_loom_failure`: the reason text after other words, and an indented line: exit 1, no cause.
- `adv2_a_line_that_names_no_reason_stays_a_loom_failure`: unknown name, empty name, wrong case: exit 1.
- `adv2_the_first_of_two_refusal_lines_wins_and_the_second_is_not_the_cause`.
- `adv2_a_refusal_line_on_a_tree_run_that_exits_with_a_known_code_is_not_a_refusal`: exit 0 with a refusal line is not exit 12.
- `adv2_the_stderr_tail_of_a_refusal_holds_no_path_or_environment_value_outside_the_run`: the child prints `HOME` and an environment token before its refusal line; the console, `outcome.json`, `run-stopped.json` and `events.jsonl` hold neither the token nor a home path.

`runbook/tests/adv_pages.rs`, the reverse of `pages.rs`, green now:

- `adv_the_list_and_the_table_of_causes_name_no_cause_the_code_does_not_have`.
- `adv_the_pages_count_the_reasons_as_refusal_rs_does` (`The N causes`, `one of N`, CHANGELOG `(N, one cause each`).

Two library probes (a refused opening: files of the run hold no path outside the run; console text) ran from a copy of `inproc.rs`, green, then deleted: they added no red.

## 3a. Red output (mutations of the pages, restored byte for byte; sha256 of both pages equal before and after, `git status` clean for `docs/`)

| mutation | `pages.rs` (the unit's) | `adv_pages.rs` (mine) |
|---|---|---|
| M1 page loses `tree_stopped` | red: `docs/loom-run.md does not list these causes of refusal.rs` | green |
| M2 page names `ghost_cause` and `ghost_cause_two` | 5 passed | red: `docs/loom-run.md names causes refusal.rs does not have` |
| M3 page says `The 31 causes`, `one of 33` | 5 passed | red: `docs/loom-run.md does not say 'The 29 causes'` |
| M4 run-directory page names no list | red | green |
| M5 `check_level` sentence loses its tail | 5 passed | green |

So (b): a cause added to `refusal.rs` and not to the page fails (M1; the `cause` is matched as `` `name` `` over the whole page); a page that names a cause that does not exist, or a stale count, does not fail the unit's tests (M2, M3). The count (29) is written in 3 places and held by none.

## 4. Suite run, verbatim

```
cargo nextest run --locked --no-fail-fast -E 'binary(adv_child2) | (binary(adv_lib) & test(adv_lib))'
     Summary [  25.552s] 47 tests run: 46 passed, 1 failed, 57 skipped
```
The 1 failure was my own first probe (it looked at the console, where the absolute run path of the person's own `--runs-dir` is expected); corrected to the files of the run, then:
```
cargo nextest run --locked --no-fail-fast -E 'binary(adv_lib) & test(adv_lib)'
     Summary [   0.092s] 2 tests run: 2 passed, 57 skipped
cargo nextest run --locked --no-fail-fast --no-capture -E 'binary(cli) & (test(loom_run::pages) | test(loom_run::child_refusal) | test(a_level_the_library_cannot_give) | test(a_bindings_file_loom_refuses) | test(the_settings_file_must_agree))'
     Summary [   9.054s] 12 tests run: 12 passed, 548 skipped
cargo nextest run --locked --no-fail-fast -E 'binary(adv_pages)'
     Summary [   0.084s] 2 tests run: 2 passed, 0 skipped
```
The 47 counted 41 `loom_run::core::` and `decider::` tests that my `adv_child2` binary pulls in by `mod`; 6 are mine. No test over 7 s (slowest 6.6 s). Host load 70 to 170, no time over a bound. The tree suite and the test step were not run whole.

## 5. Findings

| # | severity | verdict | origin | where | what |
|---|---|---|---|---|---|
| A | note | CONFIRMED (by reading, probe green) | introduced | `runbook/src/engine/refusal.rs:139` `in_stderr`, `child_host.rs:55` | (a) A false exit 12 needs a line that starts with `b10x-loom: tree refused: ` and names a reason. Loom prints that line once per refusal and escapes control characters with `printable` (tree.rs:64, :658), so a program's output (it goes into a blob; the refusal texts name an exit code and a digest, never the output: program.rs:1288, :1321; approve.rs:350) cannot add one; my cases show a reason inside another message, an indented line, an unknown name, a known exit code, and two lines each end as they should. Only a child that is not Loom could forge it. No change asked. |
| B | warning | CONFIRMED | introduced | `docs/loom-run.md` (bullet "In a library run every refusal ..."), `CHANGELOG.md:48` | Both say a library run ends exit 12 for "a refusal of `tree run`, of an answer and of a handoff". The library runs no handoff: `library.rs:195` `handoff` returns `Halt::Failed("the library runs no call for a handoff")` and `runs_handoff` is false. The clause is true for `--loom` only. Also: the Changelog's `StopUnusable`; The whole change ...` (a capital after a semicolon) and two `Tests:` sentences in one bullet (the second one lists the first round's tests); and no test of the round covers a refused handoff with `--loom` although both pages name it (my `adv2_a_refused_handoff_...` is the guard). |
| C | note | CONFIRMED (mutation) | introduced | `runbook/tests/loom_run/pages.rs:23` | The page test is one-way (M2, M3 above): a ghost cause or a stale count passes. `runbook/tests/adv_pages.rs` holds both directions and is green now; it is red under M2 and M3. |
| D | note | CONFIRMED (run in and out of the pool) | introduced | `runbook/tests/loom_run/inproc.rs:251`, `library.rs:222` | The exact text of the `check_level` refusal is asserted only by a test that returns at once when the host delegates a cgroup, which the pool's jobs do (run in the pool: "this host delegates a cgroup ... no level to refuse here"; outside it, from this shell: it runs and passes). The unit test of `check_level` asserts only that the text holds `substrate` and `local`. In the gate the code's text is therefore held by nothing; `pages.rs` checks the page against a literal. The sentence is true: measured outside the pool. |
| E | note | CONFIRMED (by reading) | introduced | `docs/loom-run.md` ("Exit 2 ... a usage error") against its own line "64 usage" | Exit 2 is called a refusal "before a run directory exists (a usage error, ...)". True for `--loom` without `--loom-sha256` (measured: `runbook run: refused: usage: ... go together`, exit 2, nothing written) and false for a missing or invalid option (clap, measured: exit 64). The page says both numbers without saying which usage error is which. Also (c): "tree status and tree verify after the run has ended stay exit 1" is true for both paths, and mid-run `tree status` refusals (`refresh_status`) are exit 1 too with no cause; with `--loom` the label is `error` and `loom_exit_code` is set. |

(a) second half, what the tail carries: `stderr_tail` is the last 1500 characters of the child's standard error, with the run directory made relative and nothing else cleaned. Measured with a child that prints `HOME` and an environment token: the token is not there (the child's environment is built, not inherited) and `HOME` is a path inside the run (relative). Real Loom puts no ticket text on standard error: the lines are its refusal line, its refusal JSON, and `tree waits`/`run:` lines for decisions, which come with exit 3, not with a refusal. The console prints the refusal's text with Loom's absolute paths (for example the run's own `bindings/<file>` path under the person's `--runs-dir`), as it did for every failure of a child before the round; the files of the run (library path measured) hold none. Nothing to change.

## 6. What held

- (d) UI spec file identical to `566a0c3f`.
- The new bindings-refused test, `child_refusal.rs` (4) and `pages.rs` (5) green; the round's table of causes matches `refusal.rs` (every backticked snake-case token of the page is a cause or a non-cause word).
- No cause is written for exit 2; `check_level` text and exit 2 as the page says (measured outside the pool).
- `parse_run` reads the line for every exit code outside 0, 3, 4, 5, so a handoff refusal with `--loom` is exit 12.
- Pass-1 note 8 (story text) not examined.

Paths written outside the worktree: `<scratch>/pin-db2726ff/adv/` (logs `run1.txt`, `run2.txt`, probe sources, `mutate.sh`, `mutate2.sh`, backups of the two pages `*.orig`), this report. `cargo clean` ran in `runbook/` (7,662 files, 1.3 GiB); `driver/` and `watch/` had no `target/`. The pages `docs/loom-run.md` and `docs/loom-run-directory.md` were edited in place by `mutate.sh` for a few seconds each and restored from the backups (sha256 equal, `git status` clean).

## 7. Findings block

```findings
[
  {"file": "docs/loom-run.md", "category": "documentation", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "B: the pages (also CHANGELOG.md line 48) say a library run ends exit 12 for a refused handoff; the library runs no handoff (runbook/src/loom_run/library.rs line 195). True with --loom only. No test of the round covers a refused handoff with --loom. Changelog punctuation and a doubled Tests sentence."},
  {"file": "runbook/tests/loom_run/pages.rs", "line": 23, "category": "test-strength", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "C: the page test fails for a missing cause, not for a cause the code does not have or a stale count of 29."},
  {"file": "runbook/tests/loom_run/inproc.rs", "line": 251, "category": "test-strength", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "D: the exact check_level text is asserted only on a host with no delegated cgroup; the pool's jobs have one, so the gate asserts only substrings."},
  {"file": "docs/loom-run.md", "category": "documentation", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "E: exit 2 is called a usage error in general; that holds for --loom without --loom-sha256, option errors are exit 64."},
  {"file": "runbook/src/engine/refusal.rs", "line": 139, "category": "boundary", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "A: a false exit 12 needs a line only a forged child can print; the stderr tail holds no token and no path outside the run (probe green). No change asked."}
]
```
