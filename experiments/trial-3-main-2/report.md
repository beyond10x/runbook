# trial-3-main-2: Trial 3 from main 8db4b93 with fix/review-reply, the first passes with a review model

Written by `sdc-experiment report trial-3-main-2` from `experiments/trial-3-main-2.yaml` and the run summaries in `experiments/trial-3-main-2/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: retrospective; status: running; created 2026-11-21.
- Purpose: Rerun trial-3-main-1's settings on main with the review reply fix, to see whether runs pass with a real review model.
- Hypothesis: none; the record describes its runs.
- Input `todo-app` (idea, sha256 f384a752a2cb): A to-do list web app for me and my partner that also works on the train without a connection
- Tree: `1001970d83d872d1d0dcb80372cd22a8115c9359`
  - trial/main-2 at 1001970 = main 8db4b93 + fix/review-reply 49b0a87 (bot merge commit), worktree sdc-trial-main-2; driver, watch and goals built there before the runs
  - the same tree served the reopen legs of trial-3-strong-3, trial-3-strong-4 and trial-3-mixed-7, which belong to trial-3-main-1
- Runs per variant: 2; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (descriptive): No hypothesis. One or two runs per setting establish no difference between settings; the record describes the runs.

## Settings

| setting | strong | mixed | haiku |
|---|---|---|---|
| `adapter` | metaharness | metaharness | metaharness |
| `adapters.review.adapter` | claude-cli | claude-cli | claude-cli |
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
| `models.review.model` | claude-sonnet-5-5 | claude-sonnet-5-5 | null |
| `models.writer.model` | claude-sonnet-5-5 | claude-sonnet-5-5 | claude-haiku-5-5 |

Known confounds:

- `canon`: every run used the canon binary of the canon checkout at run time (run-trial.sh's default), which no record pins

## Runs

| run | variant | end | clean pass | cost USD | wall s | looping factor | note |
|---|---|---|---|---|---|---|---|
| trial-3-strong-5 | strong | closed: operating | yes | 5.338 | 967 | 1.417 | 7 change.review calls |
| trial-3-strong-6 | strong | closed: operating | yes | 4.055 | 677 | 1.333 | 5 change.review calls |
| trial-3-mixed-8 | mixed | stopped: rework_spent in IDEA-1.todo.access | no | 1.272 | 1030 | 1.3 |  |
| trial-3-haiku-9 | haiku | stopped: writer_spent in IDEA-1.list.tasks | no | 0.878 | 3446 | 1.417 | ended 2026-11-21T12:31:35Z, stopped writer_spent at change.implement in IDEA-1.list.tasks (3 runs); not reopened |

## Per variant

Median [min, max] (n) over the ended runs.

| | strong | mixed | haiku |
|---|---|---|---|
| runs listed | 2 | 1 | 1 |
| ended | 2 | 1 | 1 |
| clean passes | 2 | 0 | 0 |
| `run.cost_usd` | 4.6963 [4.0549, 5.3377] (2) | 1.2719 [1.2719, 1.2719] (1) | 0.8776 [0.8776, 0.8776] (1) |
| `run.llm_calls` | 22.5 [21, 24] (2) | 16 [16, 16] (1) | 22 [22, 22] (1) |
| `run.decider_calls` | 11 [11, 11] (2) | 10 [10, 10] (1) | 17 [17, 17] (1) |
| `run.wall_s` | 822 [677, 967] (2) | 1030 [1030, 1030] (1) | 3446 [3446, 3446] (1) |
| `run.driver_s` | 651 [514, 788] (2) | 394 [394, 394] (1) | 2198 [2198, 2198] (1) |
| `run.real_takes` | 98.5 [97, 100] (2) | 80 [80, 80] (1) | 117 [117, 117] (1) |
| `run.rework_takes` | 29.5 [28, 31] (2) | 30 [30, 30] (1) | 52 [52, 52] (1) |
| `run.looping_factor` | 1.375 [1.333, 1.417] (2) | 1.3 [1.3, 1.3] (1) | 1.417 [1.417, 1.417] (1) |
| `run.first_pass_yield` | 0.667 [0.667, 0.667] (2) | 0.9 [0.9, 0.9] (1) | 0.75 [0.75, 0.75] (1) |
| `run.rework_cost_usd` | 0.9953 [0.4839, 1.5066] (2) | 0.635 [0.635, 0.635] (1) | 0.2949 [0.2949, 0.2949] (1) |
| `run.furthest_stage_index` | 7 [7, 7] (2) | 6 [6, 6] (1) | 6 [6, 6] (1) |
| `run.findings_raised` | 2.5 [2, 3] (2) | 4 [4, 4] (1) | 25 [25, 25] (1) |
| `run.waiting_on_person_s` | 0 [0, 0] (2) | 0 [0, 0] (1) | 0 [0, 0] (1) |
| `run.clean` | 1 [1, 1] (2) | 0 [0, 0] (1) | 0 [0, 0] (1) |

## Loops

Per variant, over the ended runs: of the writer and approval nodes taken again at least once, the three with the highest median looping factor; and the three edges with the highest loop cost.

- strong (n 2):
  - `story/change.implement` (llm-writer): looping factor median 2 (max 2.333, n 2), rework takes 6, rework cost 1.101 USD
  - `direction/vision.write` (llm-writer): looping factor median 2 (max 2, n 2), rework takes 2, rework cost 0.041 USD
  - `spec/spec.author` (llm-writer): looping factor median 1.5 (max 2, n 2), rework takes 1, rework cost 0 USD
  - `story: change.review:rejected -> change.implement` (check): count 6 in 2 runs, loop cost 1.95 USD, loop driver 481 s
  - `direction: finding@vision -> intake` (finding): count 2 in 2 runs, loop cost 1.025 USD, loop driver 47 s
  - `story: finding@story -> intake` (finding): count 3 in 2 runs, loop cost 0 USD, loop driver 0 s
- mixed (n 1):
  - `story/change.implement` (llm-writer): looping factor median 4 (max 4, n 1), rework takes 3, rework cost 0.449 USD
  - `story/merge.approve` (approval): looping factor median 3 (max 3, n 1), rework takes 2, rework cost 0 USD
  - `story: merge.approve:rejected -> change.implement` (approval): count 2 in 1 runs, loop cost 0.465 USD, loop driver 105 s
  - `story: finding@story -> intake` (finding): count 2 in 1 runs, loop cost 0.162 USD, loop driver 65 s
  - `direction: finding@context -> intake` (finding): count 2 in 1 runs, loop cost 0 USD, loop driver 0 s
- haiku (n 1):
  - `story/change.implement` (llm-writer): looping factor median 2.667 (max 2.667, n 1), rework takes 5, rework cost 0.235 USD
  - `story: finding@story -> intake` (finding): count 20 in 1 runs, loop cost 0.198 USD, loop driver 371 s
  - `story: merge.approve:rejected -> change.implement` (approval): count 1 in 1 runs, loop cost 0.105 USD, loop driver 218 s
  - `story: change.review:rejected -> change.implement` (check): count 2 in 1 runs, loop cost 0.049 USD, loop driver 159 s

## Verdict

descriptive: no hypothesis: the record describes its runs and decides nothing.

## Facts

- Pass by the trial-3 report's rule: trial-3-strong-5 and trial-3-strong-6, both closed operating by rule:autopilot, replay 10 of 10, 0 s waiting on a person; the first passes in which a review model was called.
- trial-3-strong-6: change.review 5 calls (Sonnet, 0.748 USD): offline rejected (r3 no local copy, edits not shown, sync after a partial failure) then approved; task rejected (the UI never calls /api/tasks, r1, r5) then approved; sharing approved. merge.approve approved all three stories, no merge rejection.
- trial-3-strong-5: change.review 7 calls (1.149 USD): offline rejected twice (no last-edit-wins; the page queues Add only) then approved; sharing rejected once (offline edits never reach a shared list) then approved; task rejected once (the page never calls the task API) then approved. merge.approve approved all three.
- trial-3-mixed-8: stopped rework_spent in IDEA-1.todo.access at merge.approve: decider:haiku rejected the change 3 times (r1 local-only and one list; r5 hosted managed database against a local data file; q9 defers the database client to no later story) while change.review approved all 3 calls. Not reopened.
- The reopens of trial-3-strong-3, trial-3-strong-4 and trial-3-mixed-7 on this tree stopped again at once with review_invalid and no new call: the driver listed change.review as admissible but not useful, so the review was not called again with the new adapter. They are listed in trial-3-main-1 as reopened runs.
- run-settings.jsonl of every real start records the review adapter claude-cli with a response schema.

## Links

- experiments/trial-3-main-1.yaml: the first legs of the three reopened runs
- docs/trial-3.md on branch docs/trial-3 (243a22f and later): section 9
