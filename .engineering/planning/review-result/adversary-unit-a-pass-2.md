---
format: aep.planning-md/3
id: review-result:adversary-unit-a-pass-2
kind: review-result
status: active
title: 'Adversary pass 2 on unit A (deletion) at 985e8ee: red, 3 findings'
summary: 'Second attack on impl/line3-deletion: findings 1, 2, 4 and 6 of pass 1 fixed, 3 in part; 2 introduced warnings (the guard does not scan the decider module root; leftovers of the unstick condition), 1 pre-existing. The reviewer resent the report with one home path replaced; the resend is recorded.'
tags:
- wave-2026-11-22-w1
relations:
- reviews: story:driver-runtime-removed
revision: 1
---
unit: A (deletion) at 985e8ee, pass 2
verdict: red
cases: executed driver `main` 391→393, runbook `cli` 485→488, runbook bin 133→134 (6 added), red 3
origin: introduced 2 / pre-existing 1 / undecided 0
wrote-outside-worktree: `<scratch>/removal/line3-a/adversary/pass-2/` (`cases.patch`, one probe output dir `pr/`)
needs-coordinator: none. Findings 1, 2 and 6 are fixed. Finding 3 is fixed in part (new finding B). Finding 4 is fixed.

## 1. git --no-pager diff --stat (cases only, tree reverted afterwards)

```
 driver/tests/bugfix_prompt.rs         | 30 +++++++++++++++++++
 runbook/src/loom_run/decider/tests.rs | 30 +++++++++++++++++++
 runbook/tests/cli.rs                  | 55 +++++++++++++++++++++++++++++++++++
 3 files changed, 115 insertions(+)
```

Saved as `<scratch>/removal/line3-a/adversary/pass-2/cases.patch`. After `git apply -R` and `git reset -q`, `git status --porcelain` is empty and `git apply --check cases.patch` passes. HEAD is still 985e8ee.

## 2. Cases added

Red (3):

- `runbook/tests/cli.rs` `adversary_check_fails_a_held_row_whose_named_test_does_not_exist`. A `held by a Rust test` row names `replay_unchanged_is_a_test_that_nobody_ever_wrote_at_all`.
  ```
  assertion `left == right` failed: check: ok (9 bindings files and 9 JSON bindings, 134 instruction files, 1 views, 54 cycles, 130 compared with docs/cycles-suite.md)
    left: Some(0)  right: Some(1)
  ```
- `driver/tests/bugfix_prompt.rs` `adversary_the_guard_scans_the_decider_module_root`. The case rebuilds the guard's scan set with the same `program_sources` calls.
  ```
  the guard does not scan <trees>/b10x/runbook/line3-a/runbook/src/loom_run/decider.rs: a pasted instruction line there passes
  ```
- `driver/tests/bugfix_prompt.rs` `adversary_the_guard_finds_an_instruction_line_cut_in_two`. This is evidence for the judgement note in section 4, not a finding.
  ```
  a line cut in two pieces passes the scan: `met_after_this_action` in in/checks.json lists what can only be met by a step after {{action}}: …
  ```

Green (3):

- `adversary_check_fails_every_near_miss_of_a_state_word` tries 10 variants and all exit 1: capitalised, upper case, trailing `.`, trailing text, `**bold**`, backticks, a double space, a non-breaking space, an empty cell, and two words joined by a comma.
- `adversary_check_fails_a_row_cut_to_one_cell` gets the finding `entry pool-wait … no row`.
- `adversary_no_file_under_the_run_directory_holds_the_login_path` walks every file under the run directory after the real `llm-adapter-claude-cli.sh` ran. None holds the login path.

My first draft of the one-cell case was wrong. It cut only the state cell and left the third cell, so the check read the reason text as the state. It reported that as a state finding, which is also correct.

## 3. Suite runs

All through `cargo nextest` with `SLOT_TIMEOUT=20m`. Every step took under 10 s and no test took over 1 s.

- `task check:cycles-suite` prints `check: ok (… 54 cycles, 130 compared with docs/cycles-suite.md)`. It agrees with `runbook check`, which accepts every row of the page.
- The page table has 39 held, 37 dropped, 17 pending and 37 converted rows. That equals the headline on `docs/cycles-suite.md:106`. 130 rows equals the 130 lines of `cycles/old-suite-names.txt`.
- runbook `--test cli check_`: 27 run, 26 passed, 1 failed (the held-row case above).
- runbook `--bin runbook decider`: 79 passed, including my login-walk case and the kept adapter case.
- driver `--test main bugfix_prompt sdc_node`: 172 run, 170 passed, 2 failed (the two guard cases). `the_rust_binaries_hold_no_instruction_text` and `every_node_a_class_names_is_a_node_some_binding_runs` both pass.
- `sdc-experiment check --all` prints `experiments: ok`.
- The adapter case passes with `claude` taken off `PATH` (the real one is `<home>/.local/bin/claude`). It uses its own stand-in `claude` script and makes no model call.

## 4. Judgement findings

Findings 1, 2, 3, 4 and 6 of pass 1:

- **1, fixed.** The `unstick` class and its `default_efforts` entry are gone from `bindings/tools/node-classes.json`; `grep unstick` there returns 0 lines. `driver/tests/sdc_node.rs:1767` (`every_node_a_class_names_is_a_node_some_binding_runs`) holds the class list. Leftovers outside the class are in new finding D.
- **2, fixed.** `runbook/src/check.rs:421` (`TABLE_STATES`) and `:546-552` refuse any state outside the four words and name the entry. The 10 near-miss variants and the one-cell row are all caught.
- **3, fixed in part.**
  - `driver/tests/bugfix_prompt.rs:532-572` now scans `sdc-node` and `decider/*.rs`. Nothing there holds instruction text today.
  - Mutants:
    - Pasted line into an `sdc-node` file: caught (the guard's own mutant uses `sources[0]`).
    - Pasted line into a `decider/` file: caught, the same scan covers it.
    - Pasted line into `conform.rs` or `implement/{mod,tree,reply,assemble,run}.rs`: caught, the original loop still runs.
    - Pasted line into `runbook/src/loom_run/decider.rs` (601 lines, the decider's module root), `ledger.rs`, `decide.rs` or `bugfix.rs`: not caught. This is finding B.
  - A line cut in two with `concat!`, or with one character changed, is not caught. The base guard matched the same way, so I do not count it as a finding.
  - A scan of every `.rs` file in `driver/src` and `runbook/src` finds only test code holding instruction text today: `sdc-bugfix.rs:1775` and `decider/tests.rs:1087`.
- **4, fixed.** `runbook/src/loom_run/decider/tests.rs:717` restores the seam. It writes no login path into any kept file (the walk case above), and the no-login mutant returns `CannotDecide`.
- **6, fixed.** `bindings/tools/sdc-tool.sh:1-10` now says Loom starts it, and that the `SDC_CASE`-only branches are reached by `sdc-conform` and by `impl_gate.rs` and `questions_check.rs`. One small remainder: `sdc-node/setup.rs:105` also sets `SDC_CASE` before it runs `sdc-tool.sh`. I did not make that a finding.

## 5. Attacked, not broken

- The state check refuses every near-miss variant I tried, and a row with fewer cells.
- Every test function a row names exists as a `fn`. My script `row-tests-exist.sh` prints only `rejected_without_rework_path`, which is a cause string and not a test.
- `--set unstick=on` is refused by name under `runbook run` (`sdc-replay.rs:945`).
- No `unstick` remains in `eval/`. The `docs/progress.json` and `experiments/*` records are history.
- `sdc-experiment check --all` passes.
- The adapter case needs neither a login file in anything written nor a real `claude`.

Not a finding, from the cut-in-two case: it is evidence only.

## 6. Paths written outside the worktree

- `<scratch>/removal/line3-a/adversary/pass-2/cases.patch`
- `<scratch>/removal/line3-a/adversary/pass-2/pr/` (one `sdc-replay prepare --plan-only` probe, which refused `unstick` and wrote nothing)

```findings
[
  {
    "file": "docs/cycles-suite.md",
    "line": 197,
    "category": "acceptance",
    "severity": "warning",
    "verdict": "NEEDS-CHANGE",
    "origin": "pre-existing",
    "message": "A row `held by a Rust test` names the test that holds it, and `runbook check` does not read the named test: a row naming a test that nobody wrote passes with exit 0 (red case adversary_check_fails_a_held_row_whose_named_test_does_not_exist; the check also lives at runbook/src/check.rs:546-566). At 4677c2b the table was not read at all (no table_rows in check.rs), so the 32 held rows there were unchecked too. All named tests exist at 985e8ee (checked by script), so nothing is wrong today."
  },
  {
    "file": "driver/tests/bugfix_prompt.rs",
    "line": 562,
    "category": "acceptance",
    "severity": "warning",
    "verdict": "NEEDS-CHANGE",
    "origin": "introduced",
    "message": "The merged guard scans runbook/src/loom_run/decider/*.rs but not the decider's module root runbook/src/loom_run/decider.rs (601 lines of its messages and calls), and no other runbook/src/loom_run/*.rs; the deleted guard held decider.sh, so a line pasted into the replacement's root passes (red case adversary_the_guard_scans_the_decider_module_root). No instruction text is in those files today."
  },
  {
    "file": "driver/src/bin/sdc-replay.rs",
    "line": 749,
    "category": "acceptance",
    "severity": "warning",
    "verdict": "NEEDS-CHANGE",
    "origin": "introduced",
    "message": "Leftovers of the removed unstick class after the readers (run-trial.sh, sdc-unstick, llm-adapter-fixture.sh) were deleted: sdc-replay writes SDC_UNSTICK at lines 749 and 897 and builds the unstick plan member at 891-914 with its test at 1785, and runbook/src/loom_run/replay.rs:189 copies it into the run settings, though nothing reads the variable; loom_settings (sdc-replay.rs:945) refuses the same condition by name. docs/driver.md:751, 755 and 1026 still describe Unstick as a live feature (unit C's page)."
  }
]
```
