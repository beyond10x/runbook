---
format: aep.planning-md/3
id: review-result:adversary-line2-fix-rounds-1-2-pass-2
kind: review-result
status: active
title: 'Adversary pass 2 on line 2 fix rounds 1 and 2 (units 3, 4, 6, 7) at c413209: red, 6 findings, all notes'
summary: 'Second attack on fix/line2-review-2: F1 to F8 and R1, R2 of pass 1 fixed; 4 introduced notes (the ESS label mask covers every input; a recording at another label diverges without a named cause; the dry bug-fix test is documented at the 60 s kill; the already-recorded guard has no scenario), 2 pre-existing notes (decider prompt files read per decision; the plan file in the run directory is trusted).'
tags:
- wave-2026-11-22-w1
relations:
- reviews: story:driver-runtime-removed
revision: 1
---
```
unit: line 2 fix rounds 1 and 2 (units 3, 4, 6, 7) at c413209, pass 2
verdict: red
cases: executed 634→647 in the runbook binary (13 added; the driver binary is unchanged), red 2
origin: introduced 4 / pre-existing 2 / undecided 0
wrote-outside-worktree: <scratch>/removal/line2/adversary-2/pass-2/ (cases.patch, run1.log, run2.log, suite.log, ess.log, mut-a5.log, bugfix-slow.log); the nextest tmp dirs under runbook/target/tmp (ignored by git)
needs-coordinator: no
```

The block has 6 entries. Four are introduced: F1 scope, F1 residual, the F5 doc range and R3. Two are pre-existing: the decider prompt files and the plan file. Red 2 is the two F1 cases in section 3.

## 1. Per-finding status at c413209

| Finding | Fixed? | Now at |
|---|---|---|
| F1 ESS version in `in/tests.txt` | Yes for the committed recording. The mask is too wide and misses other labels (section 3). | `driver/src/bin/sdc-replay.rs:232` (mask) and `:256` (regex) |
| F2 `find_plan` | Yes. The adapter reads only `SDC_REPLAY`. | `sdc-replay.rs:1058`; `driver/src/bin/sdc-node/setup.rs:113`; `runbook/src/loom_run/replay.rs:266` |
| F3 `replay` key without `--recorded` | Yes. Any value is refused. | `replay.rs:120-125`, `:133` |
| F4 stale rows in the cycles-suite doc | Yes | `docs/cycles-suite.md:275`, `:377` |
| F5 dry bug-fix test in no lane | Yes. See the 60 s note below. | `runbook/tests/loom_run/bugfix.rs:413`, `:447`; `Taskfile.yml:89` |
| F6 settings page said "no recorded mode" | Yes | `docs/loom-run-settings.md:93` |
| F7 `request.effort` compared | Decided and documented | `docs/driver.md:1020`, `docs/loom-run.md:378`; compare at `sdc-replay.rs:1101-1105` |
| R1 Decide and Recommend guards | Yes | scenarios in `ess/ess-inputs.yaml:16-18`; mutants in `driver/tests/decision_contract_mutants.rs:87`, `:95` |
| R2 A5 ignored a missing table | Yes | `runbook/src/loom_run/decider.rs:290-293` |
| R3 `already-recorded` unobservable | Not fixed, and `ess/domains/decision.yaml` is untouched. It stays INFEASIBLE, a note, origin introduced. | `ess/domains/decision.yaml:746` |

## 2. Diff stat (cases only)

```
 runbook/tests/loom_run/adversary_pass2.rs | 172 +++
 runbook/tests/loom_run/decisions.rs       |  71 +++
 runbook/tests/loom_run/mod.rs             |   1 +
```

No implementation file is in the diff. The cases are in `<scratch>/removal/line2/adversary-2/pass-2/cases.patch`. After `git apply -R` and `git reset -q`, `git status --porcelain` is empty, and `git apply --check` accepts the patch.

## 3. Cases with red output (run alone first)

Both red cases are F1.

- `adversary_pass2::a_version_in_another_input_file_that_looks_like_the_label_is_still_an_input` is RED (0.05 s).
  - The recorded `notes.md` is `shop v2 against upstream-api 0.97.3 — …`. The new one is the same line with `9.9.9`.
  - `left: "recorded"`, `right: "diverged"`, `difference: []`. A changed input replays silently, so the mask reaches more than the ESS log header.
- `adversary_pass2::a_recording_packed_at_another_label_than_the_mask_still_replays` is RED (0.12 s).
  - The recorded `tests.txt` digest is raw text at `0.97.1`. The new one is at `0.97.3`.
  - Result: `difference: "in/tests.txt changed"`, `mode: diverged`.
  - A recording packed before the fix at any other label diverges. The one committed recording is at `0.97.3`, so it is unaffected.

## 4. Suite run

Command: `SLOT_TIMEOUT=20m cargo nextest run --locked --no-fail-fast` with `-E` on `loom_run::adversary_pass2`, `loom_run::decisions`, `loom_run::replay_boundaries`, `loom_run::replay::`, `decider::tests` and `replay::tests`.

- Result: 111 tests, 110 passed, 1 failed (the label case above), 10.2 s.
- The other red case (`a_version_in_another_input_file…`) ran in its own single-test run. It was fixed after the suite run: its recorded text first used a raw version that no pack would produce.
- Slowest test: `a_table_that_is_unreadable_at_the_start_stops_for_a_person_at_every_level`, 10 to 13 s. It runs 5 levels with 3 broken forms each, 15 runs.

Attacks on the fixes, all with the cases above:

- **F8, deleting the table.** After the start, deleting the table and the classes file (A3), and deleting the table at A5, change nothing. The decider still takes what the first read gave it. The A5 close is still `rule:autopilot` with 0 model calls.
- **F8, unreadable table at the start.** Missing, empty and non-JSON tables each stop for a person at A1 to A5. All 15 combinations give exit 8, no decider start and no approval.
- **F8, decider instructions.** Deleting `<run>/instructions` after the start still gave exit 10 with the decider asked once. `instructions_root` walks up from the run copy to any ancestor directory that has `instructions/` (`decider.rs:619-629`). It reads per decision and is not pinned. The code is the same on `main`, so this is pre-existing (finding 4).
- **Round 1 mutants.** The suite catches both survivors from pass 1: the Decide guard for person only (1.9 s) and the Recommend guard for strong only (1.8 s). Both are in the driver step that `task check` runs through `test:driver`.
  - I put the A5 mutant back: the A5 return moved before the table load, for a non-close. Three tests failed: `a_missing_table_stops_for_a_person_at_a5_too…`, the unit test `a_table_that_is_missing_or_unusable_is_a_person_s…`, and my per-level case. I removed the mutant afterwards.
  - `task check:ess-generated` exits 0 at head. It prints `ess conformance suite current (sources 08aa902b…)`.
- **F3.**
  - `replay` as null, an empty object, an empty array, a string or false, without `--recorded`, is refused (exit 2, no run directory).
  - `--recorded` given twice is refused by clap (exit 64).
  - A `replay` key nested under `loom` is neither read nor run.
- **F2.**
  - A relative `SDC_REPLAY` is read against the adapter's own directory.
  - With no `SDC_REPLAY` the adapter fails closed, as before.
  - The plan file in the run directory is still trusted for `live.bin` and the cap, which the old code did too (finding 5).
- **F5.**
  - The default step runs 0 tests for `bugfix_from_ticket_` (647 skipped).
  - `slot --timeout 30m task test:bugfix-slow` passes: 26.1 s and 27.8 s, 27.8 s for the step.
  - The runner kills a test at 60 s: `.config/nextest.toml` has `slow-timeout = {period = "30s", terminate-after = 2}`.
  - The doc quotes 49 to 62 s for the dry test at load 130. That is at or past the kill (finding 3).

## 5. Judgement findings

1. **F1 scope.** `normalise` applies the label mask to every normalised text: all input files, the prompt and the request. A column-0 line shaped `<x> v<n> against <y> <A.B.C> — …` in any other input collapses to the same digest. The mask belongs to `in/tests.txt` only, or to its first line.
2. **F1 residual.** The mask is fixed at `0.97.3`. A recording packed by the code before the fix at another label diverges with the message "in/tests.txt changed", which does not name the cause. Pass 1 found that no such recording is committed, so this is a note.
3. **F5.** The dry test's doc range (49 to 62 s at load 130) reaches the 60 s kill, so the lane is flaky under load. It is 27.8 s now. The doc is at `docs/cycles-suite.md:377`.
4. **F8.** The decider prompt files (`decider.rs:399`, `:467`; `ask.rs:85`) are read from the run copy per decision, and `instructions_root` walks above the run directory. The table pin does not cover them.
5. **Plan file trust.** `<run>/replay/plan.json` is read on every call and holds `live.bin`, the cap and the policy. A program bound in the run that can write it changes the policy. I read the code and built no end-to-end case, so this is not tested end to end. The code is the same on `main` (`199f311ffb:driver/src/bin/sdc-replay.rs:1076`). The table was pinned for this reason, and the plan was not.

## 6. What could not be broken

- A relative, linked or in-`work/` plan path cannot be set by anyone but the runner, because `SDC_REPLAY` comes only from `--replay-plan` and `decider.replay_plan`.
- A caller's settings cannot set the `replay` key in any form.
- A recording at `0.97.3` replays under a pin that prints `0.98.0`.
- A changed test name under a new label diverges.
- The two round-1 guard mutants are killed.

## 7. Paths written outside the worktree

`<scratch>/removal/line2/adversary-2/pass-2/` (listed in the header). I made no commit and no store write, and the tree is clean.

```findings
[
  {"file": "driver/src/bin/sdc-replay.rs", "line": 256, "category": "contract-drift", "severity": "note", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the ESS label mask runs in normalise() over every input file, the prompt and the request, not only in/tests.txt; a column-0 line shaped \"x v2 against y 0.97.3 - ...\" in notes.md and the same line with 9.9.9 index the same, and the adapter answers mode recorded with an empty difference (runbook/tests/loom_run/adversary_pass2.rs a_version_in_another_input_file_that_looks_like_the_label_is_still_an_input is red); restrict the mask to in/tests.txt or to its first line."},
  {"file": "driver/src/bin/sdc-replay.rs", "line": 232, "category": "acceptance", "severity": "note", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "ESS_LABEL_MASK is fixed at 0.97.3, so a recording packed before the fix at another label (0.97.1 in my case) holds a raw digest and diverges at in/tests.txt with \"in/tests.txt changed\", naming no cause (a_recording_packed_at_another_label_than_the_mask_still_replays is red); the one committed recording is at 0.97.3 and replays, so the fix is to name the cause in the divergence message or to say in docs/loom-run.md that such a recording must be packed again."},
  {"file": "docs/cycles-suite.md", "line": 377, "category": "acceptance", "severity": "note", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the dry bug-fix test is documented at 49 to 62 s at load 130 while the default nextest profile kills a test at 60 s (slow-timeout 30 s, terminate-after 2), so test:bugfix-slow can fail under load; measured 26.1 s and 27.8 s here; the doc should state the load at which it is killed or the test should be made shorter."},
  {"file": "runbook/src/loom_run/decider.rs", "line": 619, "category": "acceptance", "severity": "note", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "the decider's prompt files are read from the run copy on every decision (decider.rs:399, :467, ask.rs:85) and instructions_root walks up past the run directory to any ancestor that has instructions/, so the pin of the table covers neither; deleting <run>/instructions after the first action still gave exit 10 with the decider asked, shown by decisions.rs a_program_that_deletes_the_decider_instructions_does_not_change_whether_the_decider_is_asked; same code on main 199f311ffb."},
  {"file": "driver/src/bin/sdc-replay.rs", "line": 1076, "category": "acceptance", "severity": "note", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "<run>/replay/plan.json is read on every adapter call and holds live.bin, the cap and the policy, so a program bound in the run that can write the run directory could set policy live and name any program as the live adapter; the table was pinned in memory for this reason and the plan was not; same read of live.bin on main 199f311ffb; found by reading the code, not tested end to end because it needs the replay lane."},
  {"file": "ess/domains/decision.yaml", "line": 746, "category": "contract-drift", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "not fixed, and decision.yaml is untouched: already-recorded still has no scenario that tells defined(produced_record) from the old coalesce predicate, and empty-record-id still precedes no-request, so an empty id on an unknown request answers EmptyRecordId; the new scenario decision-empty-record-id-on-an-unknown-request.yaml now pins that order, so the open part is only the unobservable guard."}
]
```
