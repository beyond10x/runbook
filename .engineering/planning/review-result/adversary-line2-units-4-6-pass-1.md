---
format: aep.planning-md/3
id: review-result:adversary-line2-units-4-6-pass-1
kind: review-result
status: active
title: Adversary pass 1 on units 4 and 6 of line 2, wave 2026-11-22-w1, head b2ce1cb
summary: '7 findings: 6 introduced, 1 undecided; the replay lane is red at the head (5 of 9).'
tags:
- wave-2026-11-22-w1
relations:
- reviews: story:driver-runtime-removed
revision: 1
---
```
unit: units 4 and 6 and their seam at b2ce1cb (tree <trees>/b10x/runbook/line2, branch impl/loom-line-2)
verdict: red
cases: executed 620→627, red 4 (new cases); the existing replay lane, run as the lane command, ran 9 and was red on 5
origin: introduced 4 / pre-existing 0 / undecided 1
wrote-outside-worktree: none beyond <scratch>/ (see 6)
needs-coordinator: yes (the replay lane is red at the head; the fix is the implementor's, see finding 1)
```

## 1. `git --no-pager diff --stat` (every path is a test file)

```
 runbook/src/loom_run/bindings_copy/tests.rs |  54 ++++++++
 runbook/tests/loom_run/adversary_replay.rs  | 192 ++++++++++++++++++++++++++++
 runbook/tests/loom_run/mod.rs               |   1 +
 3 files changed, 247 insertions(+)
```

No implementation file changed. The patch is `<scratch>/cases.patch`. After `git apply -R` and `git reset -q`, `git status --porcelain` printed nothing, and `git apply --check` accepts the patch again. I used no mutants. `git diff --stat -- '*/src/*' ui/server.mjs ui/web` printed nothing.

## 2. Cases and their red output

I wrote all cases first and ran each one alone before any wider run. Timing is from the combined run:

| Case | Asserts | Now | Time |
|---|---|---|---|
| `adversary_replay::a_replay_plan_in_the_work_tree_is_not_the_plan_of_the_run` | the adapter takes the run's `replay/plan.json`, not a nearer one in `work/` | red | 0.16 s |
| `adversary_replay::a_settings_file_with_a_replay_key_and_no_recorded_option_is_refused` | settings key `replay` without `--recorded` is exit 2 | red | 6.0 s (9.05 s alone; over 5 s) |
| `adversary_replay::the_cycles_suite_doc_quotes_no_ignore_attribute_that_no_test_holds` | every `#[ignore = "…"]` the doc quotes exists in a test source | red | 0.29 s |
| `adversary_replay::the_settings_page_does_not_say_the_runner_has_no_recorded_mode` | `docs/loom-run-settings.md` does not say `flags.recorded` is refused | red | 0.03 s |
| `adversary_replay::a_live_cap_that_cannot_be_read_or_is_spent_starts_no_live_call` | cap absent, text, negative, fraction, null, 0 or spent starts no live call; cap 1 lets one through | green | 0.43 s |
| `bindings_copy::tests::an_effort_and_a_replay_plan_land_on_the_same_nodes_and_the_effort_changes_nothing_else_in_argv` | seam: both settings reach the same model nodes; removing the `--effort` pair leaves argv equal | green | 0.54 s |
| `bindings_copy::tests::the_levels_the_copy_accepts_are_the_levels_node_classes_json_lists` | `EFFORTS` equals the levels in `node-classes.json` | green | 0.07 s |

Red output, each case alone, verbatim:

```
the live adapter of a plan found in the work tree was started: {"adapter":"live",...,"mode":"live","name":"change.fix",...,"paid":true,...}
docs/cycles-suite.md quotes attributes no test has: ["#[ignore = \"waits for --dry\"]"]
docs/loom-run-settings.md: the runner has a recorded mode (`runbook run --recorded`)
settings.replay without --recorded must be a refusal: runbook run: ended: delivered   (left: Some(0), right: Some(2))
```

## 3. The suite runs, after the cases existed

`SLOT_TIMEOUT=25m cargo nextest run --locked --color never --no-fail-fast -E 'test(adversary_replay) | test(an_effort_and_a_replay_plan) | test(the_levels_the_copy_accepts)'` ended with exit 100:

```
Summary [6.040s] 7 tests run: 3 passed, 4 failed, 620 skipped
```

Counts: 620 other tests exist, so 620→627.

The existing replay lane command ran without `slot`. It is `cargo nextest run --locked --profile replay --run-ignored only -E '<the lane filter>'`, with the log in `<scratch>/lane.log`.

```
SETUP PASS [141.145s] replay-runs
PASS replay_live_dry…  PASS replay_reject_nearest…  PASS replay_check_fail…  PASS replay_timeout…
FAIL replay_unchanged_closes_operating_on_recorded_replies_only   exit 11
FAIL rule_approvals_default_is_off…  FAIL review_mode_default_…  FAIL implement_coverage_default_is_off   exit 11
FAIL replay_reject_stop_ends_recording_diverged…   reason names change.review, not change.implement@1.2
runbook run: stopped: recording diverged: model call at IDEA-1.billing.invoices/change.review@1.1: in/tests.txt changed
Summary [141.216s] 9 tests run: 4 passed, 5 failed
```

The probes in `<scratch>/probe-control.log` agree. Two runs with `{}` settings and two with all four efforts set to `high` (the recorded level) all ended exit 11 at that same call.

## 4. Findings

**F1: the replay slow lane is red at the head (blocker, introduced, CONFIRMED).**
- Measured: the lane output above, and `<scratch>/verify-copy`. I took the run directory of the unchanged replay and rewrote one string in the call's `in/tests.txt`, `billing-reference 0.97.4` to `0.97.3`. `sdc-replay index` then gives `77990b93…`, which is the hash in `recording.json:407`. Without the rewrite it gives `39a9f07e…`.
- Cause: `tests.txt` carries the pinned ESS version, and `normalise` (`driver/src/bin/sdc-replay.rs:230`) does not mask it. Unit 4 packed the recording at ESS 0.97.3 (`27c9007` has pin 0.97.3). Pin 0.97.4 (`f663dfd`) came in with the merge of `main` into the line (`31f5168`).
- Reaches it: `task test:replay`, the only lane that reads a run. It is not part of `task check`, so the merge went through green.
- Fix: mask the ESS version in `normalise`, or re-pack the recording at 0.97.4. Add a default-step check that ties the recording to `driver/ess-pin.json`.
- The setup step took 141 s under load 45 to 60 (60 s per test, 300 s for setup).

**F2: the adapter takes the nearest `replay/plan.json` above the call (warning, introduced, CONFIRMED).**
- Measured: `driver/src/bin/sdc-replay.rs:1042` (`find_plan`), used at `:1147`. The red case puts a decoy `work/replay/plan.json` beside the run's plan. The decoy's live adapter ran and the run's `stop` policy was ignored.
- Reaches it: a project with its own `replay/plan.json` in the work tree, or a live model that writes one under `--on-divergence live`. Execution stays inside the confinement.
- Fix: take the plan of the run directory only, or fail when two plans are found.

**F3: settings key `replay` is accepted without `--recorded` (warning, introduced, CONFIRMED).**
- Measured: `runbook/src/loom_run/replay.rs:122` returns before it checks the key, and `bindings_copy.rs:175` reads it.
- The red case runs the engine fixture with `settings.replay.adapter = /bin/true` and no `--recorded`. It ended `delivered`, exit 0.
- Effect: any program path becomes `--adapter` of every model node. `settings.adapter` refuses that (`ADAPTERS`, `bindings_copy.rs:24`).
- Reaches it: any settings file. The runner's own settings key is writable by the caller.
- Fix: refuse the key without `--recorded` (exit 2, before the run directory), as `preset` does with it.

**F4: stale rows in `docs/cycles-suite.md` (warning, introduced, CONFIRMED).**
- Row `bugfix-from-ticket` at `docs/cycles-suite.md:377` still says the constraints "wait for `--dry`" and quotes `#[ignore = "waits for --dry"]`. Unit 6 changed that attribute.
- The `efforts` row at `:275` names a "slow lane" that no task runs.

**F5: unit 6 left the dry bug-fix test ignored and in no lane (warning, introduced, CONFIRMED).**
- The brief said to remove the `#[ignore]`. The test now carries `#[ignore = "slow lane…"]` and no Taskfile task runs it (`runbook/tests/loom_run/bugfix.rs:447`).
- I ran it with `--run-ignored only`. It passes in 30.4 s at load 45, so the prompt path works and the claim is true.
- That time is over the 5 s mark, so it is named here. The `efforts` row leans on it.

**F6: stale prose after the integration (note, introduced, CONFIRMED).**
- `docs/loom-run-settings.md:91-93` says `flags.recorded` stays refused and "the runner has no recorded mode". Unit 4 changed `sdc-experiment` to pass it on as `--recorded`. The red case is the doc test above.
- `driver/src/bin/sdc-experiment.rs:455` still gives "no argument of llm-node.sh carries an effort" as the reason for refusing `efforts.*`. That is false after unit 6. It is deliberate that `sdc-experiment` does not pass efforts yet, but the reason is stale.

**F7: `request.effort` is compared in replay (note, undecided).**
- A caller's `efforts` setting that differs from the recorded effort diverges at the first call. Probe in `<scratch>/probe-efforts.log`: `request.effort "high" -> "low"`, exit 11.
- Under `--on-divergence nearest` the same run ended `operating`, exit 0.
- This matches the way a changed model is treated, and the old driver compared the effort too. I could not run it against the old driver, so it is undecided.

## 5. What I attacked and could not break

- Live cap fails closed for six unreadable shapes and a spent cap. A cap of 1 lets exactly one call through (green case).
- Seam order: the replay preset runs, then `efforts`, then `replay.apply`. `--effort` and `--adapter` land on the same nodes and the effort changes nothing else in argv. Nodes the source never sent to a model get the stub adapter and keep their effort.
- An invalid effort or an unknown role is exit 2 before the run directory, with the setting's name. This held with a recording and without. A third invalid case was exit 2 in 0 s (probe `effort-bad`).
- A level listed in `node-classes.json` is accepted by the copy. The two lists agree today, though nothing pins them to each other.
- Dry makes no model call. The effort goes into `request.json` and the dry command. It is not an extra adapter argv entry.
- Divergence reasons hold keys and field names, not model or decider text. Paths in a refusal are the caller's own.
- The settings test `the_settings_file_must_agree_with_the_way_the_options_name` is about the `loom.*` keys, not `replay`. A refusal run takes 6 to 30 ms, plus two 66 MB hashes of about 0.2 s each. It runs no Loom step and no model call. It took 7.5 s at load 45, and I could not explain that part. It does no expensive step per case.
- Not examined: the live adapter's own stderr, written to `live-stderr.txt` in the call directory, which stays in the run directory.

## 6. Paths written outside the worktree

All under `<scratch>/`: `cases.patch`, `case1.log`, `case2.log`, `case4.log`, `run3.log`, `lane.log`, `probe-efforts/`, `probe-control/`, `verify-copy/`, `time-refusal/`, and the `*.sh` scripts and `probe-*.log` files.

Test runs also wrote under `<trees>/b10x/runbook/line2/runbook/target/tmp/adversary-replay-*`, and the lane wrote its six re-drives under `~/.local/state/runbook/replay-tests/`.

```findings
[
  {"file": "driver/src/bin/sdc-replay.rs", "line": 230, "category": "contract-drift", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "normalise does not mask the ESS version that in/tests.txt names, so after main moved the pin to 0.97.4 the committed recording (packed at 0.97.3) diverges at invoices/change.review and 5 of 9 replay lane tests are red."},
  {"file": "driver/src/bin/sdc-replay.rs", "line": 1042, "category": "boundary", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "find_plan takes the nearest replay/plan.json above the call, so a decoy in the work tree replaces the run's plan, policy, live adapter and cap (failing case a_replay_plan_in_the_work_tree_is_not_the_plan_of_the_run)."},
  {"file": "runbook/src/loom_run/replay.rs", "line": 122, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "settings.replay without --recorded is accepted and puts any program path on every model node as --adapter, which settings.adapter refuses (failing case a_settings_file_with_a_replay_key_and_no_recorded_option_is_refused)."},
  {"file": "docs/cycles-suite.md", "line": 377, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the bugfix-from-ticket row still says the fix prompt waits for --dry and quotes an ignore attribute the test no longer has, and the efforts row at :275 names a slow lane no task runs."},
  {"file": "runbook/tests/loom_run/bugfix.rs", "line": 447, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the brief ordered the ignore removed; the dry bug-fix test stays ignored with no lane that runs it, though it passes in 30.4 s at load 45."},
  {"file": "docs/loom-run-settings.md", "line": 91, "category": "contract-drift", "severity": "note", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the page says flags.recorded stays refused and the runner has no recorded mode, but unit 4 passes it on as --recorded; sdc-experiment.rs:455 also still gives a stale reason for refusing efforts."},
  {"file": "driver/src/bin/sdc-replay.rs", "line": 1089, "category": "judgement", "severity": "note", "verdict": "CONFIRMED", "origin": "undecided", "message": "request.effort is part of the compared request, so a caller's efforts setting that differs from the recorded effort diverges at the first call (exit 11 under stop, ended under nearest); whether the old driver compared it the same way I could not run."}
]
```
