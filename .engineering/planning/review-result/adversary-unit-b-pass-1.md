---
format: aep.planning-md/3
id: review-result:adversary-unit-b-pass-1
kind: review-result
status: active
title: 'Adversary pass 1 on unit B (producer ids, old bindings) at 91f7c72: red, 7 findings'
summary: 'First attack on impl/line3-producer-ids: 5 introduced (the routes check accepts the old key and a member without provided_by; provided_by cites lines that do not say it; one sentence of docs/driver.md no runner makes true), 2 pre-existing notes. The header writes the verdict as NEEDS-CHANGE; it is red.'
tags:
- wave-2026-11-22-w1
relations:
- reviews: story:driver-runtime-removed
revision: 1
---
```
unit: B (producer ids, old bindings) at 91f7c72, pass 1
verdict: NEEDS-CHANGE
cases: executed 393→399, red 5
origin: introduced 5 / pre-existing 2 / undecided 0
wrote-outside-worktree: <scratch>/ (adversary/ only: logs, mutant copies, a kept stub-cycle store, tmp/ and xdg/ for the script's TMPDIR and XDG_RUNTIME_DIR)
needs-coordinator: no
```

The `before` figure is the test count of the `main` binary from nextest's skip count in my first run, not a suite run. Findings 1 to 5 are `introduced`; findings 6 and 7 are `pre-existing`.

## 1. Diff stat

I touched only test files. Both are in the patch at `<scratch>/cases.patch`. I put the tree back (`git apply -R`, `git reset -q`).

```
 driver/tests/adv_b.rs | 108 ++++++++++++++++++++++++++++++++++++++++++++++++++
 driver/tests/main.rs  |   1 +
 2 files changed, 109 insertions(+)
```

After the revert, `git status --porcelain` is empty and `git apply --check` succeeds.

## 2. Cases added (`driver/tests/adv_b.rs`)

I ran them alone before any suite run. Each takes under 0.1 s.

| Case | Asserts | State |
|---|---|---|
| `only_the_lead_of_the_argv_starts_a_model_step` | `["tools/sdc-tool.sh","check","--out","bin/sdc-node"]` is no model step | RED: panic at `adv_b.rs:48` |
| `a_program_check_with_sdc_node_as_an_argument_is_still_a_program_check` | that argv passes `program_check` | RED: `left: Err("it is run by a model, not a program check")`, `right: Ok(())` |
| `the_old_key_driver_is_refused_not_read_as_an_empty_runtime` | a file with `driver` instead of `runtime` exits non-zero and the message names `runtime` | RED: exits 1, but only 15 or more `+ shared-cap … (found, not in …reject-routes.md)` lines and no word `runtime` |
| `a_member_without_provided_by_is_refused` | exit non-zero | RED: output `capability backstop: undeclared (no provided_by)`, exit 0 |
| `an_unknown_member_of_the_runtime_is_refused` | exit non-zero | RED: output `capability bogus_member: loom: nothing`, exit 0 |
| `the_live_tree_prints_six_capability_lines_and_exits_0` | `--doc docs/reject-routes.md` exits 0 with six `capability` lines | GREEN |

In my first run the cases omitted `--doc`. The binary then exits 1 on the 15 listed findings, so three of the red cases were red for the wrong reason. I added `--doc`, re-ran, and the table above is from that run.

## 3. Test runs

Filter `test(producer_ids) | test(adv_b) | test(sdc_node)` in the `main` binary, which finished in 6.8 s of nextest time:

- 167 run, 162 passed, 5 failed (my five red cases), 232 skipped.
- The ten slowest tests are `sdc_node` ones at 1.3 s or less.

`cargo nextest run --lib -E 'test(only_sdc_node)'` passes (0.008 s).

I did not run the whole binary, as ordered.

## 4. Findings

**F1: stale doc for the renamed key (`docs/reject-routes.md:139-143`, introduced, warning).**
- What was measured: the doc still says "`run-trial.sh` reads two of them" and "The `driver` entries state facts of the driver and of `llm-node.sh`".
- The key is now `runtime`. The doc's "Check output" block is compared by `check-bindings.sh`, but the prose around it is not.
- What reaches it: any reader of the doc.
- Docs are unit C's; I report it because B's rename made it false.

**F2: the old key `driver` and a missing `provided_by` are accepted (`driver/src/bin/sdc-check-routes.rs:83`, `:305-319`, introduced, warning).**
- `ends["runtime"]` is `Null` for a file with the old key, so no effect is capped and the check runs.
- A member without `provided_by` is printed as `undeclared (no provided_by)` with exit 0.
- A member nothing reads is printed as a capability with exit 0.
- What reaches it: a stale or hand-edited `rule-ends.json`.
- The old-key case is loud only by accident (it exits 1 on many findings). No message names `runtime`.
- The doc comment calls the lines facts, so exit 0 may be intended. Say so, or refuse.
- Fix: have `load` refuse a file with no `runtime` block and one with a member of `runtime` that has no `provided_by`.

**F3: the `provided_by` citations do not say what they cite (`driver/examples/sdc/rule-ends.json`, claim 4, introduced, warning).**
- `cap_per_kind` cites `docs/loom-bindings.md:103`, which is about `retry_when` and `LOOM_RETRY_FEEDBACK`, not per-kind caps.
- `backstop` cites `:106`, which is about `max_runs` counting since the last arrival, not a backstop.
- A grep of `loom-bindings.md`, `loom-run.md` and `PENDING.md` finds no line on per-kind caps or a backstop.
- `capped_effects` is credited to `loom: budget.max_runs (docs/loom-run.md:281)`. Loom caps every program action by `max_runs`: `crates/loom-bindings/src/program.rs:740` and `lib.rs:83` in the pinned Loom at `70b0e6e`. The driver capped only `write` effects. So `capped_effects: ["write"]` is a driver fact, not a Loom one.
- As a result, the `inert-cap sdc.story change.review` finding in the doc ("which the driver does not cap") is false for a run on Loom, which does cap that read node.
- Checked and true: `loom-run.md:281`, `:103` for the feedback members, `:106` for `rejection_is_arrival`. `max_runs` agrees between the YAML and its JSON twin on every action of all nine files.

**F4: a statement that no runner makes true (`docs/driver.md:255`, introduced, warning).**
- The line now says the opened-child `case_outcome` records carry `producer: runbook`.
- In my `stub-cycle` run the four `case_outcome` records carry `loom-child-case`. Nothing writes `runbook` there.
- Before B the sentence was true for the deleted driver.
- What reaches it: readers of `docs/driver.md`.

**F5: `producer_ids` is blind to `$independence` keys (`driver/tests/producer_ids.rs:28`, introduced, note).**
- The scan looks only at lines that contain `producer`.
- `"$independence": {"sdc-drive": [..]}` in a bindings JSON, where the producer id is a key, passes. I checked this with a mutant.
- No such key exists today: `git grep` at `985e8ee` finds none.
- The mutants of one YAML, one JSON and one generated file are all caught, with `file:line`.

**F6: `model_step_argv` accepts `sdc-node` as any member of the argv (`driver/src/reply/rules.rs:16-20`, pre-existing, note).**
- The doc comment says "starts". The code uses `.any`, which `985e8ee` also did.
- What reaches it: nothing found. All 14 agent actions carry an `llm` object, which independently makes `program_check` and `is_llm_node` true, so the argv test is a second line of defence.
- Within `model_step_argv`, all of these return false: `sdc-node.sh`, a path ending in `/llm-node.sh`, and an empty or missing argv. `{dir}/…/sdc-node` and an absolute path return true.
- A rule given a binding with `llm` absent and an unrecognised argv treats it as a program check. That widens only if a binding drops `llm`.

**F7: a `producer` id that names the deleted script (`bindings/*.yaml`, six lines of `producer: llm-node`, and the JSON twins, pre-existing, note).**
- It is a label naming `llm-node.sh`, which my grep pattern `llm-node\.sh` does not see.
- It is the same at `985e8ee`. It is not in the brief's list.

## 5. Attacked and held

- **Claim 1, readers.** Nothing in `runbook/src`, `driver/src`, `ui/*.mjs`, `ui/web`, the protocols or `ess/` compares a producer to `sdc-drive`. The only `sdc-drive` hits outside docs and history are `driver/tests/producer_ids.rs`, `eval/`, `website/docs/*`, `website/src/data/layers.ts` and the named-history text in `website/data/protocols/index.json`.
- **Claim 1, what a run writes.** In my own `stub-cycle` run (47.7 s, exit 0) the store has `runbook` on seven records, plus `runbook.build`, `.deploy`, `.merge` and `.monitor`. So the ids are produced, not only declared. `.inspect` and `.falsifier` exist only in `driver/examples/investigation/bindings.json`.
- **Claim 1, cycle coverage.** The three cycles cover the ids but assert none. `stub-cycle` and `scope-answer` check only exit and outcome. `review-reject-once` checks `llm-review`, which did not change. No cycle would go red if an id changed back; `producer_ids` is the only guard.
- **Claim 2, timing.** The real `scripts/check-bindings.sh` takes 3.7 s and exits 0.
- **Claim 2, mutants.** I ran them on a scratch copy of the script's own text, cut before the `sdc-check-routes` step. I did not mutate the tree.

| Mutant | Result |
|---|---|
| JSON argv lead changed (`sdc-nodx`) | caught: `argv starts [...], … starts [...]` |
| JSON argv lead set to `llm-node.sh` | caught |
| JSON `argv[1]` changed | caught |
| YAML `argv[0]` changed | caught |
| action in the JSON only | caught: `undeclared action` and `not in bindings/….yaml` |
| action in the YAML only | caught: `binds undeclared action` |
| `--instructions` value changed in either file | caught |
| `--instructions` removed from the YAML | caught |
| instruction path containing `..` | caught |
| JSON `argv` deleted | not caught; the check skips actions with a null `argv` on either side |
| YAML `argv[2:]` (`--model`, `--max-turns`) changed | not caught: only two argv members are compared; the unit says so |

- The unit's whitespace-only edit `\.md$&&` still works.
- **Claim 4, exit codes.** The exit code for a file with findings is unchanged: 1 with findings, 0 with `--doc` when the block matches. The live "Check output" block equals the live output: `routes: 15 findings, all listed in docs/reject-routes.md`. A scratch copy of `reject-routes.md` also gave 0.
- **Claim 5.** `sdc-protocol-site check` reports "current" at the head. I ran the `git grep` of claim 5 and sorted its hits:
  - `ui/test/recorded/trial-todo-1-slim/**`: recorded run, covers `sdc-drive` and `llm-node.sh`.
  - `website/data/protocols/index.json:689-691, 856`: named history inside prose, covers `run-trial.sh`, `decider.sh` and `sdc-drive`.
  - `runbook/src/loom_run/decider/tests.rs:715`: a doc comment naming `decider.sh`, covers `decider.sh`.
  - No hit is a live reference to a deleted thing.
- **Claim 6.** Every commit of `e9a5df5..91f7c72` is `b10x-bot[bot]|b10x-bot[bot]`. The added lines and commit messages have no `/home/`, name, email, `file://` or `[patch]`.

## 6. Paths written outside the worktree

All under `<scratch>/` (`removal/line3-b/adversary/`):
- Scripts and logs: `mutants.sh`, `mutants2.sh`, `mutants.log`, `run1.log`, `run2.log`, `run3.log`, `real-check.log`, `cyc-stub.log`, `cases.patch`.
- Scratch directories: `tree/`, `scan/`, `pid/` (a compiled copy of the `producer_ids` test), `cyc-stub/` (the kept stub-cycle store), `tmp/`, `xdg/`.

The `producer_ids` mutants used a compiled scratch copy of the test whose root comes from `SCAN_ROOT`.

## 7. Findings block

```findings
[
  {"file": "docs/reject-routes.md", "line": 139, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "The prose around the Check output block still says the `driver` entries state facts of the driver and of llm-node.sh and that run-trial.sh reads two of them, while the key is now `runtime` and both are deleted."},
  {"file": "driver/src/bin/sdc-check-routes.rs", "line": 305, "category": "boundary", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "The old key `driver`, a member of `runtime` without `provided_by` and an unknown member are all accepted: the first exits 1 with unrelated findings and never names `runtime`, the other two exit 0 with an `undeclared` or invented capability line (cases in driver/tests/adv_b.rs)."},
  {"file": "driver/examples/sdc/rule-ends.json", "line": 10, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "provided_by cites docs/loom-bindings.md:103 for cap_per_kind and :106 for backstop but neither line mentions them, and capped_effects [write] is credited to Loom whose max_runs caps every program action (loom-bindings program.rs:740), so the inert-cap finding on change.review is false for a run on Loom."},
  {"file": "docs/driver.md", "line": 255, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "The opened-child case_outcome record is said to carry producer runbook, but a stub-cycle run of runbook run writes loom-child-case on those records; nothing writes runbook there."},
  {"file": "driver/tests/producer_ids.rs", "line": 28, "category": "mutant", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "The scan only reads lines containing the word producer, so a producer id written as a key of $independence (sdc-drive) passes; the YAML, JSON and generated-file mutants are all caught."},
  {"file": "driver/src/reply/rules.rs", "line": 16, "category": "boundary", "severity": "note", "verdict": "INFEASIBLE", "origin": "pre-existing", "message": "model_step_argv matches sdc-node as any member of the argv although its comment says it starts the argv, so a program check with sdc-node as an argument is refused as a model step; all 14 agent bindings also carry llm, so nothing reaches it."},
  {"file": "bindings/sdc-intake.yaml", "category": "judgement", "severity": "note", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "Six producer ids are still `llm-node`, the name of the deleted script, in the YAML and the JSON twins; the same at 985e8ee, outside the brief's list."}
]
```
