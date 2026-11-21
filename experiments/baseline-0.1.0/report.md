# baseline-0.1.0: Baseline of release 0.1.0 on main 4468bfd, the to-do idea in three model settings

Written by `sdc-experiment report baseline-0.1.0` from `experiments/baseline-0.1.0.yaml` and the run summaries in `experiments/baseline-0.1.0/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: prospective; status: complete; created 2026-11-21.
- Purpose: Baseline of release 0.1.0; first runs on the hermetic decider with explicit efforts.
- Hypothesis: none; the record describes its runs.
- Input `todo-app` (idea, sha256 f384a752a2cb): A to-do list web app for me and my partner that also works on the train without a connection
- Tree: `4468bfd60e9665c17d57d7ffbbc8e7811d2b4a84`
  - trial/0.1.0 at 4468bfd (main, no commits), worktree sdc-trial-0-1-0; driver, watch and goals built once before the first start, the file-change recorder (watch release build) once at 12:48Z, never rebuilt under a running run
- Runs per variant: 3; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (descriptive): No hypothesis. Three runs per setting describe the release tree; every end counts, no run is repaired or reopened.

## Settings

| setting | strong | mixed | haiku |
|---|---|---|---|
| `adapter` | metaharness | metaharness | metaharness |
| `autonomy` | A5 | A5 | A5 |
| `decider.adapter` | claude-cli | claude-cli | claude-cli |
| `decider.escalate` | final | final | final |
| `decider.kind` | haiku | haiku | haiku |
| `decider.max_decider_calls` | 40 | 40 | 40 |
| `decider.model` | claude-opus-5-5 | claude-haiku-5-5 | claude-haiku-5-5 |
| `max_llm_calls` | 150 | 150 | 150 |
| `max_wall_seconds` | 5400 | 5400 | 5400 |
| `models.implement.model` | claude-sonnet-5-5 | claude-sonnet-5-5 | claude-haiku-5-5 |
| `models.other.model` | claude-sonnet-5-5 | claude-haiku-5-5 | claude-haiku-5-5 |
| `models.review.model` | claude-sonnet-5-5 | claude-sonnet-5-5 | claude-haiku-5-5 |
| `models.writer.model` | claude-sonnet-5-5 | claude-sonnet-5-5 | claude-haiku-5-5 |
| `review.adapter` | claude-cli | claude-cli | claude-cli |

Known confounds:

- `canon`: every run used the canon binary of the canon checkout at run time (run-trial.sh's default), which no record pins
- `launcher`: the runs were started by a scheduler script that copies the launcher's environment (env -i, the record's settings, no SDC_LLM_EFFORT variable, so the default efforts), not by sdc-experiment run, so they have no experiment.json; the first scheduler counted its own unit and ran two runs at once until 12:48Z
- `recorder`: trial-0.1.0-strong-1 and trial-0.1.0-mixed-1 waited about 2 minutes for the pool before their first event (the recorder's release build); the wall times in the report start at the first event and do not contain it

## Runs

| run | variant | end | clean pass | cost USD | wall s | looping factor | note |
|---|---|---|---|---|---|---|---|
| trial-0.1.0-strong-1 | strong | closed: operating | yes | 5.432 | 1075 | 1.417 |  |
| trial-0.1.0-strong-2 | strong | closed: operating | yes | 6.194 | 1247 | 1.455 |  |
| trial-0.1.0-strong-3 | strong | closed: operating | yes | 5.503 | 1430 | 1.333 |  |
| trial-0.1.0-mixed-1 | mixed | closed: operating | yes | 2.497 | 1480 | 1.167 |  |
| trial-0.1.0-mixed-2 | mixed | closed: operating | yes | 3.883 | 2091 | 1.333 |  |
| trial-0.1.0-mixed-3 | mixed | closed: operating | yes | 2.549 | 1489 | 1.417 |  |
| trial-0.1.0-haiku-1 | haiku | stopped: writer_spent in IDEA-1.list.shared | no | 2.153 | 3790 | 1.833 | writer_spent in IDEA-1.list.shared |
| trial-0.1.0-haiku-2 | haiku | stopped: decision_not_takeable in IDEA-1.list.add | no | 0.205 | 938 | 1.1 | decision_not_takeable in IDEA-1.list.add |
| trial-0.1.0-haiku-3 | haiku | stopped: writer_spent in IDEA-1.todo.spec | no | 0.283 | 1213 | 1.75 | writer_spent in IDEA-1.todo.spec |

## Per variant

Median [min, max] (n) over the ended runs.

| | strong | mixed | haiku |
|---|---|---|---|
| runs listed | 3 | 3 | 3 |
| ended | 3 | 3 | 3 |
| clean passes | 3 | 3 | 0 |
| `run.cost_usd` | 5.5029 [5.432, 6.1937] (3) | 2.5488 [2.4968, 3.883] (3) | 0.2829 [0.2051, 2.1534] (3) |
| `run.llm_calls` | 21 [20, 21] (3) | 20 [18, 21] (3) | 14 [11, 28] (3) |
| `run.decider_calls` | 13 [13, 13] (3) | 12 [11, 15] (3) | 12 [9, 19] (3) |
| `run.wall_s` | 1247 [1075, 1430] (3) | 1489 [1480, 2091] (3) | 1213 [938, 3790] (3) |
| `run.driver_s` | 968 [788, 1168] (3) | 840 [741, 1107] (3) | 748 [592, 2792] (3) |
| `run.real_takes` | 121 [104, 127] (3) | 110 [97, 113] (3) | 92 [88, 151] (3) |
| `run.rework_takes` | 56 [34, 57] (3) | 40 [28, 43] (3) | 57 [39, 87] (3) |
| `run.looping_factor` | 1.417 [1.333, 1.455] (3) | 1.333 [1.167, 1.417] (3) | 1.75 [1.1, 1.833] (3) |
| `run.first_pass_yield` | 0.75 [0.727, 0.75] (3) | 0.75 [0.75, 0.833] (3) | 0.75 [0.583, 0.9] (3) |
| `run.rework_cost_usd` | 1.1438 [1.0674, 2.0766] (3) | 0.53 [0.5195, 1.7266] (3) | 0.1137 [0.0036, 1.3808] (3) |
| `run.furthest_stage_index` | 7 [7, 7] (3) | 7 [7, 7] (3) | 6 [4, 6] (3) |
| `run.findings_raised` | 7 [5, 8] (3) | 8 [6, 12] (3) | 11 [10, 26] (3) |
| `run.waiting_on_person_s` | 0 [0, 0] (3) | 0 [0, 0] (3) | 0 [0, 0] (3) |
| `run.clean` | 1 [1, 1] (3) | 1 [1, 1] (3) | 0 [0, 0] (3) |

## Loops

Per variant, over the ended runs: of the writer and approval nodes taken again at least once, the three with the highest median looping factor; and the three edges with the highest loop cost.

- strong (n 3):
  - `spec/spec.author` (llm-writer): looping factor median 3 (max 3, n 3), rework takes 5, rework cost 0.381 USD
  - `story/change.implement` (llm-writer): looping factor median 1.667 (max 2.5, n 3), rework takes 7, rework cost 2.951 USD
  - `direction/vision.write` (llm-writer): looping factor median 1 (max 2, n 3), rework takes 1, rework cost 0.048 USD
  - `story: change.review:rejected -> change.implement` (check): count 5 in 3 runs, loop cost 3.271 USD, loop driver 782 s
  - `story: finding@story -> intake` (finding): count 11 in 3 runs, loop cost 1.033 USD, loop driver 106 s
  - `spec: finding@spec -> intake` (finding): count 4 in 2 runs, loop cost 0.424 USD, loop driver 42 s
- mixed (n 3):
  - `direction/vision.write` (llm-writer): looping factor median 2 (max 4, n 3), rework takes 5, rework cost 0.013 USD
  - `story/change.implement` (llm-writer): looping factor median 1.333 (max 1.333, n 3), rework takes 3, rework cost 2.109 USD
  - `spec/spec.author` (llm-writer): looping factor median 1 (max 3, n 3), rework takes 2, rework cost 0.147 USD
  - `story: merge.approve:rejected -> change.implement` (approval): count 1 in 1 runs, loop cost 1.621 USD, loop driver 471 s
  - `story: change.review:rejected -> change.implement` (check): count 2 in 2 runs, loop cost 1.037 USD, loop driver 266 s
  - `spec: spec.validate:invalid -> spec.author` (check): count 2 in 1 runs, loop cost 0.147 USD, loop driver 48 s
- haiku (n 3):
  - `spec/spec.author` (llm-writer): looping factor median 5 (max 6, n 3), rework takes 9, rework cost 0.177 USD
  - `direction/vision.write` (llm-writer): looping factor median 2 (max 2, n 3), rework takes 3, rework cost 0.012 USD
  - `story/change.implement` (llm-writer): looping factor median 1.8335 (max 2.667, n 2), rework takes 5, rework cost 1.268 USD
  - `story: change.review:rejected -> change.implement` (check): count 3 in 1 runs, loop cost 0.944 USD, loop driver 826 s
  - `story: tests.run:failed -> change.implement` (check): count 1 in 1 runs, loop cost 0.319 USD, loop driver 243 s
  - `story: merge.approve:rejected -> change.implement` (approval): count 1 in 1 runs, loop cost 0.113 USD, loop driver 154 s

## Verdict

descriptive: no hypothesis: the record describes its runs and decides nothing.

## Facts

- All six Sonnet runs (strong and mixed, 3 each) closed operating by rule:autopilot, exit 0, with no stop record, reopen, forced or unstuck step; replay matched in every one and no run waited on a person.
- All three all-Haiku runs stopped: trial-0.1.0-haiku-1 writer_spent in IDEA-1.list.shared (three implement runs sent back by a gate finding about the Postgres store of an accepted default, a review rejection and a gate finding about an ambiguous title edit); trial-0.1.0-haiku-2 decision_not_takeable in IDEA-1.list.add (the decider's amendment made context.deployment and context.platform contradict r6); trial-0.1.0-haiku-3 writer_spent in IDEA-1.todo.spec (four spec.author runs refused by spec.validate for a missing format version the feedback named).
- change.review ran on claude-cli with a response schema in every run, and the review rejected at least once in five of the six Sonnet runs; merge.approve rejected once in trial-0.1.0-mixed-2 and once in trial-0.1.0-haiku-1, both reworked and approved.
- Cost counts llm-calls.jsonl plus every decisions.jsonl line, suppressed escalations included; sdc-run-compare of this tree leaves the suppressed lines out.

## Links

- experiments/trial-3-main-2.yaml: the runs of the tree before 0.1.0
