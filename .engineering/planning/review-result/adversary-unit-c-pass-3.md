---
format: aep.planning-md/3
id: review-result:adversary-unit-c-pass-3
kind: review-result
status: active
title: 'Narrow check of fix round 2 of unit C (documents) at 4eb4144: red, 1 finding that unit B settles'
summary: 'A third, narrow check beyond the two attacks, read-only: N1, N2 and N3 of pass 2 are fixed; one paragraph of docs/reject-routes.md describes unit B at 91f7c72 and is true only once unit B is merged.'
tags:
- wave-2026-11-22-w1
relations:
- reviews: story:driver-runtime-removed
revision: 1
---
unit: C (documents) at 4eb4144, narrow check of fix round 2
verdict: red
cases: executed 6→6 (no new case; cases 03 and 06 rerun here, case 06 against two bases), red 0
origin: introduced 1 / pre-existing 0 / undecided 0
wrote-outside-worktree: 6 paths
needs-coordinator: (1) The one finding left is a merge-order dependency: the new paragraph of `docs/reject-routes.md` is true at unit B's `91f7c72` and false at `4eb4144`, so unit C is correct only once B is merged with or before it. (2) An in-memory merge of `4eb4144` with `91f7c72` conflicts in one hunk, in `CHANGELOG.md`; B does not change `docs/reject-routes.md`.

N1, N2 and N3 are fixed. The verdict is red only for the dependency on unit B; nothing is left for the implementor to change in this unit alone.

## 1. `git --no-pager diff --stat`

Empty. `git status --porcelain` is also empty (0 lines), before and after, at head `4eb4144` in `<trees>/b10x/runbook/line3-c`. I edited nothing in the tree and wrote nothing to the store.

## 2. Cases

No case was added. Last lines of the reruns:

- `case-03-shifted-citations.sh <tree> 4677c2b`, exit 0:
  ```
  SKIP docs/conductor-comparison.md has no citation docs/kpis.md:255
  SKIP docs/conductor-comparison.md has no citation docs/layers.md:155
  SKIP docs/takt-comparison.md has no citation docs/layers.md:164
  ```
- `pass-2/case-06-moved-citations.sh <tree> 4677c2b`, exit 0:
  ```
  same: 16  moved: 36  rewritten: 44  text changed or repeated: 7  blank: 11
  fails: 0
  ```
- `pass-2/case-06-moved-citations.sh <tree> 8e8e143`, exit 0:
  ```
  same: 94  moved: 0  rewritten: 0  text changed or repeated: 1  blank: 16
  fails: 0
  ```

## 3. Suite run

None. No cargo, no build; every check is a grep, `jq`, `git show` or `git diff`.

## 4. Findings at head `4eb4144`

### The three pass-2 findings

| # | pass-2 `file:line` | state | where now |
|---|---|---|---|
| N1 | `website/docs/context.mdx:151` | fixed | `:151` says the YAML binding holds no `llm` object and names the `argv` options; `:155` says `--instructions <json>` in the `argv` |
| N2 | `website/docs/context.mdx:163` | fixed | `:163` says the effort is not set per node class and a record that sets it is refused |
| N3 | `docs/requester.md:26` | fixed | `:26` cites `autonomy-levels.md:236-237`; all 24 citations of the pass-2 list open on their text (case 06, `fails: 0`) |

The citations the two new "Now" texts carry, opened at this head (all three links go to `blob/main`):
- `bindings/sdc-direction.yaml:16-23`: line 16 is the `argv` with `sdc-node`, `--model`, `--max-turns`, `--max-budget-usd` and `--instructions`; 17 is `budget: {max_runs: 3}`; 18 is `timeout_seconds: 900`; 19 to 21 are `retry_when`. It matches the text.
- `bindings/sdc-direction.yaml:16`: `--instructions '{"task":"instructions/context.default.md"}'`. It matches.
- `docs/loom-run-settings.md:90-91`: "Efforts (`efforts.<role>.effort`) … stay refused in a record of a `loom` variant, with the key named: `sdc-node` has no option for an effort under Loom". It matches.

### The diff `8e8e143..4eb4144`

| # | file:line | verdict | origin | what reaches it |
|---|---|---|---|---|
| R1 | `docs/reject-routes.md:149` | INFEASIBLE | introduced | The paragraph names a `runtime` block, `provided_by` and `capability` lines. At `4eb4144`, `driver/examples/sdc/rule-ends.json` has the block `driver` and no `provided_by`, and `driver/src/bin/sdc-check-routes.rs:86` reads `ends["driver"]` and prints no such line. It cannot be fixed in this unit: it becomes true when `91f7c72` is merged. |

Against unit B's `91f7c72` the paragraph is true:
- `rule-ends.json` has a `runtime` block with `capped_effects`, `cap_per_kind`, `backstop`, `rejection_is_arrival`, `own_failure_fed_back` and `rejection_logs_fed_back`.
- `provided_by` holds one entry per member, each `loom: …` or `pending: docs/loom-bindings.md:<line>`.
- `sdc-check-routes.rs:308-319` prints one `capability <member>: <who>` line per member; its comment says the lines are not findings and are not in the page's block.
- `rework_limit` is 2 and `approvals.*.reject` exists (`sdc.release/release.publish`).

The rest of the diff:
- The "Check output" block of `docs/reject-routes.md` is byte-equal to `8e8e143` (same sha256 from `## Check output` to the end of the file).
- No changed line names a deleted file as usable: `run-trial.sh` stands as "the deleted `run-trial.sh` read", and `llm-node.sh` on the context page stays in the past-tense "Where" half.
- Commit `4eb4144` changes 25 lines in 9 files, and every one differs from its old line in digits only.

### The three commits

`37a2058`, `2dbcabf` and `4eb4144` have author and committer `b10x-bot[bot]` with the bot's noreply address. Their 34 added lines hold no person name, no email address, no home path and no `file://`.

## 5. Attacked, not broken

- N1 and N2 texts against the lines they cite: no mismatch.
- Case 06 against `8e8e143`: the three new commits shifted no citation (94 same, 0 moved, 0 stale).
- The citation commit: no word changed besides line numbers.
- The gate block of `docs/reject-routes.md`: unchanged.

## 6. Paths written outside the worktree

All under `<scratch>/removal/line3-c/adversary/pass-3/`:
- `case-06-base.run`
- `case-06-8e8e143.run`
- `rule-ends-91f7c72.json`
- `sdc-check-routes-91f7c72.rs`
- `merge-probe-b.txt`
- `added-round2.txt`

The `worktree` lease of session `adversary-line3-c-p3` is released (exit 0).

## 7. Findings block

```findings
[
  {"file": "docs/reject-routes.md", "line": 149, "category": "contract-drift", "severity": "warning", "verdict": "INFEASIBLE", "origin": "introduced", "message": "the new paragraph describes the runtime block, provided_by and capability lines of unit B's 91f7c72, and at 4eb4144 the rule-ends file has the driver block and the check prints no capability line, so it is true only once unit B is merged"}
]
```
