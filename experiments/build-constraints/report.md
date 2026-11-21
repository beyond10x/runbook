# build-constraints: Build constraints known to every stage (fix/superseded-answers on 0.1.2, 3ca26fe), Haiku and mixed against the release-0.1.2 runs

Written by `sdc-experiment report build-constraints` from `experiments/build-constraints.yaml` and the run summaries in `experiments/build-constraints/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: prospective; status: complete; created 2026-11-21.
- Purpose: Whether one run file of build constraints, read by the writers and the decider and held by requirements.check and the decider's answer check, raises the Haiku setting's clean passes and lowers change.implement reruns against the same settings on 0.1.2.
- Hypothesis: none; the record describes its runs.
- Input `todo-app` (idea, sha256 f384a752a2cb): A to-do list web app for me and my partner that also works on the train without a connection
- Tree: `3ca26fee9a874cba6b0f7707d73a51af1f734df2`
- Runs per variant: 3; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (descriptive): Treatment (this record, tree 3ca26fe) against control (release-0.1.2, haiku and mixed, tree 72a3eef), per setting, n = 3 against n = 3, directions registered here before any treatment run. A difference is claimed only when the two ranges separate completely in the registered direction (exact one-sided p = 1/20); overlapping ranges claim no effect. Primary 1: run.clean of the haiku setting, direction increase: held only when all 3 treatment runs are clean and no control run is; any other count is inconclusive or no move and is published as such. Primary 2: node.story/change.implement.rework_takes per run, direction decrease, judged per setting (haiku, mixed); a run that never took change.implement has no value and is listed, not counted. The two primaries are judged separately and no combined claim is made from them (two tests: one holding by chance is more likely than 1/20). Secondary, reported and not decided: the count of stops with cause writer_spent, and the count of change.implement and merge.approve rejections whose reason names a language, a dependency or a stored technology. Guards, against the control per setting: run.delivered (worse when lower), run.cost_usd (worse when higher by more than half the control median), run.decider_calls and the decision classes' invalid_replies (worse when higher; the new decider answer check is expected to add some, reported with their reasons), run.wall_s (worse when higher by more than half the control median). Adopt (merge fix/superseded-answers-0.1.2) only when a primary holds and every guard holds; a refuted or inconclusive result is published as such.

## Settings

| setting | haiku | mixed |
|---|---|---|
| `adapter` | metaharness | metaharness |
| `autonomy` | A5 | A5 |
| `decider.adapter` | claude-cli | claude-cli |
| `decider.escalate` | final | final |
| `decider.kind` | model | model |
| `decider.max_decider_calls` | 40 | 40 |
| `decider.model` | claude-haiku-5-5 | claude-haiku-5-5 |
| `max_llm_calls` | 150 | 150 |
| `max_wall_seconds` | 5400 | 5400 |
| `models.implement.model` | claude-haiku-5-5 | claude-sonnet-5-5 |
| `models.other.model` | claude-haiku-5-5 | claude-haiku-5-5 |
| `models.review.model` | claude-haiku-5-5 | claude-sonnet-5-5 |
| `models.writer.model` | claude-haiku-5-5 | claude-sonnet-5-5 |
| `review.adapter` | claude-cli | claude-cli |

Known confounds:

- `control-time`: the control runs (release-0.1.2) ran earlier on the same day, before these; the model service, the canon checkout and the machine's load may differ between the two launches.
- `tree`: 3ca26fe is 0.1.2 (72a3eef) plus fix/superseded-answers merged, the decider-constraints cycle's principal fix and the re-packed replay recording; only the merge changes what a run does.
- `unstick`: run.unstick is on with an Opus model in both arms and is no launch setting (scripts/run-trial.sh); an unstuck run is not clean.
- `canon`: the canon binary of the canon checkout at run time, not pinned, in both arms.
- `control-complete`: if release-0.1.2 ends with fewer than 3 ended runs of a setting, that setting's comparison is inconclusive and says so.

## Runs

| run | variant | end | clean pass | cost USD | wall s | looping factor | note |
|---|---|---|---|---|---|---|---|
| build-constraints.haiku.1 | haiku | stopped: unstick_spent in IDEA-1.direction | no | 0.235 | 1176 | 1.429 |  |
| build-constraints.haiku.2 | haiku | closed: not_delivered | no | 1.261 | 3382 | 1.909 |  |
| build-constraints.haiku.3 | haiku | stopped: decision_not_takeable in IDEA-1.direction | no | 0.18 | 882 | 1 |  |
| build-constraints.mixed.1 | mixed | stopped: unstick_spent in IDEA-1.direction | no | 0.212 | 1171 | 1 |  |
| build-constraints.mixed.2 | mixed | closed: operating | yes | 2.9 | 2653 | 1.667 |  |
| build-constraints.mixed.3 | mixed | stopped: finding_reraised in IDEA-1.direction | no | 0.126 | 621 | 1.167 |  |

## Per variant

Median [min, max] (n) over the ended runs.

| | haiku | mixed |
|---|---|---|
| runs listed | 3 | 3 |
| ended | 3 | 3 |
| clean passes | 0 | 1 |
| `run.cost_usd` | 0.2351 [0.1804, 1.2611] (3) | 0.2124 [0.1263, 2.8998] (3) |
| `run.llm_calls` | 10 [5, 25] (3) | 7 [5, 23] (3) |
| `run.decider_calls` | 13 [12, 19] (3) | 15 [7, 18] (3) |
| `run.wall_s` | 1176 [882, 3382] (3) | 1171 [621, 2653] (3) |
| `run.driver_s` | 253 [152, 2223] (3) | 216 [191, 1575] (3) |
| `run.real_takes` | 51 [38, 143] (3) | 40 [38, 144] (3) |
| `run.rework_takes` | 30 [22, 80] (3) | 22 [22, 73] (3) |
| `run.looping_factor` | 1.429 [1, 1.909] (3) | 1.167 [1, 1.667] (3) |
| `run.first_pass_yield` | 0.714 [0.545, 1] (3) | 0.833 [0.667, 1] (3) |
| `run.rework_cost_usd` | 0.0198 [0, 0.5252] (3) | 0.0089 [0, 0.1998] (3) |
| `run.furthest_stage_index` | 2 [2, 7] (3) | 2 [2, 7] (3) |
| `run.findings_raised` | 6 [5, 19] (3) | 8 [5, 14] (3) |
| `run.waiting_on_person_s` | 0 [0, 0] (3) | 0 [0, 0] (3) |
| `run.clean` | 0 [0, 1] (3) | 0 [0, 1] (3) |

## Loops

Per variant, over the ended runs: of the writer and approval nodes taken again at least once, the three with the highest median looping factor; and the three edges with the highest loop cost.

- haiku (n 3):
  - `direction/vision.write` (llm-writer): looping factor median 4 (max 6, n 2), rework takes 6, rework cost 0.031 USD
  - `spec/spec.author` (llm-writer): looping factor median 3 (max 3, n 1), rework takes 2, rework cost 0.021 USD
  - `direction/roadmap.write` (llm-writer): looping factor median 2.5 (max 3, n 2), rework takes 3, rework cost 0.022 USD
  - `story: change.review:rejected -> change.implement` (check): count 2 in 1 runs, loop cost 0.482 USD, loop driver 789 s
  - `direction: roadmap.approve:rejected -> roadmap.write` (approval): count 1 in 1 runs, loop cost 0.132 USD, loop driver 51 s
  - `direction: finding@context -> intake` (finding): count 6 in 3 runs, loop cost 0.086 USD, loop driver 116 s
- mixed (n 3):
  - `direction/roadmap.write` (llm-writer): looping factor median 3 (max 3, n 1), rework takes 2, rework cost 0.015 USD
  - `direction/vision.approve` (approval): looping factor median 3 (max 3, n 1), rework takes 2, rework cost 0 USD
  - `direction/vision.write` (llm-writer): looping factor median 2.5 (max 4, n 2), rework takes 3, rework cost 0.014 USD
  - `spec: finding@spec -> intake` (finding): count 2 in 1 runs, loop cost 0.151 USD, loop driver 22 s
  - `direction: finding@roadmap -> intake` (finding): count 3 in 1 runs, loop cost 0.084 USD, loop driver 107 s
  - `direction: finding@vision -> intake` (finding): count 2 in 1 runs, loop cost 0.056 USD, loop driver 71 s

## Verdict

descriptive: no hypothesis: the record describes its runs and decides nothing.

## Facts

- The branch on 0.1.2 passes its cycles: "100 expected, 100 as expected, 0 failed" (head 3ca26fe, reported by the merging session).
- What a run does differently at 3ca26fe: work/build-constraints.yaml in every run, recorded in run-settings.jsonl (`build_constraints`); requirements.check refuses a requirement whose language or dependency contradicts it; the decider gets it on every call but the close, and an answer naming a `ruled_out` entry is refused as an invalid reply and asked again; a contradiction against an answer already resolved re-raises the finding.

## Links

- experiments/release-0.1.2.yaml: the control arm
- experiments/baseline-0.1.0/losses.md: haiku-1's q9 and the loss this change addresses (1.220 USD, 976 s)
- story:build-constraints-every-stage, story:default-answers-within-build-constraints
