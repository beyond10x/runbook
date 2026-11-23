# Trial 3: one idea, idea to delivered product, no person, in several model settings

This is the report of a trial as it was made. The programs it names (`sdc-drive`, `scripts/run-trial.sh`, `scripts/run-finding-cycles.sh`, `bindings/tools/llm-node.sh`, `bindings/tools/decider.sh`) are deleted (`story:driver-runtime-removed`); the runtime is Loom's (`docs/loom-run.md`).

Goal (`story:clean-unattended-run-compare`, `.engineering/planning/story/clean-unattended-run-compare.md:42-46`):
is the time waiting on a person in trial 2 (`docs/trial-2.md:20`) a defect of that run or what a full
run costs, and does a stronger model change the result. The idea in every run but two: "A to-do list
web app for me and my partner that also works on the train without a connection"
(`scripts/run-trial.sh --autopilot`, autonomy A5, decider final: the decider cannot hand a decision to a person).
The two small-idea runs are in their own section.

Written 2026-11-21, 11:20 UTC; section 9 updated at 11:23 UTC and 11:47 UTC, section 10 added at 11:47 UTC and brought to the end of the four runs on `trial/main-2`; every decider cost corrected (section 8, "Decider cost"). Sources are the run directories under `runs/` (git-ignored; a run is
named by its directory), the fix commits of `git log main`, and the trial runner's notes (cited as
*runner's notes*). Lines marked *(inferred)* were not observed.

## 1. Result

- Pass (a fresh run that closes `operating` by itself: exit 0, no stop record, no reopen, no forced step, replay matching, 0 s waiting on a person): 7 runs. 5 of the 8 runs of the batch tree `acd6f12` (review stage a stub), 2 of the 4 runs on `trial/main-2` at `1001970` (`strong-6` and `strong-5`, the first with a review model; section 10; `mixed-8` and `haiku-9` stopped). 0 of the 9 runs of attempts 1 to 5 and 0 of the 4 runs on `trial/main-1` (section 9).
- By setting, runs that ended: Sonnet work + Opus decider 4 of 6 (`strong-1`, `strong-2`, `strong-5`, `strong-6` passed; `strong-3`, `strong-4` stopped at the first review call on `main-1`); all Haiku 1 of 4 (`haiku-6` not delivered, `haiku-7` pass, `haiku-8` stopped, `haiku-9` stopped `writer_spent`); mixed 1 of 4 (`mixed-5` not delivered, `mixed-6` pass, `mixed-7` stopped at the review call, `mixed-8` stopped `rework_spent`); all Opus work 1 of 1; all Sonnet work with Haiku decider 0 of 1 (stopped).
- Cost and wall time of the passes (work plus every decider call, including the suppressed escalations): `strong-1` $2.93, 473 s; `strong-2` $3.00, 454 s; `haiku-7` $1.03, 2709 s; `mixed-6` $1.86, 2295 s; `opus-1` $5.09, 1662 s; `strong-6` $4.05, 677 s; `strong-5` $5.34, 967 s (sections 3 and 10). The first version of this report gave `haiku-7` as $1.00 (the tool's figure, which leaves out one decider call); the other six did not change beyond rounding.
- All 7 passes: 0 s waiting on a person, 0 person decisions, replay 10 of 10, closed by `rule:autopilot`, no stop record, one `run-settings.jsonl` line (no reopen). The 3 other closes were `not_delivered`, 0 s waiting, replay matching (section 5). The two runs the story's comparison rests on: the all-Haiku pass `haiku-7` and the review-node pass `strong-6` (Sonnet for specification, plan, implementation and reviews, Opus decider). `strong-6` (5 review calls, 2 rejections reworked) and `strong-5` (7 review calls, 4 rejections reworked) are the only passes in which a review model made a call. The all-Haiku run with a review model, `haiku-9`, stopped (section 10).
- Not shown: that any product works. No oracle check ran on any trial-3 product (section 7). With 1 or 2 runs per setting, no difference between settings is established at these sample sizes.

## 2. Settings and trees

Models per class and decider are the last line of each run's `run-settings.jsonl`
(`sdc.run-settings/1`, `models`, `decider`). Classes: writer = `spec.author`, `plan.decompose`;
implement = `change.implement`; review = `spec.review`, `change.review`; other = `idea.dissect`,
`questions.write`, `questions.answer`, `requirements.normalise`, `context.default`, `vision.write`,
`roadmap.write`.

| Setting (name used here) | writer | implement | review | other | decider | Runs |
|---|---|---|---|---|---|---|
| all Haiku | Haiku 5.5 | Haiku 5.5 | none (null) | Haiku 5.5 | Haiku 5.5 | `haiku`, `haiku-2` to `haiku-8` |
| all Haiku, review on Haiku | Haiku 5.5 | Haiku 5.5 | `spec.review` null, `change.review` Haiku 5.5 | Haiku 5.5 | Haiku 5.5 | `haiku-9` |
| mixed | Sonnet 5.5 | Sonnet 5.5 | Sonnet 5.5 | Haiku 5.5 | Haiku 5.5 | `mixed`, `mixed-2` to `mixed-8` |
| Sonnet all, Haiku decider | Sonnet | Sonnet | Sonnet | Sonnet | Haiku | `sonnet-1` |
| strong (Opus decider) | Sonnet | Sonnet | Sonnet | Sonnet | Opus 5.5 | `strong-1` to `strong-6` |
| Opus work | Opus 5.5 | Opus | Opus | Opus | Haiku | `opus-1` |

- Caps: `max_llm_calls` 90 (attempts 1 to 5), 150 (batch and main runs); `max_decider_calls` 20 (to the batch), 40 (main runs).
- Decider adapter: `metaharness` in attempt 1 (replies not schema-constrained: `decisions.jsonl` `schema_constrained` null); `claude-cli`, schema-constrained, from attempt 2 on (runner's notes, attempt 2; `decisions.jsonl`).
- A review model is set for `mixed`, `strong`, `sonnet` and `opus` runs, but on the trees up to the batch `change.review` was a program stub (`test -s impl/README.md`, `0a4b71b` body): no `change.review` or `spec.review` line is in any `llm-calls.jsonl` of a batch run. The setting names a model that was never called. From `trial/main-2` on, `change.review` runs: `strong-5`, `strong-6`, `mixed-8` and `haiku-9` made review calls (section 10; `haiku-9` ran it on Haiku, `run-settings.jsonl` `models.review.nodes.change.review`).

| Attempt | Runs | Tree | Source |
|---|---|---|---|
| 1 | `haiku`, `mixed` | `trial/3` at `cedd691` *(inferred: the last commit of the branch before the 03:52:34Z start; the notes do not name it)* | `run-settings.jsonl` `at` |
| 2 | `haiku-2`, `mixed-2` | `trial/3` at `f426876` (= `f74c68b`) | runner's notes, attempt 2 |
| 3 | `haiku-3` | `trial/3` at `4b8e177` (merges `9df3062` = `5471dab`, `4b8e177` = `37fba0b`) | runner's notes, attempt 3 |
| 4 | `haiku-4`, `mixed-3` | `trial/3b` at main `753210d` | runner's notes, attempt 4 |
| 5 | `haiku-5`, `mixed-4` | `trial/3b` at `ea2f111` (= main `32265be` + `073c173`); reopens at `acd6f12` (`haiku-5`) and `3663770` (`mixed-4`) | runner's notes, attempt 5 |
| batch | `mixed-5` (start 10:16Z), `strong-1`, `strong-2`, `haiku-6`, `haiku-7`, `mixed-6`, `sonnet-1`, `opus-1`, `small-haiku-1`, `small-sonnet-1` (start 10:26Z to 10:27Z) | `trial/3b` at `acd6f12` (= `ea2f111` + `102ce99` + `83d3cbf`) | runner's notes, attempt 5; `mixed-5` started on `3663770` and its binaries were rebuilt at `acd6f12` in its round 1 |
| main-1 | `strong-3`, `strong-4`, `mixed-7`, `haiku-8` (start 11:06Z) | `main` at `8db4b93`, branch `trial/main-1` | `git log trial/main-1`; section 9 |
| main-2 | `strong-5`, `strong-6`, `mixed-8`, `haiku-9` (start 11:33:50Z to 11:34:09Z); reopens of `strong-3`, `strong-4`, `mixed-7` (11:34:13Z) | `trial/main-2` at `1001970` (`main` `8db4b93` + `fix/review-reply` `49b0a87`) | `git log trial/main-2`; `run-settings.jsonl`; section 10 |

## 3. The comparison

Commands (from the repository root, with the binaries of this worktree's `driver/`, built by
`cargo build --bin sdc-finding-kpis --bin sdc-run-compare`):

```console
$ sdc-run-compare --run strong-1=runs/trial-3-strong-1 --run strong-2=runs/trial-3-strong-2 \
    --run haiku-6=runs/trial-3-haiku-6 --run haiku-7=runs/trial-3-haiku-7 \
    --run mixed-5=runs/trial-3-mixed-5 --run mixed-6=runs/trial-3-mixed-6 \
    --run sonnet-1=runs/trial-3-sonnet-1 --run opus-1=runs/trial-3-opus-1 --out batch.md --json batch.json
$ sdc-finding-kpis runs/trial-3-<run>
```

The table below is the `runs[].metrics` of `batch.json` (the tool's per-run tables; its
`## Comparison` lines hold the same values) with one column from another file: decisions asked and
rejected are the lines of `decisions.jsonl` whose `decision` is `approve` or `reject` (the tool
counts decider calls, which also include an invalid reply). Seconds, USD and counts are the tool's
(`docs/run-compare.md`). `sdc-finding-kpis` L7 (time waiting on a person) is 0, not failed, for all 8. The decider $ and total $ columns are not the tool's for `haiku-6`, `haiku-7` and `mixed-5` (see the correction below); they are the sum of `cost_usd` over every `decisions.jsonl` line.

| Run | Setting | End | Wall s | Driver s | Wait s | Work calls | Decider calls | Work $ | Decider $ | Total $ | Findings raised / late | Resolved: agent / decider / person | Rework revisions | Asked / rejected | Replay |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `strong-1` | Sonnet work, Opus decider | closed `operating` | 473 | 306 | 0 | 12 | 11 | 0.970 | 1.955 | 2.926 | 2 / 1 | 2 / 0 / 0 | 13 | 11 / 0 | 10 of 10 |
| `strong-2` | same | closed `operating` | 454 | 278 | 0 | 12 | 11 | 1.034 | 1.962 | 2.996 | 0 / 0 | 0 / 0 / 0 | 4 | 11 / 0 | 10 of 10 |
| `haiku-6` | all Haiku | closed `not_delivered` | 1916 | 983 | 0 | 17 | 14 | 0.304 | 0.255 | 0.560 | 19 / 17 | 16 / 3 / 0 | 68 | 14 / 1 | 10 of 10 |
| `haiku-7` | all Haiku | closed `operating` | 2709 | 1435 | 0 | 17 | 17 | 0.662 | 0.371 | 1.034 | 18 / 17 | 14 / 4 / 0 | 62 | 16 / 1 | 10 of 10 |
| `mixed-5` | mixed | closed `not_delivered` | 1693 | 478 | 0 | 15 | 14 | 1.308 | 0.280 | 1.588 | 5 / 5 | 3 / 2 / 0 | 34 | 14 / 3 | 9 of 9 |
| `mixed-6` | mixed | closed `operating` | 2295 | 775 | 0 | 16 | 14 | 1.514 | 0.344 | 1.859 | 7 / 6 | 4 / 3 / 0 | 45 | 14 / 2 | 10 of 10 |
| `sonnet-1` | Sonnet all, Haiku decider | stopped `decider_budget_spent` | 1444 | 399 | 0 | 24 | 20 | 1.580 | 0.303 | 1.882 | 3 / 3 | 1 / 2 / 0 | 113 | 20 / 0 | not recorded |
| `opus-1` | Opus work, Haiku decider | closed `operating` | 1662 | 989 | 0 | 12 | 11 | 4.884 | 0.204 | 5.088 | 9 / 8 | 9 / 0 / 0 | 32 | 11 / 0 | 10 of 10 |

- Correction of the decider cost. `sdc-run-compare` on `main` (before 0.1.1: `df58754`, `9ff2996`; the 0.1.1 notes name the defect) leaves out the `decisions.jsonl` lines whose `decision` is `escalation_suppressed`, and those lines carry the cost of a real decider call (the line that follows, the best decision taken, has `cost_usd` null). `haiku-6` has 1 such line, `haiku-7` 1, `mixed-5` 3; the other five runs have none and are unchanged. Recomputed here as the sum of `cost_usd` over `llm-calls.jsonl` and over all `decisions.jsonl` lines; the decider figure also equals the sum of `decider/*/response.json`; the fixed tool prints the same. Decider calls and asked / rejected counts do not change (a suppressed line and its following decision are one call). The first version of this report took the tool's figures.
- Findings dismissed, unresolved and rejected by the program check: 0 in all 8 runs (`batch.md`); oracle check `none` in all 8.
- Late finding = a finding whose stage is not `requirements` (`docs/run-compare.md`, findings late).
- Decisions by a person: 0 in all 8 (tool metric).
- Ends: 5 closed `operating`, 2 closed `not_delivered` (section 5), 1 stopped (section 6). The tool prints the stop text of `sonnet-1` as `awaiting_authority ... autopilot:` with no reason; the cause is the `cause` field of its `run-stopped.json`.

All Haiku against Sonnet for the specification, plan and implementation (`llm-calls.jsonl`, `action`, `cost_usd`):

| Run | `spec.author` calls, $ | `plan.decompose` calls, $ | `change.implement` calls, $ | Merge rejections |
|---|---|---|---|---|
| `haiku-6` (Haiku) | 2, 0.040 | 1, 0.006 | 4, 0.222 | 0 |
| `haiku-7` (Haiku) | 1, 0.023 | 1, 0.003 | 6, 0.606 | 1 |
| `mixed-5` (Sonnet) | 1, 0.187 | 1, 0.045 | 5, 1.048 | 2 (both on `list.sync`; the second with the escalation suppressed) |
| `mixed-6` (Sonnet) | 2, 0.245 | 1, 0.050 | 5, 1.194 | 2 |
| `strong-1` (Sonnet) | 1, 0.177 | 1, 0.041 | 3, 0.458 | 0 |
| `strong-2` (Sonnet) | 1, 0.186 | 1, 0.043 | 3, 0.521 | 0 |
| `sonnet-1` (Sonnet) | 15, 1.108 | 1, 0.043 | 1, 0.140 | 0 (stopped in the specification) |
| `opus-1` (Opus) | 1, 0.448 | 1, 0.094 | 3, 3.652 | 0 |

- Per call (table cost over calls): `spec.author` Haiku $0.020 and $0.023, Sonnet $0.12 to $0.19, Opus $0.45; `change.implement` Haiku $0.056 and $0.10, Sonnet $0.15 to $0.24, Opus $1.22. Not a paired measurement: the runs wrote different code.
- Review: no model made a review call in any of the 8 runs. The review stage of every batch run was the stub, so whether a Sonnet or an Opus review changes a result was not tried. The runs from `main` are the first with a review model: three stopped at its first call (section 9); `strong-6` on `trial/main-2` is the first pass with one (section 10).
- Merge rejections that were reworked and then approved: `haiku-7` 1, `mixed-6` 2, `mixed-5` 2 (`decisions.jsonl`; the approval followed each time, then the release refusal in `mixed-5`). Each rejected the change against a requirement and cited a file line from the change (examples: `mixed-6`, offline copy not kept on the device and the server bound to 127.0.0.1 only; `haiku-7`, offline adds not synced).
- Deciders: the Opus decider rejected nothing in `strong-1` and `strong-2` (11 decisions asked each, 0 rejections; `strong-3` and `strong-4` from `main` also 0 of 6, but stopped before a merge). `opus-1` used a Haiku decider (work on Opus) and also had 0 rejections of 11. The Haiku-decider runs that passed went through merge rejections with rework: `mixed-6` 2, `haiku-7` 1. In `strong-1` and `strong-2` every merge was approved at the first ask, with 3 `change.implement` calls (one per story). Whether that is the Opus decider or a Sonnet writer that needed no rework is not separable here: `sonnet-1` (Sonnet work, Haiku decider) never reached a merge.
- The decider's principal is recorded as `decider:haiku` whatever model decided; the model is only in `decisions.jsonl` `model` (section 8).
- Cost: the Opus decider is most of the cost of a strong run (decider $1.96 of $2.93 in `strong-1`); Opus work is the most expensive setting ($4.88 of $5.09 is work in `opus-1`); all Haiku is the cheapest pass ($1.03) and the slowest ($2709 s against 454 to 473 s for strong). `haiku-7` took 15 rounds and 62 rework revisions, `strong-1` and `strong-2` 9 rounds and 13 and 4 (`rounds.jsonl`, tool).
- Conclusion: one or two runs per setting. No difference between the settings is established, in pass rate, cost, time, findings or rework. What the numbers support: every setting except Sonnet-all-Haiku-decider closed at least once; the Opus decider never rejected, and its two runs closed without rework of a story; the cost ranking above holds for these runs.

### Per setting

| Setting | Runs | Passed | Wall s | Total $ |
|---|---|---|---|---|
| Sonnet work, Opus decider | `strong-1`, `strong-2` | 2 of 2 | 473, 454 | 2.926, 2.996 |
| all Haiku | `haiku-6`, `haiku-7` | 1 of 2 | 1916 (not delivered), 2709 | 0.560, 1.034 |
| mixed | `mixed-5`, `mixed-6` | 1 of 2 | 1693 (not delivered), 2295 | 1.588, 1.859 |
| Opus work | `opus-1` | 1 of 1 | 1662 | 5.088 |
| Sonnet all, Haiku decider | `sonnet-1` | 0 of 1 | 1444 (stopped) | 1.882 |

## 4. The attempts before the batch

Figures are `sdc-run-compare` over `runs/trial-3-haiku` to `runs/trial-3-mixed-4` (`attempts.md`; wall
seconds of `haiku-5` and `mixed-4` include the gap before a reopen). None passed. Causes are those of the
fix commit bodies and the runner's notes.

| Attempt | Run: where it stopped | Cause | Fix | How far the next got |
|---|---|---|---|---|
| 1 | `haiku` and `mixed`: `IDEA-1.direction`, `context.approve`, exit 6 after 92 s and 95 s | The decider's reply cited ids that did not exist (`context.platform` and others); `decider.sh` built the citable ids with a `yq` expression that fails in yq 4.30, so none was citable; the invalid reply became a reject under escalate final | `bbd927e` (resolvable citations, retry on an invalid reply); `f74c68b` (`SDC_DECIDER_ESCALATE=final` exported under `--autopilot`); `2e616c4` (a stop by rule is no wait in L7) | attempt 2: intake and direction passed in both |
| 2 | `haiku-2`: `IDEA-1.list.spec`, `spec.author` spent (4 runs of 4), exit 4 | `spec.trace` failed (exit 5) every loop; the Haiku writer raised a new finding on each re-derivation | `7f5ef7c` (drop repeated findings, no LLM re-derivation for own defaults), `37fba0b` (say what sent a spent writer back), on `fix/spec-trace-loop` | attempt 3: specification and plan passed |
| 2 | `mixed-2`: `IDEA-1.list.items`, exit 6 | The decider rejected the merge validly; nothing sent a rejected change back to the implementer | `a597a6a` (the stop names the cause `rejected_without_rework_path`), `a23755a` (a rejected approval sends the writer back, at most 2 reworks) | attempt 4: rework ran (`haiku-4`, `mixed-3`) |
| 3 | `haiku-3`: `IDEA-1.app.release`, `release.publish` rejected, exit 6 | The release had no sign-in (r3) and no device-side offline copy (r4), sync half built; the publish approval has no `rejected` result and no writer | `25e0709` (protocol: a refused release ends the tree without a person), `b6ce269` (`run-trial.sh` closes it withheld, `not_released`, `not_delivered`), `eb8e384` (every requirement must trace to behaviour in the spec: r3 cited an actor with no command and passed) | attempt 4: `mixed-3` closed; `haiku-4` reached the second story (`todo.list`) |
| 4 | `haiku-4`: `IDEA-1.todo.list`, `change.implement` spent (3 runs), exit 4 | The rework after a rejected merge raised findings; the next `change.implement` ended "not called" and that empty run took the last of `max_runs 3`; separately, replay of the root failed after a child closed (7 of 9) | `b296a18` (a not-called run is not a run; a pending rejection is reworked), `ccb3571` (the parent is evaluated after a child closes), merged in `d241d21` | attempt 5: `haiku-5` passed every story and the release |
| 4 | `mixed-3`: closed `not_delivered` | False refusal (section 5) | `073c173` | attempt 5 |
| 5 | `haiku-5`: root close, exit 6, `rejected_without_rework_path` | The decider rejected the close of a terminable `operating` root (r1 cited both Postgres and the data file; the roadmap check true while the artifact was draft; 8 scope-changing decisions by `decider:haiku`) | `83d3cbf` (D140: under A5 `rule:autopilot` closes a terminable root, the decider is not asked), merged in `6f82949` | reopened at `acd6f12`: closed `operating` in its first round, no new call; not a pass |
| 5 | `mixed-4`: `IDEA-1.intake`, `amend.upstream`, `decision_not_takeable`, exit 7 after 55 s in the driver | The program check refused the decider's valid decision on f1 because r3 cited a dark spot (a fault the decision did not write) | `102ce99` (citations checked at write; a decision refused only for faults it wrote), merged in `9322e51`; reopen at `3663770` stopped again at once; `6c4886a` on `fix/attempt5-close` (`--reopen` puts a refused decision back for the current check) is not merged | not rerun in the batch |

- `haiku-5` replay 9 of 9; `haiku-4` replay 7 of 9 (`IDEA-1.todo` state mismatch, `IDEA-1.todo.sync` never evaluated), the first fixed by `ccb3571` (runner's notes, attempt 4); `haiku-3` replay 9 of 9; stopped runs without a `replay.json` read `not recorded`.
- Waiting on a person in `sdc-run-compare` for these runs: `haiku` 9 s, `mixed` 9 s, `mixed-2` 85 s, `haiku-3` 52 s (the gap from the last round's end to the stop record, which the tool counts; `sdc-finding-kpis` L7 does not count a stop by rule, `2e616c4`), `haiku-5` 553 s and `mixed-4` 2870 s (time stopped for a fix before a reopen). `9ce86bf` on `fix/attempt5-close` separates "time stopped for a fix" from L7; it is not merged, so the tables here still show those figures.
- Costs, total (work plus every decider call, section 3 correction): `haiku` $0.026, `mixed` $0.027, `haiku-2` $0.211, `mixed-2` $0.886, `haiku-3` $0.469, `haiku-4` $0.669, `mixed-3` $1.487, `haiku-5` $0.779, `mixed-4` $0.042. The first version took the tool's figures, $0.015, $0.017, $0.186, $0.886, $0.447, $0.618, $1.386, $0.761 and $0.042; `mixed-2` and `mixed-4` have no suppressed line and did not change.

## 5. Runs that closed by themselves as `not_delivered`

`mixed-3` (attempt 4), `mixed-5`, `haiku-6` (batch). Each closed with the decider's refusal of `release.publish`; the release was recorded withheld and the tree closed `not_delivered` without a person.

| Run | What the decider withheld, and why | Right about the product? |
|---|---|---|
| `mixed-3` | The release (08:52:25Z): `health.required` was an open obligation with no claim value and no health route, and publishing with an open obligation looked unsafe (`decisions.jsonl`) | No: a tool defect. The obligation is met only by a `release.observe` after the publish, so it was never a condition of the publish. Fixed in `073c173` (`sdc-reply later`; the decider lists it under "met after this action"). Its two earlier merge rejections (offline POSTs fail because the service worker answers GET only, `impl/src/main.rs:28`; `r3` against `q8`, sync with no offline queue) were about the product and led to rework |
| `mixed-5` | The release (10:44:36Z): with no token the server still opens the list; vision and roadmap say the list link is the only way in (`aep/vision/product.md:25`, `aep/initiative/roadmap.md:30`); `q7` ("any token opens the list") and the README (`release/README.md:47`) leave a request with no token open | Yes about the code: `GET /api/items` answers without a token (`store/IDEA-1/work/release/src/web.rs:161`) and `open_list` takes an empty token (`web.rs:163-164`). The requirement is itself in conflict (`q7` against the vision); that conflict is what the decider named |
| `haiku-6` | The release (10:58:29Z): the `SharedList` view and the device-side offline page are not built, sync has no HTTP route, so r1, r2, r4, r5 are not met | Yes: the release's own README says so (`store/IDEA-1/work/release/README.md:98`, `:169`, `:172-176`; `GET /` is the sign-in page, `:42`). The earlier merge approvals let thin stories through, none rejected (Haiku decider, 1 reject of 14) |

- A refusal at the release is the first point where the whole product is read; the stories before it were approved (`haiku-6`: 0 merge rejections).
- `mixed-3` and `mixed-5` carry `closed_by: null` in `status.json` (older writer); `haiku-6` carries `rule:autopilot`.
- Roadmap left saying offline editing in `mixed-3` after story decision `q8`: `story:roadmap-rederived-after-story-amend` (`1b57466`, on `fix/superseded-answers`).

## 6. Stops in the batch

| Run | Stop | Cause | Fix |
|---|---|---|---|
| `sonnet-1` | 10:50:37Z, `rule:autopilot`, `IDEA-1.todo.access`, `decider_budget_spent` (20 of 20 decider calls) | 14 of the 20 decider calls were the trace gap `p-trace-r7` on one finding (the decider answered "r7 is a constraint, not behaviour"; the answer was not recorded as the constraint option, the gap returned); `spec.author` ran 15 times (`llm-calls.jsonl`); the stop reason was empty (`autopilot: `) | `fix/superseded-answers` `b4aa0d1` (not merged): a decision that starts with the constraint option sets `behaviour`; a decided finding supersedes its earlier answers; a re-raised finding stops the run `finding_reraised`; the budget stop names step and cap. The branch's text names `small-sonnet-1`; `sonnet-1` shows the same pattern *(inferred: not rerun on the branch)* |
| `mixed-4` | 09:28:33Z and, reopened, 10:16:23Z, `decision_not_takeable` in `IDEA-1.intake` | section 4, attempt 5 | `102ce99` on main; `6c4886a` on `fix/attempt5-close` (not merged). It was not run again on a fixed tree: it is an attempt-5 run, not a batch run |
| `small-haiku-1` | 10:50:11Z, `IDEA-1.counter.count`, `writer_spent` (3 runs of `change.implement`) | see below | none |
| `small-sonnet-1` | 10:46:51Z, `IDEA-1.count.spec`, `decider_budget_spent` | 15 of 20 decider calls were `p-trace-r5` (8) and `p-trace-r7` (7), the `sonnet-1` pattern | `fix/superseded-answers` `b4aa0d1` |

`fix/attempt5-close` also holds `2a95956` (`--keep-going`: a trial run forced past a stop records the defect, D142), `6c4886a` and `9ce86bf`; none is in `main` or in any run above.

### Small idea (own section)

The idea (`runs/trial-3-small-haiku-1/store/IDEA-1/work/idea.yaml`): a command-line tool that prints the lines, words and characters of a text file; one argument; standard library only. Figures: `sdc-run-compare --run small-haiku-1=runs/trial-3-small-haiku-1 --run small-sonnet-1=runs/trial-3-small-sonnet-1` (`small.md`).

| Run | Setting | End | Wall s | Driver s | Wait s | Work calls / $ | Decider calls / $ | Total $ | Findings raised / late | Rework | Asked / rejected |
|---|---|---|---|---|---|---|---|---|---|---|---|
| `small-haiku-1` | all Haiku | stopped `writer_spent` | 1349 | 662 | 0 | 18 / 0.174 | 12 / 0.191 | 0.366 | 13 / 12 | 60 | 12 / 2 |
| `small-sonnet-1` | Sonnet all, Haiku decider | stopped `decider_budget_spent` | 1092 | 221 | 57 (tool; L7 0) | 16 / 0.843 | 20 / 0.270 | 1.113 | 4 / 4 | 59 | 20 / 0 |

- `small-haiku-1`: the decider rejected `merge.approve` twice because the change was Rust while r5 says Python 3 (`requirements.yaml:41`); the artifacts name both languages (`questions.yaml:31`, `:223-228` say Rust); the second rework added the Python package beside the Rust crate and left the crate. No branch fixes it. The conflict between the requirement and the decided questions is the observed cause; why the writer chose Rust was not read *(inferred: the questions' options)*.
- Replay: none recorded for either stopped run. No oracle.

## 7. The oracle check

- `docs/eval-design.md` defines success as a legitimate Canon outcome and every hidden oracle check holding, decided by programs only (M1, `:129`); the oracle file `oracle.json` sits beside the case tree, out of the agent's reach, with one `held`, `contradicted` or `unknown` per check (`:174-176`).
- For a specification the check is `eval/oracle-spec.sh` (`:589`): it synthesizes the suite from the authored spec and runs it against ESS's `billing` reference target; only `billing` is confirmed as a target (`:429-431`). For a story the oracle is conformance against the actual implementation, which ESS 0.96.0 cannot run (D1, `:616`; `story:conformance-against-implementation` waits on an ESS release).
- Not run on any trial-3 product: no `oracle.json` exists in any `runs/trial-3-*` (searched). `sdc-run-compare` prints `none` for all of them.
- It could not have been run as written *(inferred, not tried)*: the to-do spec has no `billing` target to run against.
- So the 7 passes are Canon outcomes closed by the program rule, not delivered products. No one started a delivered product: whether any of them serves a list, syncs after being offline or opens from the link was not observed by running it.

## 8. Known limits and faults seen in passing

- Citations: the program checks that a decider cite exists (`sdc-reply resolve`, `bbd927e`), not that it supports the claim. Every merge or publish approval of the 8 runs that had one (`haiku-5` to `opus-1` without `sonnet-1`) cites at least one `path:line` except one approval in `opus-1`; the number is 1 to 23 per approval (`decisions.jsonl` `citations`, form `path_line`). `strong-1` cites 1, 5, 6 and 5. Whether the cited line says what the reason says was not read for these approvals.
- The to-do spec's `topology.yaml` names a Postgres store in every run that reached a spec (for example `trial-3-haiku-5/store/IDEA-1/work/spec/topology.yaml:10`); the products use a data file (`haiku-5` `published/README.md:86-87`, `release/src/main.rs:3`; `strong-1`, `strong-2`, `mixed-6`, `opus-1` READMEs name a data file). In `haiku-5`, r1 cited both Postgres (`q10`) and the data file (`q12`) (`requirements.yaml:3`), which is what the decider's close rejection quoted. The brief of this report said the published spec named Postgres: the spec's topology does, the published README states the departure.
- Record faults: the principal is `decider:haiku` in `events.jsonl`, `decisions.jsonl` and `run-settings.jsonl` (`decider.kind`) whatever model decided (`strong-1`: model `claude-opus-5-5`, principal `decider:haiku`); the driver's "LLM calls so far" counts runs that called no model (23 against 18 lines in `haiku-5`, runner's notes; `strong-1` prints 13, the record has 12), fixed in `9ce86bf` (not merged); `run-stopped.json` has no `cause` before `a597a6a`; `sonnet-1` and `small-sonnet-1` stop reasons are empty.
- Tool figures that do not mean what they say: `haiku-5` 553 s and `mixed-4` 2870 s "waiting on a person" are time stopped for a fix; `strong-4` is 1 s in `sdc-run-compare` and 0 in `sdc-finding-kpis`.
- Pass criteria: "no forced step" was read from the events (only `decider:haiku` and `rule:autopilot` principals) and from `run.log`; the tool of `main` has no figure for it (it is on `fix/attempt5-close`).
- Products were not run or read as a whole; `haiku-6` and `mixed-5` show that a story set can pass every merge and still miss the idea.
- `small-haiku-1` cause: requirements chose Python where the build prompt demands Rust. The coordinator assigned the fix to `fix/superseded-answers` as a build-constraints file read by every stage; it is not in that branch's commits or in its worktree *(not observed)*.
- The decider's budget of 20 calls was spent by one repeated finding twice (section 6); the main runs have 40.
- Decider cost: before 0.1.1 (`df58754`, `9ff2996`), `sdc-run-compare` summed the cost of the `decisions.jsonl` lines that are not `escalation_suppressed`, so a decider call whose escalation was suppressed (`SDC_DECIDER_ESCALATE=final`, the call whose result is re-taken as the best decision) was missing from its decider cost and total. Every cost in this report is recomputed from all lines (section 3). Affected runs (tool figure to recomputed total): `haiku-7` $0.998 to $1.034, `haiku-6` $0.540 to $0.560, `mixed-5` $1.476 to $1.588, `haiku-2` $0.186 to $0.211, `haiku-3` $0.447 to $0.469, `haiku-4` $0.618 to $0.669, `mixed-3` $1.386 to $1.487, `haiku-5` $0.761 to $0.779, `haiku-8` $0.331 to $0.346, `small-haiku-1` $0.343 to $0.366, `haiku-9` $0.711 to $0.878, `mixed-8` $1.236 to $1.272. All-Haiku runs are hit most, since the Haiku decider escalates most often; the Opus-decider runs had no suppressed line.

## 9. Runs from main with the review node (`trial/main-1`)

State at 2026-11-21 11:47 UTC. `main` at `8db4b93`, branch `trial/main-1`; the first runs with a review model
(`change.review`, class REVIEW, `0a4b71b`; `e46c21f` puts it after the tests). Figures: `sdc-run-compare` over
the four directories; work and decider calls and cost re-added from `llm-calls.jsonl` and `decisions.jsonl`.
None of the four passed.

| Run | Setting | End | Wall s | Work calls / $ | Decider calls / $ | Total $ | Asked / rejected |
|---|---|---|---|---|---|---|---|
| `strong-3` | Sonnet work, Opus decider | stopped 11:12:15Z, `stuck` in `IDEA-1.todo.access`; reopened 11:34:13Z on `main-2`, stopped again 11:34:14Z, `review_invalid` | 350 first leg | 12 / 0.814 | 6 / 1.014 | 1.828 | 6 / 0 |
| `strong-4` | same | stopped 11:12:15Z, `stuck` in `IDEA-1.todo.list`; reopened and stopped again as `strong-3` | 349 first leg | 11 / 0.707 | 6 / 0.990 | 1.697 | 6 / 0 |
| `mixed-7` | mixed | stopped 11:17:09Z, `stuck` in `IDEA-1.todo.access`; reopened and stopped again 11:34:15Z, `review_invalid` | 643 first leg | 14 / 0.627 | 8 / 0.082 | 0.709 | 8 / 0 |
| `haiku-8` | all Haiku | stopped 11:36:16Z, `writer_spent` in `IDEA-1.list.access` (`run-stopped.json` `cause`) | 1791 | 16 / 0.203 | 11 / 0.143 | 0.346 | 11 / 0 |

- `strong-3`, `strong-4`, `mixed-7` stopped at their first `change.review` call: the review call (Sonnet, read-only operations) exited 0 and the check refused it, "invalid review reply: no readable reply" at `out/review.json` after 3 attempts (`review-check.err` in each run's `llm/NNN-change.review/`; `llm-calls.jsonl` `applied_exit` 2). The review model cannot write `out/review.json` (the coordinator's reading; `out/` is empty in all three runs). The case then had no admissible and useful action (`stuck`, driver exit 4). No story of these runs was merged. Fix: `49b0a87` on `fix/review-reply` (section 10).
- The reopens of the three, on `trial/main-2`, ended in about 1 s with `review_invalid` naming the old call and no new LLM call: a reopen did not dispatch the review again. Fixed on `fix/review-reply` at `3cbda08` (a step that recorded nothing runs once more after a reopen; not on `trial/main-2` at `1001970`). The tool prints 1318 s, 1319 s and 1027 s waiting on a person for the three; that is the time stopped for a fix before the reopen (the `fix/attempt5-close` caveat of section 8); `sdc-finding-kpis` L7 also reads FAILED for them.
- `haiku-8`: `tests.run` failed 3 times in `IDEA-1.list.access` and the writer's feedback held only the build pool's line, not the test output (the coordinator's reading: not verified here); the feedback defect is assigned on `audit/reject-routes`. All Haiku has no review model (`review` null); `haiku-9` on `trial/main-2` ran `change.review` on Haiku (section 10).
- Waiting on a person 0 s for `strong-3`, `strong-4`, `mixed-7`, `haiku-8` in their first legs (L7 0 before the reopen).

## 10. Runs on `trial/main-2` (review model working)

State at 2026-11-21 13:55 UTC: all four runs have ended (`strong-6` 11:45:26Z, `strong-5` 11:49:57Z, `mixed-8` 11:51:19Z, `haiku-9` 12:31:35Z). Tree: `trial/main-2` at `1001970` (`main` at `8db4b93` merged with `fix/review-reply` `49b0a87`: the review class runs on a schema adapter, and a run that cannot is refused). Figures: `sdc-run-compare` of 0.1.1 (`9ff2996`) (`--run strong-6=runs/trial-3-strong-6 --run strong-5=runs/trial-3-strong-5 --run mixed-8=runs/trial-3-mixed-8 --run haiku-9=runs/trial-3-haiku-9 ...`, the table is its `runs[].metrics`), checked for each run by summing `cost_usd` over `llm-calls.jsonl` and over every line of `decisions.jsonl` with `jq` (section 3 correction); the review calls from `llm-calls.jsonl`. The first version of this section was written at 11:47Z while three of the runs were still running.

| Run | Setting | End | Wall s | Driver s | Wait s | Work calls / $ | Decider calls / $ | Total $ | Findings raised / late | Resolved: agent / decider / person | Rework | Asked / rejected | Replay |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `strong-6` | Sonnet work and review, Opus decider | closed `operating` 11:45:26Z | 677 | 514 | 0 | 21 / 2.085 | 11 / 1.970 | 4.055 | 2 / 2 | 2 / 0 / 0 | 18 | 11 / 0 | 10 of 10 |
| `strong-5` | same | closed `operating` 11:49:57Z | 967 | 788 | 0 | 24 / 3.259 | 11 / 2.078 | 5.338 | 3 / 3 | 3 / 0 / 0 | 19 | 11 / 0 | 10 of 10 |
| `mixed-8` | mixed (Sonnet writer, implement, review; Haiku rest and decider) | stopped 11:51:19Z, `rework_spent` in `IDEA-1.todo.access` | 1030 | 394 | 114 (tool) | 16 / 1.118 | 10 / 0.154 | 1.272 | 4 / 4 | 3 / 1 / 0 | 19 | 10 / 3 | not recorded |
| `haiku-9` | all Haiku, review on Haiku | stopped 12:31:35Z, `writer_spent` in `IDEA-1.list.tasks` | 3446 | 2198 | 0 | 22 / 0.552 | 17 / 0.325 | 0.878 | 25 / 23 | 18 / 7 / 0 | 70 | 17 / 1 | not recorded |

The mixed-8 wait of 114 s is the time after the stop record that the tool counts; no person decided (0 person decisions) *(inferred: the cause of the 114 s was not traced)*.

`strong-6` is the first pass with a review model, and the run that answers the story's "Sonnet for the specification, plan, implementation and reviews". Its decider was Opus (`run-settings.jsonl`: `decider.model` `claude-opus-5-5`), so it is the strong setting of section 3 with the review stub replaced; it does not isolate the review from the decider.

### `strong-6` and `strong-5` (passes)

- Both closed `operating` by `rule:autopilot`, exit 0, no stop record, one `run-settings.jsonl` line (no reopen), replay 10 of 10, 0 s waiting on a person, 0 person decisions (`status.json`, `replay.json`, `sdc-finding-kpis` L7 0 for `strong-6`; `strong-5` read from the tool).
- `strong-6`: 21 work calls $2.085, of which 5 are `change.review` ($0.748): all Sonnet on `claude-cli`, `exit` 0. Decider: 11 Opus calls $1.970, 11 approvals, 0 rejections. Total $4.055.
- The 5 reviews of `strong-6`: `todo.offline` rejected (11:39:40Z: `impl/src/main.rs:14-21`, the list is never cached, no local copy or service worker, so offline viewing fails; violates r3 and `aep/story/offline.md:7`), approved on the second (11:40:43Z); `todo.sharing` approved (11:41:58Z); `todo.task` rejected (11:43:52Z: `impl/src/main.rs:7-30`, the UI never calls the task API, violates r5 and r1), approved on the second (11:44:33Z). A rejection is a valid reply (`applied_exit` 1).
- Both rejections were reworked (`change.implement` 5 calls, against 3 stories) and then approved by the review and by `merge.approve` (`decisions.jsonl`: 3 approvals, at 11:41:20Z, 11:42:34Z, 11:45:07Z).
- `strong-5`: 24 work calls $3.259, of which 7 are `change.review` ($1.149; `applied_exit` in order 1, 1, 0 on `todo.offline`, 1, 0 on `todo.sharing`, 1, 0 on `todo.task`: 4 rejections, 3 approvals) and 7 `change.implement` ($1.559). Decider: 11 Opus calls $2.078, 11 approvals, 0 rejections, 3 `merge.approve` approvals (11:42:39Z, 11:47:06Z, 11:49:37Z). Total $5.338.
- Against `strong-1` and `strong-2` (same setting, review a stub, 3 `change.implement` calls each, $2.93 and $3.00, 473 s and 454 s): `strong-6` cost $4.05 and took 677 s, `strong-5` $5.34 and 967 s; the extra is the reviews ($0.75 and $1.15) and the reworks (2 and 4 rejections, 5 and 7 `change.implement` calls against 3 stories). Whether a review rejected what the decider would have missed is not shown: the Opus decider approved every merge in all four runs, and the four runs wrote different code. The review rejections cite lines of the change; I did not run the product (section 7).

### `mixed-8` (stop)

- Stop 11:51:19Z, `rule:autopilot`, `merge.approve` in `IDEA-1.todo.access`, cause `rework_spent` (`run-stopped.json`, `cause`): the Haiku decider rejected the change three times (11:43:55Z, 11:48:11Z, 11:51:19Z). Each time `change.built`, `change.reviewed`, `gate.passed` and `tests.pass` held, so the rejections rest on the requirements: the first on the share link served on 127.0.0.1 only (`impl/README.md:11`) and the list in a local data file instead of a managed database (r5, `impl/README.md:9`); the third on the one list kept in a line-protocol server's data file, not a managed database (`impl/README.md:18`, `:20`), with the database client deferred by `q9` to a story the plan does not contain (`aep/story/access.md:7`, `aep/initiative/roadmap.md:30`). The 3 reviews of the story (Sonnet, `applied_exit` 0) approved each revision. The rejected points of the first and second were addressed in the next revision; the one that was not, the managed database, is a requirement the plan has no story for.
- At each revision the review approved and the decider rejected; why the review did not reject the points the decider found was not read *(inferred: the review checks the change against the story, the decider against the requirements)*.

### `haiku-9` (stop)

- Stop 12:31:35Z, `rule:autopilot`, `change.implement` in `IDEA-1.list.tasks`, cause `writer_spent` (3 runs, last exit 0; exit 4 of the driver; the 3 runs are the `change.implement` runs after the merge rejection, below). It is the first all-Haiku run with a review model (`change.review` on Haiku, `spec.review` null) and merged `list.access` and `list.offline` before `list.tasks`.
- Cause: the decider's own decision on `q29` (12:20:55Z, `amend.upstream`, "Leave the list commands as library calls until a session story exists") conflicts with what `merge.approve` and `change.review` then demanded. `q29` says the list commands AddTask, EditTask and TickTask have no route because no session story says which partner is asking (`store/IDEA-1/work/questions.yaml:411-416`). The review of the third revision approved (the decision was already taken); the decider then rejected `merge.approve` at 12:25:17Z (all four checks held) on the ground that the web app shows no list (no list route; r1 needs the shared list; `q22` and `q29` defer the list to a session story the roadmap does not contain, `aep/epic/list.md:7`), and the first two of the three reworks after it were each rejected by the Haiku review for having no route to the list (`impl/README.md:57-61`, `impl/src/api.rs:84-105`, `impl/src/api.rs:92-103`, against `aep/story/tasks.md:7`, r1 and r2). The writer could not satisfy both: the decided question forbids a route, the review and the merge check require one. The stop is the writer cap, but the cause is the contradiction between the decision and the checks, not the writer.
- The order of the case, read from `events/IDEA-1.list.tasks.jsonl` (`seq`, action, exit): 6 `change.implement` runs and 5 `tests.run`. `seq` 8 `change.implement`, 13 `tests.run` 5, 22 `change.implement`, 26 `tests.run` 5 (the first two revisions failed the tests and were reworked before any review); 38 `change.implement`, 43 `tests.run` 0, 47 `change.review` 0 (approved); 54 `merge.approve` (rejected, 12:25:17Z); 58 `change.implement`, 63 `tests.run` 0, 67 `change.review` 1 (rejected); 71 `change.implement`, 76 `tests.run` 0, 80 `change.review` 1 (rejected); 84 `change.implement`, then the stop (no tests run on it). So 2 runs after failed tests, 1 green, 3 after the merge rejection; those 3 are the runs `writer_spent` counts. `tests.run` passed on every revision that reached a review, so it is not the cause. The order was given by the coordinator and checked against the log here.
- Cost: work 22 calls $0.552 (5 are `change.review`, $0.158: 3 approvals, 2 rejections), decider 17 calls $0.325 (6 suppressed escalations; the tool figure was $0.159), total $0.878. The decider cost of the first version of this section was wrong in the same way.
- Read with the section 6 stops: the same shape as `small-haiku-1` (a decided question and a requirement that disagree, the writer spent between them), now with a review model in the loop.

### What section 10 supports

- Read the table with section 3's caveat: one or two runs per setting, two passes with a review model, both on the strong setting; the two Haiku-decider runs with a review model (`mixed-8`, `haiku-9`) both stopped on a conflict between the decider and the review or the requirements. No difference between settings is established at these sample sizes. The comparison the story asked for rests on two runs: the all-Haiku pass `haiku-7` (batch tree, no review model, section 3) and the review-node pass `strong-6` (this section, Opus decider).
