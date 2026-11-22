---
format: aep.planning-md/3
id: review-result:adversary-unit-b-pass-2
kind: review-result
status: active
title: 'Adversary pass 2 on unit B (producer ids, old bindings) at 802e9f5: red, 5 findings'
summary: 'Second attack on impl/line3-producer-ids: F2, F4 and F5 of pass 1 fixed, F3 in part; 3 introduced (the refusal does not check values; two notes), 2 pre-existing warnings (the routes check reads the JSON copies, which differ from the YAML in retry_when; five findings name stops of the deleted runtime).'
tags:
- wave-2026-11-22-w1
relations:
- reviews: story:driver-runtime-removed
revision: 1
---
**Pass 2 report: unit B, `802e9f5`.** Four of the five fixes hold, and five findings remain.

```
unit: B (producer ids, old bindings) at 802e9f5, pass 2
verdict: red
cases: executed 401→408, red 5
origin: introduced 3 / pre-existing 2 / undecided 0
wrote-outside-worktree: <scratch>/removal/line3-b/adversary/pass-2/ (logs, pidmut.sh, pid/ with a compiled copy of the producer_ids test, scan/ mutant copies, cases.patch)
needs-coordinator: no
```

`before` is the test count of the `main` binary minus mine, from nextest's skip count. I ran no whole-binary suite.

### 1. Diff stat (the cases)

Only test files changed. The patch is `<scratch>/removal/line3-b/adversary/pass-2/cases.patch`.

```
 driver/tests/adv_b2.rs | 100 +++++++++++++++++++++++++++++++++++++++++++++++++
 driver/tests/main.rs   |   1 +
 2 files changed, 101 insertions(+)
```

After `git apply -R` and `git reset -q`, `git status --porcelain` is empty and `git apply --check` succeeds.

### 2. Cases added (`driver/tests/adv_b2.rs`)

I ran them alone first. All finished within 0.17 s.

| Case | State |
|---|---|
| `a_provided_by_value_with_the_prefix_and_nothing_after_it_is_refused` | RED: `"loom: " accepted:` (exit is not 2) |
| `a_loom_value_with_no_file_and_line_is_refused` | RED: `loom: nothing in particular` accepted |
| `a_member_of_the_wrong_type_is_refused` | RED: `backstop="8" accepted (exit 0)` |
| `two_refusals_at_once_are_both_named` | RED: `only one is named` (stderr has `backstop` only) |
| `a_member_that_says_it_is_present_while_provided_by_says_absent_is_refused` | RED: `cap_per_kind true next to 'absent:' accepted` |
| `a_refusal_prints_no_route_and_names_the_file` | GREEN: stdout is empty and stderr names `rule-ends.json` |
| `the_unedited_block_is_read_and_exits_0` | GREEN |

Run result: 7 run, 2 passed, 5 failed, 401 skipped.

Existing tests: `runtime_block` and `producer_ids` give 10 of 10 passing in 0.56 s.

### 3. Pass-1 findings at `802e9f5`

- **F2 (refusal) is fixed.** `sdc-check-routes.rs:300-327` refuses with exit 2 and prints no route before it. It handles a missing `runtime` block, the old key `driver` (with the "was renamed" message), an unknown member, a missing `provided_by`, a stray `provided_by` entry, and a value without a known prefix. Gaps remain, see N1.
- **F3 (truth of the block) is partly fixed.**
  - Each cited line says what its member says; line-cite notes are under "Attacked and held" below.
  - `inert-cap … change.review` is rightly gone, and `change.review` has effect `read` with `max_runs: 3`.
  - The check prints 35 findings. No finding text names the driver, `run-trial` or `llm-node`.
  - What is not right: the check reads the JSON twins, which differ from the YAML Loom runs (N2), and five findings still describe driver-only stops (N3).
- **F4 is fixed.** `docs/driver.md:255` now says `loom-child-case`, and `children.rs:26` is exactly `CHILD_OUTCOME_PRODUCER`. My own `stub-cycle` store had `loom-child-case` on its `case_outcome` records and `runbook` on 7 `*_opened` records.
- **F5 is mostly fixed.** The mutants and the one miss:

| Mutant | Result |
|---|---|
| `$independence` key in one JSON | caught: `…/$independence: key sdc-drive` |
| `independence` key in one YAML | caught |
| `producer:` value in one YAML | caught |
| `producer` value split across two lines | caught (the old line scan alone missed this) |
| `producer` as an array | caught |
| generated site JSON | caught |
| prose naming the id after the word `producer` (JSON note, YAML description) | FAILS the test; this must not fail (N4) |

### 4. Findings

**N1: the refusal does not check values (`driver/src/bin/sdc-check-routes.rs:321`, introduced, warning).**
- What was measured: the five red cases above.
- What reaches it: any hand edit of `rule-ends.json`.
- Not enforced: a prefix with an empty remainder, no `<file:line>` in a `loom:` value (the comment at `:296-299` promises that form), a member of the wrong type, and a member contradicting its `absent:` entry.
- Only the first refusal is reported.
- `check-bindings.sh` runs the check as `|| fail=1`, so exit 2 and exit 1 look the same except for stderr.
- Fix: validate type and remainder, and collect all refusals.

**N2: the route check reads the JSON twin, not the YAML that Loom runs (`driver/examples/sdc/sdc-story.bindings.json:50`, pre-existing, warning).**
- `retry_when` differs between the two copies in 3 actions: `sdc-story` `change.implement` lacks `implementation_change`, `sdc-intake` `questions.answer` lacks `answer_applied`, `sdc-direction` `amend.upstream` lacks `context_defaults`.
- The finding `shared-cap sdc.story change.implement: 2 retry kinds` is 3 kinds in the YAML.
- It is identical at `985e8ee`, and `check-bindings.sh` does not compare `retry_when`.
- What reaches it: every run of the routes check.
- F3 now claims the findings state what holds on Loom, so the drift matters more.

**N3: five findings still describe driver-only stops (`driver/src/bin/sdc-check-routes.rs:218`, `:267`, pre-existing, warning).**
- `rework-limit … ends in the stop rework_spent` (four lines) and `no-reject-route amend.upstream … decision_not_takeable` name stops that nothing in `runbook/src` or `driver/src` writes.
- The change log itself says `rework_limit` is not a count on Loom.
- They are no longer actor names, but the live block still says what the deleted runtime did.

**N4: the line scan still false-positives (`driver/tests/producer_ids.rs:28`, introduced, note).**
- A prose string such as `"the producer sdc-drive was renamed"` fails the test, in JSON and in YAML. The structural check was meant to remove this.
- What reaches it: nothing found. No such string exists at the head.
- The evidence is a mutant of the compiled scratch copy, `<scratch>/…/pass-2/pidmut.sh`, not a case in the tree.

**N5: `backstop` is stated, never read (`driver/examples/sdc/rule-ends.json:6`, introduced, note).**
- No check reads `drv["backstop"]`. Its meaning changed from the old driver's per-arrival backstop of `max(8, max_runs)` (`docs/reject-routes.md:81`) to Loom's default `max_runs` of 8.
- Its cite `lib.rs:83` is the doc comment; the constant is at `:84`.
- `retry.rs:4` and `:8` are line-ends inside a sentence: `:8` covers the program's run and `:9` the person's reasons.

### 5. Attacked and held

- **Loom lines.** `program.rs:740` is `if runs >= binding.budget.max_runs`, and the default is 8 when no budget is named. So `capped_effects` `["external","read","write"]` is true, and the IR effects are exactly those three. `program.rs:956` counts the runs of an action in a case, with no per-kind count and no arrival reset. `retry.rs:4-9` feeds back only records whose `kind` and `result` `retry_when` lists, so `own_failure_fed_back: false` holds. The tail of a cited program run goes into the feedback, so `rejection_logs_fed_back: true` holds.
- **Sampled new findings.** Six checked against the YAML.
  - `own-failure bug.dissect`, `questions.answer` and `plan.decompose` are real: `sdc-node` write nodes with no evidence for a failed run.
  - `shared-cap spec.author` (6 kinds, max 4), `plan.decompose` (4 kinds, max 3) and `vision.write` (reworks after a rejected approval, max 3) are real.
- **Fix-round diff.** `git diff 91f7c72 802e9f5 --stat` is 8 files, all within the five findings plus the change-log bullet and `main.rs`. The commit is `b10x-bot[bot]|b10x-bot[bot]`. The added lines hold no name, email, `/home/` path, `file://` or `[patch]`. `sdc-protocol-site check` is "current".
- **Still true from pass 1.** The `check-bindings.sh` mutants and the 3.7 s run time, and the producers a run writes.

### 6. Paths written outside the worktree

All under `<scratch>/removal/line3-b/adversary/pass-2/`: `cases.patch`, `pidmut.sh`, `pidmut.log`, `run1.log`, and the directories `pid/` and `scan/`. Pass 1 left its own directories next to it.

### 7. Findings block

```findings
[
  {"file": "driver/src/bin/sdc-check-routes.rs", "line": 321, "category": "boundary", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "The refusal checks only the prefix of a provided_by value and the presence of members: `loom: ` with nothing after it, a loom value with no file:line, a member of the wrong type (backstop \"8\") and a member contradicting its absent entry are all accepted, and two refusals at once name only the first (five red cases in driver/tests/adv_b2.rs)."},
  {"file": "driver/examples/sdc/sdc-story.bindings.json", "line": 50, "category": "contract-drift", "severity": "warning", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "The route check reads the JSON twins, whose retry_when differs from the YAML Loom runs (change.implement lacks implementation_change, questions.answer lacks answer_applied, amend.upstream lacks context_defaults), so shared-cap and own-failure counts are computed on a copy that is not the one run, and check-bindings.sh does not compare retry_when."},
  {"file": "driver/src/bin/sdc-check-routes.rs", "line": 218, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "Five findings (four rework-limit lines naming the stop rework_spent and the no-reject-route amend.upstream line naming decision_not_takeable) describe stops of the deleted runtime that nothing in runbook/src or driver/src writes, while the block now claims to state what holds in a run on Loom."},
  {"file": "driver/tests/producer_ids.rs", "line": 28, "category": "mutant", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "The line scan is kept beside the structural scan, so prose such as 'the producer sdc-drive was renamed' in a JSON or YAML string fails the test although only a producer value or an independence key should; nothing in the tree has such a string today (shown on a compiled scratch copy)."},
  {"file": "driver/examples/sdc/rule-ends.json", "line": 6, "category": "judgement", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "backstop is stated but read by no check and now means Loom's default max_runs where reject-routes.md:81 means the old per-arrival backstop, and its cite lib.rs:83 is the doc comment (the constant is at :84) while retry.rs:4 and :8 cut a sentence."}
]
```
