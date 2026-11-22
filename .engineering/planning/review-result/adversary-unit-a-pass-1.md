---
format: aep.planning-md/3
id: review-result:adversary-unit-a-pass-1
kind: review-result
status: active
title: Adversary pass 1 on unit A (deletion) of wave 2026-11-22-w1, head e9a5df5
summary: '7 findings: 6 introduced, 1 pre-existing; 2 NEEDS-CHANGE with red cases, 5 CONFIRMED notes. The reviewer''s second sending: the store refused the first one''s findings block (unquoted YAML).'
tags:
- wave-2026-11-22-w1
relations:
- reviews: story:driver-runtime-removed
revision: 1
---
unit: A (deletion) at e9a5df5
verdict: red
cases: executed 497→501, red 2
origin: introduced 4 / pre-existing 1 / undecided 0
wrote-outside-worktree: `<scratch>/line3-a/adversary/` (scripts, logs, `cases.patch`, `scratch-suite/a`); the build outputs under `target/` in `driver/` and `runbook/` (git-ignored, inside the tree)
needs-coordinator: no

The 497→501 count is made of three runs: driver `--test main` 390→392 (full binary), runbook `cli` filtered to `check_`/`doctor_` 30→31, and runbook `--bin` filtered to `loom_run::decider` and `check::` 77→78. The "before" figures are the "after" figures minus my cases. The cases were not run before they existed.

The header counts differ from the findings block: the header says introduced 4 / pre-existing 1, but the block has 6 introduced and 1 pre-existing. I left the header as first sent, because the coordinator asked for no new work, and the block is authoritative.

## 1. Diff stat (test files only; the patch is applied back and the tree is clean)

```
 driver/tests/bugfix_prompt.rs         | 57 +++++++++++++++++++++++++++++++++++
 driver/tests/sdc_node.rs              | 25 +++++++++++++++
 runbook/src/loom_run/decider/tests.rs | 37 +++++++++++++++++++++++
 runbook/tests/cli.rs                  | 11 +++++++
 4 files changed, 130 insertions(+)
```

`runbook/src/loom_run/decider/tests.rs` is a `#[cfg(test)]` file. I added only; no existing case was changed.

`git status --porcelain` is empty after the revert, and `git apply --check <scratch>/line3-a/adversary/cases.patch` passes.

## 2. Cases added, each run alone first

| case | asserts | now | time |
|---|---|---|---|
| `driver/tests/sdc_node.rs` `every_node_a_class_names_is_a_node_some_binding_runs` | every node named in a class of `bindings/tools/node-classes.json` appears in some `bindings/*.yaml` | **red** | 0.07 s |
| `runbook/tests/cli.rs` `check_fails_a_row_whose_state_is_none_of_the_four` | a row of `docs/cycles-suite.md` whose state is `banana` makes `runbook check` fail | **red** | 0.82 s |
| `driver/tests/bugfix_prompt.rs` `sdc_node_and_the_runner_decider_hold_no_instruction_text` | no instruction line of 30+ characters is pasted into `sdc-node` or the runner's decider sources; a pasted line is detected | green | 0.52 s |
| `runbook/src/loom_run/decider/tests.rs` `the_real_claude_cli_adapter_finds_the_login_under_the_environment_the_decider_gives_it` | the real `llm-adapter-claude-cli.sh`, with a stand-in `claude`, finds the login under the decider's environment; the same call with no login does not answer | green | 0.61 s |

No case is over 5 s.

Red output, verbatim:

```
sdc_node::every_node_a_class_names_is_a_node_some_binding_runs
  node-classes.json names nodes that no binding in bindings/*.yaml has: ["unstick: run.unstick"]
cli::check_fails_a_row_whose_state_is_none_of_the_four
  assertion `left == right` failed: check: ok (9 bindings files and 9 JSON bindings, 134 instruction files, 1 views, 54 cycles, 130 compared with docs/cycles-suite.md)
    left: Some(0)  right: Some(1)
```

## 3. Suite runs, after the cases existed (`<scratch>/line3-a/adversary/suite-*.txt`)

- `cd driver && SLOT_TIMEOUT=20m cargo nextest run --locked --no-fail-fast --test main` ran 392 tests: 391 passed, 1 failed (`sdc_node::every_node_a_class_names_is_a_node_some_binding_runs`). It took 65.8 s, exit 100.
- `cd runbook && cargo nextest run --locked --no-fail-fast --test cli -E 'test(/^check_/) | test(/^doctor_/)'` ran 31 tests: 30 passed, 1 failed (`check_fails_a_row_whose_state_is_none_of_the_four`). It took 3.9 s, exit 100.
- `cargo nextest run --locked --no-fail-fast --bin runbook loom_run::decider check::` ran 77 tests (before the last case): all passed. That includes `golden.rs` and `requester_tests.rs`. It took 2.4 s, exit 0.
- `runbook test cycles/tree-suite.yaml --only decider-rework,finding-person-stop,bugfix-not-red --confinement local`, exit 0:
  - `finding-person-stop: as expected (43.5 s)`
  - `decider-rework: pending, waits for story:tree-stop-record-and-labels (approve.result)`
  - `bugfix-not-red: pending, still failing: expect.outcome ... expected fixed, found not_reproduced`
  - I did not run all 54 cycles; the unit's doc measures that at 940 s, far over the 120 s step limit. So the acceptance line for the whole suite is checked on three entries only.

## 4. Findings

| # | file:line | verdict | origin | what was measured / what reaches it |
|---|---|---|---|---|
| 1 | `bindings/tools/node-classes.json:10` (also `:4`) | NEEDS-CHANGE | introduced | The class `unstick` (node `run.unstick`, default effort `xhigh`) is kept. No binding names it, and the program that ran it is deleted. The case is red. It is read by `sdc-reply adapters`, `sdc-node` (efforts), `sdc-replay` and `sdc-protocol-site`. On the base the node was live, run by `sdc-unstick` via `run-trial.sh`. Fix: drop the class and its `default_efforts` entry. Rendered copies under `website/data` belong to unit B. |
| 2 | `runbook/src/check.rs:540` | NEEDS-CHANGE | introduced | `runbook check` acts on a table row only when its state is `converted` or `pending`. The doc says every entry is in one of four states, but the check passes any other word (`banana`, a typo, an empty cell). The base check validated states (`CYCLE_STATES`) in the suite file. Reached: `task check:cycles-suite`, part of `task check`. Fix: reject a state outside the four listed, plus the free-text variants the table now uses ("Rust test", "rust test", "held by Rust tests", "held in part by a Rust test"). |
| 3 | `driver/tests/bugfix_prompt.rs:263` | CONFIRMED | pre-existing | The deleted guard held `decider.sh` and `llm-node.sh` to "no instruction text in a program". The loop now scans every `.sh` in `bindings/tools`, but `sdc-node` and the runner's decider (`runbook/src/loom_run/decider/*.rs`) are in no list of `the_rust_binaries_hold_no_instruction_text`. This is a gap and not a defect: nothing is pasted today (my green case proves it, and proves it can fail). The base had the same gap and its guard sat on the shells. |
| 4 | `docs/cycles-suite.md:136` | CONFIRMED | introduced | The two tests of `questions_check.rs` that ran the real `llm-adapter-claude-cli.sh` under the decider's environment were dropped. The runner's tests put a fake adapter in its place. My green case restores the seam and holds on both sides: with the login it answers, without it it does not. The seam works. |
| 5 | `driver/src/reply/rules.rs:17` | CONFIRMED | introduced | The new `model_step_argv` recognises the deleted script by `name.starts_with("llm-node.")` and so avoids the acceptance grep. It also matches more than the old `ends_with("/llm-node.sh")`. It exists because the kept `*.bindings.json` twins still name `llm-node.sh` (unit B). Reached by `sdc-check-routes` (`is_llm_node`) and `program_check`. Remove it when unit B lands. |
| 6 | `bindings/tools/sdc-tool.sh:3` | CONFIRMED | introduced | The header says the tool is run "in driver mode, by the variables of the removed driver", and `:13` says it is run as under the driver. The `SDC_CASE`-only branches are reached only by `sdc-conform` and tests (`driver/tests/impl_gate.rs`, `driver/tests/questions_check.rs:379`), not by a run. The text misleads. |
| 7 | `driver/src/bin/sdc-check-routes.rs:164` | CONFIRMED | introduced | The route check still reads `ends["driver"]` (`own_failure_fed_back`, `rework_limit`, `capped_effects`) as capabilities of the deleted driver. The unit dropped the own-failure feedback goldens as "left with the driver", yet this guard still demands the capability of every writer binding. |

Severity: 1 and 2 are warnings, 3 to 7 are notes. Finding 4 is a note that carries the green case.

## 5. Attacked, not broken

- The five files are gone and `[[bin]] sdc-drive` is gone. The package is `sdc-tools` and 26 files are deleted, exactly the unit's list.
- The acceptance grep prints 666 lines.
  - The lines in unit A's areas are only named history in `docs/cycles-suite.md`, in the history groups: 3, 184, 208, 215, 269, 270, 282, 312.
  - Per top directory the rest is: docs 366, website 85, experiments 49, CHANGELOG.md 42, ui 38 (all `ui/test/recorded`), driver 28 (all `driver/examples`, unit B), cases 24, bindings 11, AGENTS.md 10, eval 6, plugin 4, README.md 3.
  - `docs/cycles-suite.md` lines 6 and 58 now name `cycles/old-suite-names.txt` and `runbook check`.
- `Taskfile.yml` is valid (`task --list-all` works, no dry-run line names a deleted file). `.github/` names no deleted file.
- `sdc-check-unreferenced` exits 0.
- `cycles/old-suite-names.txt` equals the 130 ids of the base `cycles/suite.yaml`; the tree suite has 54 cycles, all in the list, and 17 carry `pending: true` as the Totals section says.
- Every Rust test named in a row of `docs/cycles-suite.md` exists as a `fn`. The `#[ignore]`d holders are rows of the "Re-drive" section that says so (line 190), plus the disclosed `bugfix-from-ticket` slow lane. A shell check of this is in `<scratch>/line3-a/adversary/`.
- The two re-blessed requester goldens are what the program sends.
  - `in/goals.yaml` is listed because the file exists.
  - The retry text `the grounding check refused it: goals@g1#G9: no such entry` is pinned by an independent assertion at `requester_tests.rs:307-311`.
  - The 12 decider prompts and the 3 requests are unchanged from the base goldens.
- The absence tests still assert something. `concat!("sdc-dr", "ive")` evaluates to the old name, so the `FORBIDDEN` lists and `OLD_LAUNCHER`/`OLD_DRIVER` would go red if the name came back.
- The login never leaves the child's environment: it is held by the runner's `the_login_path_reaches_the_adapter_environment_only_and_is_redacted...`.
- `--confinement` has no default on `runbook run` and `runbook test`. `runbook/src/loom_run` changed only in comments, plus the decider goldens.
- The `watch/tests/e2e.sh` cases (sed -i, rustfmt, a new file, the action window) are held by `watch/tests/watch.rs`.
- The `sdc-experiment` launch and stop tests assert `runbook run` and no script.
- `runbook check` and `doctor` tests: the 30 existing ones pass.
- `docs/driver.md`, `docs/parity.md`, `docs/kpis.md`, `docs/orchestration.md`, `docs/eval-design.md`, `docs/trial-2.md` and `AGENTS.md` were edited only through the merged commit `f2c6a12`, which the brief ordered. Unit A wrote none of them itself.

## 6. Paths written outside the worktree

- `<scratch>/line3-a/adversary/`: `cases.patch`, `grep.txt`, `row-tests-exist.sh`, `row-tests-ignored.sh`, `ignored-in-class-d.sh`, `instr-text-in-src.sh`, `orphan-instructions.sh`, `test-names.sh`, the `*-tests.txt`, `*-rows.txt`, `*-ids.txt` and `names.txt` lists, `run-*.txt` and `suite-*.txt` logs.
- `<scratch>/line3-a/adversary/scratch-suite/a/`: the named `--scratch` directory of the `runbook test` subset. `runbook test` never removes a directory it is given by name.
- Build and test outputs under `<trees>/b10x/runbook/line3-a/driver/target` and `<trees>/b10x/runbook/line3-a/runbook/target` (git-ignored).

```findings
[
  {
    "file": "bindings/tools/node-classes.json",
    "line": 10,
    "category": "acceptance",
    "severity": "warning",
    "verdict": "NEEDS-CHANGE",
    "origin": "introduced",
    "message": "the class `unstick` (node `run.unstick`, default effort `xhigh` at line 4) outlived `sdc-unstick` and `run-trial.sh`; no binding names the node, and the red case `every_node_a_class_names_is_a_node_some_binding_runs` prints [\"unstick: run.unstick\"]."
  },
  {
    "file": "runbook/src/check.rs",
    "line": 540,
    "category": "contract-drift",
    "severity": "warning",
    "verdict": "NEEDS-CHANGE",
    "origin": "introduced",
    "message": "`runbook check` reads only table rows that say converted or pending, so a row of docs/cycles-suite.md with any other state word passes (red case `check_fails_a_row_whose_state_is_none_of_the_four`), though the page says every entry is in one of four states and the base check validated states."
  },
  {
    "file": "driver/tests/bugfix_prompt.rs",
    "line": 263,
    "category": "mutant",
    "severity": "note",
    "verdict": "CONFIRMED",
    "origin": "pre-existing",
    "message": "the deleted guard held decider.sh and llm-node.sh to \"no instruction text in a program\" and sdc-node and the runner's decider are in no list of the_rust_binaries_hold_no_instruction_text; the green case sdc_node_and_the_runner_decider_hold_no_instruction_text closes the gap and detects a pasted line."
  },
  {
    "file": "docs/cycles-suite.md",
    "line": 136,
    "category": "mutant",
    "severity": "note",
    "verdict": "CONFIRMED",
    "origin": "introduced",
    "message": "the two deleted tests that ran the real claude-cli adapter under the decider's environment left no test of that seam (the runner's tests use a fake adapter); the green case the_real_claude_cli_adapter_finds_the_login_under_the_environment_the_decider_gives_it restores it and shows the seam holds with and without a login."
  },
  {
    "file": "driver/src/reply/rules.rs",
    "line": 17,
    "category": "judgement",
    "severity": "note",
    "verdict": "CONFIRMED",
    "origin": "introduced",
    "message": "model_step_argv recognises the deleted script by starts_with(\"llm-node.\") (wider than the old ends_with(\"/llm-node.sh\")) and keeps the name out of the acceptance grep; it exists only because the kept *.bindings.json twins still name llm-node.sh (unit B), and it should go when they change."
  },
  {
    "file": "bindings/tools/sdc-tool.sh",
    "line": 3,
    "category": "contract-drift",
    "severity": "note",
    "verdict": "CONFIRMED",
    "origin": "introduced",
    "message": "the header still describes a \"driver mode\" run \"by the variables of the removed driver\" although the SDC_CASE-only branches are reached only by sdc-conform and tests, not by any run."
  },
  {
    "file": "driver/src/bin/sdc-check-routes.rs",
    "line": 164,
    "category": "judgement",
    "severity": "note",
    "verdict": "CONFIRMED",
    "origin": "introduced",
    "message": "the route check still demands the capabilities of the deleted driver (`own_failure_fed_back`, `rework_limit`, `capped_effects` read from ends[\"driver\"]) of every writer binding although the own-failure feedback was dropped with the driver."
  }
]
```
