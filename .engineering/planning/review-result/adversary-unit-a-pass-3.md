---
format: aep.planning-md/3
id: review-result:adversary-unit-a-pass-3
kind: review-result
status: active
title: 'Narrow check of fix round 2 of unit A (deletion) at b726f25: red, 1 finding'
summary: 'A third, narrow check beyond the two attacks: findings B and D of pass 2 are fixed; 1 introduced warning: the instruction-text guard drops the text after an inline test module, so a line pasted there passes. No scanned file has code after its test module today.'
tags:
- wave-2026-11-22-w1
relations:
- reviews: story:driver-runtime-removed
revision: 1
---
unit: A (deletion) at b726f25, narrow check of fix round 2
verdict: red
origin: introduced 1 / pre-existing 0 / undecided 0
wrote-outside-worktree: `<scratch>/removal/line3-a/adversary/pass-3/` (`cases.patch`, `plan-run.sh`, the probe plans `p1`, `p2`, `q-cond`, `q-member`, and `lintprobe/`)
needs-coordinator: none
cases: 5 added in `driver/tests/bugfix_prompt.rs`, red 1; the tree is clean at `b726f25`
head: author and committer of `b726f25` are both `b10x-bot[bot]`

## (1) Findings B and D of pass 2

**B: fixed, with one hole left.**
- The guard now takes its sources from a directory walk.
  - `driver/tests/bugfix_prompt.rs:560` calls `guarded_sources()`, defined at `:665`.
  - It adds `sdc-node.rs`, then every `.rs` file under `sdc-node/` and `runbook/src/loom_run/` through `program_sources` (`:650`).
  - The mutant is checked on a `sdc-node` file and on `decider.rs` (`:567-571`).
- The hole is the cut for the text after an inline test module (`:642`). It is a new finding, see section 4.

**D: fixed.** The writers are gone.
- `SDC_UNSTICK` is no longer written, the plan member is no longer built, `replay::mark` no longer copies it, and `settings` no longer prints the `unstick` member.
- `--set unstick=…` is refused at `driver/src/bin/sdc-replay.rs:742`, with the refusal text and the plan check at `:1030-1043`.
- `driver/src/bin/sdc-replay.rs:1778` holds the committed test of the refusal.

## (2) The guard: mutants and the exclusion rule

Each mutant was a line from `instructions/decider/` pasted as a string constant at the end of the real file text, run through the guard's own cut.

| mutant | result |
|---|---|
| `runbook/src/loom_run/decider.rs` | caught |
| `runbook/src/loom_run/ledger.rs` (another `loom_run` file) | caught |
| `runbook/src/loom_run/decider/prompt.rs` (a sub-module) | caught |
| `driver/src/bin/sdc-node/call.rs` and `driver/src/bin/sdc-node.rs` | caught, still scanned as before |
| a new file `runbook/src/loom_run/zz_adversary_new.rs` | caught, no edit of the test |
| a new file `runbook/src/loom_run/decider/zz_adversary_new_sub.rs` | caught, no edit of the test |
| non-test code after an inline `#[cfg(test)] mod tests {` (`budget.rs`, `bugfix.rs`, `replay.rs`) | passes, because the rule drops the text after the module |

- **New-file mutant.** I created both files from inside the test, and a drop guard removes them even on a panic. Run beside other guard tests in parallel they race: the other test fails reading a file that just vanished. Run alone it passes. After the runs no `zz_*` file was left.
- **After-the-test-module hole.** `clippy::items_after_test_module` does not cover it. On a scratch crate with an item after a test module, `cargo clippy -- -D warnings` was clean. No file of the scanned directories has code after its inline test module today, so nothing is wrong now.
- **Files excluded by name.**
  - The five files are `answers/tests.rs`, `bindings_copy/tests.rs`, `decider/tests.rs`, `decider/golden.rs` and `decider/requester_tests.rs`.
  - Each is declared in its parent as `#[cfg(test)]` followed by `mod <name>;`. No `x_tests.rs` or `golden.rs` holds non-test code.
  - `sdc-node/` has no such file and no inline test module.
- **Real guard.** `the_rust_binaries_hold_no_instruction_text` passes in 1.7 s.
- **Cases.** Four of the five cases pass, one is red:
  - Green: `adversary3_a_line_pasted_into_a_scanned_file_is_found`.
  - Green: `adversary3_a_new_file_in_the_scanned_directories_is_found`.
  - Green: `adversary3_every_file_excluded_by_name_is_declared_behind_cfg_test`.
  - Green: the committed `the_guard_scans_the_decider_module_root_and_leaves_out_test_code`.
  - Red: `adversary3_a_line_pasted_after_an_inline_test_module_is_found`, whose output is `a line pasted into non-test code after the inline test module of runbook/src/loom_run/budget.rs passes`.

## (3) `unstick` at the head

`git grep -n -i unstick -- driver/src runbook/src bindings scripts cycles ui Taskfile.yml` gives only the implementor's list, and no hit is a writer or a live setting:
- readers of recorded old runs: `bindings/tools/sdc-tool.sh:596`, `driver/src/bin/sdc-findings.rs:59-62`, `driver/src/bin/sdc-finding-kpis.rs:266-291` and `driver/src/bin/sdc-run-compare.rs:446` (a comment);
- pinned `pass_rule` text: `driver/src/bin/sdc-experiment.rs:382`, `:826`, `:1443` and `driver/src/bin/sdc-experiment/progress.rs:11`, `:803`, `:960`, `:1046`;
- `cycles/old-suite-names.txt:71-75`;
- the refusal and its test, plus the doc comment `runbook/src/loom_run/replay.rs:26`.

The refusal runs on the real binary with plans from the stub recording:
- **`prepare --set unstick=on`:** prints the refusal and exits 64. It creates an empty `<out>/replay` directory and writes no file.
- **Plans that name `unstick`:** I made two edited copies, one with `conditions.unstick` and one with an `unstick` member. `program --plan <plan> -- true` refuses both with the same message and exits 127, and no file changes.
- **`env` and `settings`:** they read the plan but do not call the plan check. `settings` on the plan with `conditions.unstick` exits 0 and echoes `"conditions":{"unstick":"on"}`. `env` reads only `env.json`.
- **Judgement:** this is read-only and only reachable with a plan an older `prepare` wrote, so I do not make it a finding. `replay::mark` would put the echoed condition into the run settings of a re-drive that then stops at its first node.

## (4) `git diff 985e8ee b726f25 --stat`

```
 driver/src/bin/sdc-replay.rs          | 58 +++++++++++++--------------
 driver/tests/bugfix_prompt.rs         | 75 +++++++++++++++++++++++++++--------
 runbook/src/loom_run/decider/tests.rs | 30 ++++++++++++++
 runbook/src/loom_run/replay.rs        |  7 ++--
 runbook/tests/cli.rs                  | 39 ++++++++++++++++++
 runbook/tests/loom_run/replay.rs      |  3 +-
 6 files changed, 160 insertions(+), 52 deletions(-)
```

All six files serve B or D, apart from the committed copies of my pass-2 green cases (`decider/tests.rs`, `cli.rs`). The commit is a single one over `985e8ee`, author and committer `b10x-bot[bot]`.

## Tree state

`cases.patch` is saved as `<scratch>/removal/line3-a/adversary/pass-3/cases.patch`. After `git apply -R` and `git reset -q`, `git status --porcelain` is empty, `git apply --check` passes, and HEAD is `b726f25`. No test ran over 2 s. I added no new Python, heredoc or store write. One stray no-op heredoc was typed once and wrote nothing.

```findings
[
  {
    "file": "driver/tests/bugfix_prompt.rs",
    "line": 642,
    "category": "acceptance",
    "severity": "warning",
    "verdict": "NEEDS-CHANGE",
    "origin": "introduced",
    "message": "without_inline_test_module drops everything from `#[cfg(test)]` + `mod x {` to the end of the file, so an instruction line pasted into non-test code that stands after an inline test module passes the guard (red case adversary3_a_line_pasted_after_an_inline_test_module_is_found on budget.rs, bugfix.rs and replay.rs text), and clippy::items_after_test_module does not flag such a file (scratch-crate probe, `-D warnings` clean). No file of the scanned directories has code after its inline test module today; the cut should end at the module's closing brace, or the guard should fail a file that has items after it."
  }
]
```
