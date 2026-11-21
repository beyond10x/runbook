# trial-3-batch-1: Trial 3 batch on acd6f12, five model settings

Written by `sdc-experiment report trial-3-batch-1` from `experiments/trial-3-batch-1.yaml` and the run summaries in `experiments/trial-3-batch-1/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: retrospective; status: complete; created 2026-11-21.
- Purpose: Run the to-do idea from idea to close with no person on one tree in five model settings, to see which settings pass and where each one loops.
- Hypothesis: none; the record describes its runs.
- Input `todo-app` (idea, sha256 f384a752a2cb): A to-do list web app for me and my partner that also works on the train without a connection
- Tree: `acd6f12c4924e7a552d0ab518287d1c873618915`
  - trial/3b at acd6f12 (ea2f111 + fix/attempt5-intake 102ce99 + fix/attempt5-close 83d3cbf), worktree sdc-trial-3b, built before the batch started; the review step was a program stub on this tree
  - trial-3-mixed-5 started on 3663770 (acd6f12 without 83d3cbf); its binaries were rebuilt in place to acd6f12 during its round 1, so it ran on two trees
- Runs per variant: 2; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (descriptive): No hypothesis. One or two runs per setting establish no difference between settings; the record describes the runs.

## Settings

| setting | haiku | mixed | sonnet | strong | opus |
|---|---|---|---|---|---|
| `adapter` | metaharness | metaharness | metaharness | metaharness | metaharness |
| `autonomy` | A5 | A5 | A5 | A5 | A5 |
| `decider.adapter` | claude-cli | claude-cli | claude-cli | claude-cli | claude-cli |
| `decider.escalate` | final | final | final | final | final |
| `decider.kind` | haiku | haiku | haiku | haiku | haiku |
| `decider.max_decider_calls` | 20 | 20 | 20 | 20 | 20 |
| `decider.model` | claude-haiku-5-5 | claude-haiku-5-5 | claude-haiku-5-5 | claude-opus-5-5 | claude-haiku-5-5 |
| `max_llm_calls` | 150 | - | 150 | 150 | 150 |
| `max_wall_seconds` | 5400 | 5400 | 5400 | 5400 | 5400 |
| `models.implement.model` | claude-haiku-5-5 | claude-sonnet-5-5 | claude-sonnet-5-5 | claude-sonnet-5-5 | claude-opus-5-5 |
| `models.other.model` | claude-haiku-5-5 | claude-haiku-5-5 | claude-sonnet-5-5 | claude-sonnet-5-5 | claude-opus-5-5 |
| `models.review.model` | null | claude-sonnet-5-5 | claude-sonnet-5-5 | claude-sonnet-5-5 | claude-opus-5-5 |
| `models.writer.model` | claude-haiku-5-5 | claude-sonnet-5-5 | claude-sonnet-5-5 | claude-sonnet-5-5 | claude-opus-5-5 |

Known confounds:

- `max_llm_calls`: trial-3-mixed-5 ran with 90, trial-3-mixed-6 with 150
- `canon`: every run used the canon binary of the canon checkout at run time (run-trial.sh's default), which no record pins

## Runs

| run | variant | tree | end | clean pass | cost USD | wall s | looping factor | note |
|---|---|---|---|---|---|---|---|---|
| trial-3-haiku-6 | haiku | - | closed: not_delivered | no | 0.56 | 1916 | 1.417 |  |
| trial-3-haiku-7 | haiku | - | closed: operating | yes | 1.034 | 2709 | 1.417 |  |
| trial-3-mixed-5 | mixed | 3663770 | closed: not_delivered | no | 1.588 | 1692 | 1.364 | max_llm_calls 90; rebuilt in place to acd6f12 in round 1 |
| trial-3-mixed-6 | mixed | - | closed: operating | yes | 1.859 | 2295 | 1.333 |  |
| trial-3-sonnet-1 | sonnet | - | stopped: decider_budget_spent in IDEA-1.todo.access | no | 1.882 | 1444 | 2.4 |  |
| trial-3-strong-1 | strong | - | closed: operating | yes | 2.926 | 473 | 1 |  |
| trial-3-strong-2 | strong | - | closed: operating | yes | 2.996 | 454 | 1 |  |
| trial-3-opus-1 | opus | - | closed: operating | yes | 5.088 | 1662 | 1 |  |

## Per variant

Median [min, max] (n) over the ended runs.

| | haiku | mixed | sonnet | strong | opus |
|---|---|---|---|---|---|
| runs listed | 2 | 2 | 1 | 2 | 1 |
| ended | 2 | 2 | 1 | 2 | 1 |
| clean passes | 1 | 1 | 0 | 2 | 1 |
| `run.cost_usd` | 0.7967 [0.5597, 1.0337] (2) | 1.7233 [1.5879, 1.8586] (2) | 1.8821 [1.8821, 1.8821] (1) | 2.9612 [2.9259, 2.9964] (2) | 5.088 [5.088, 5.088] (1) |
| `run.llm_calls` | 17 [17, 17] (2) | 15.5 [15, 16] (2) | 24 [24, 24] (1) | 12 [12, 12] (2) | 12 [12, 12] (1) |
| `run.decider_calls` | 15.5 [14, 17] (2) | 14 [14, 14] (2) | 20 [20, 20] (1) | 11 [11, 11] (2) | 11 [11, 11] (1) |
| `run.wall_s` | 2312.5 [1916, 2709] (2) | 1993.5 [1692, 2295] (2) | 1444 [1444, 1444] (1) | 463.5 [454, 473] (2) | 1662 [1662, 1662] (1) |
| `run.driver_s` | 1209 [983, 1435] (2) | 626.5 [478, 775] (2) | 399 [399, 399] (1) | 292 [278, 306] (2) | 989 [989, 989] (1) |
| `run.real_takes` | 127.5 [127, 128] (2) | 111.5 [96, 127] (2) | 205 [205, 205] (1) | 84.5 [82, 87] (2) | 100 [100, 100] (1) |
| `run.rework_takes` | 56.5 [56, 57] (2) | 45.5 [34, 57] (2) | 155 [155, 155] (1) | 16 [14, 18] (2) | 28 [28, 28] (1) |
| `run.looping_factor` | 1.417 [1.417, 1.417] (2) | 1.3485 [1.333, 1.364] (2) | 2.4 [2.4, 2.4] (1) | 1 [1, 1] (2) | 1 [1, 1] (1) |
| `run.first_pass_yield` | 0.667 [0.667, 0.667] (2) | 0.784 [0.75, 0.818] (2) | 0.9 [0.9, 0.9] (1) | 1 [1, 1] (2) | 1 [1, 1] (1) |
| `run.rework_cost_usd` | 0.1793 [0.0377, 0.3208] (2) | 0.6439 [0.6072, 0.6806] (2) | 0.9153 [0.9153, 0.9153] (1) | 0 [0, 0] (2) | 0 [0, 0] (1) |
| `run.furthest_stage_index` | 7 [7, 7] (2) | 7 [7, 7] (2) | 6 [6, 6] (1) | 7 [7, 7] (2) | 7 [7, 7] (1) |
| `run.findings_raised` | 18.5 [18, 19] (2) | 6 [5, 7] (2) | 3 [3, 3] (1) | 1 [0, 2] (2) | 9 [9, 9] (1) |
| `run.waiting_on_person_s` | 0 [0, 0] (2) | 0 [0, 0] (2) | 0 [0, 0] (1) | 0 [0, 0] (2) | 0 [0, 0] (1) |
| `run.clean` | 1 [1, 1] (2) | 1 [1, 1] (2) | 0 [0, 0] (1) | 1 [1, 1] (2) | 1 [1, 1] (1) |

## Loops

Per variant, over the ended runs: of the writer and approval nodes taken again at least once, the three with the highest median looping factor; and the three edges with the highest loop cost.

- haiku (n 2):
  - `direction/vision.write` (llm-writer): looping factor median 2.5 (max 3, n 2), rework takes 3, rework cost 0.009 USD
  - `direction/roadmap.write` (llm-writer): looping factor median 2 (max 2, n 2), rework takes 2, rework cost 0.007 USD
  - `direction/vision.approve` (approval): looping factor median 2 (max 2, n 2), rework takes 2, rework cost 0 USD
  - `story: merge.approve:rejected -> change.implement` (approval): count 1 in 1 runs, loop cost 0.251 USD, loop driver 232 s
  - `story: finding@story -> intake` (finding): count 24 in 2 runs, loop cost 0.148 USD, loop driver 217 s
  - `direction: finding@roadmap -> intake` (finding): count 3 in 2 runs, loop cost 0.1 USD, loop driver 65 s
- mixed (n 2):
  - `story/change.implement` (llm-writer): looping factor median 2.0835 (max 2.5, n 2), rework takes 5, rework cost 1.21 USD
  - `direction/vision.write` (llm-writer): looping factor median 2 (max 2, n 2), rework takes 2, rework cost 0.006 USD
  - `story/merge.approve` (approval): looping factor median 1.8335 (max 2, n 2), rework takes 4, rework cost 0 USD
  - `story: merge.approve:rejected -> change.implement` (approval): count 4 in 2 runs, loop cost 1.365 USD, loop driver 385 s
  - `story: finding@story -> intake` (finding): count 4 in 2 runs, loop cost 0.194 USD, loop driver 31 s
  - `spec: finding@spec -> intake` (finding): count 2 in 1 runs, loop cost 0.086 USD, loop driver 16 s
- sonnet (n 1):
  - `spec/spec.author` (llm-writer): looping factor median 15 (max 15, n 1), rework takes 14, rework cost 0.915 USD
  - `spec: finding@spec -> intake` (finding): count 2 in 1 runs, loop cost 0.964 USD, loop driver 192 s
  - `story: finding@story -> intake` (finding): count 1 in 1 runs, loop cost 0 USD, loop driver 0 s
- strong (n 2):
  - `intake: finding@requirements -> intake` (finding): count 1 in 1 runs, loop cost 0 USD, loop driver 0 s
  - `story: finding@story -> intake` (finding): count 1 in 1 runs, loop cost 0 USD, loop driver 0 s
- opus (n 1):
  - `story: finding@story -> intake` (finding): count 7 in 1 runs, loop cost 0 USD, loop driver 0 s
  - `intake: finding@requirements -> intake` (finding): count 1 in 1 runs, loop cost 0 USD, loop driver 0 s
  - `spec: finding@spec -> intake` (finding): count 1 in 1 runs, loop cost 0 USD, loop driver 0 s

## Verdict

descriptive: no hypothesis: the record describes its runs and decides nothing.

## Facts

- Pass by the trial-3 report's rule (closed operating by itself, no stop record, no reopen, no forced step, replay matching, 0 s waiting): 5 of the 8 runs. strong 2 of 2, haiku 1 of 2, mixed 1 of 2, opus 1 of 1, sonnet 0 of 1.
- Cost and wall time of the passes (docs/trial-3.md): strong-1 2.93 USD, 473 s; strong-2 3.00 USD, 454 s; haiku-7 1.00 USD, 2709 s; mixed-6 1.86 USD, 2295 s; opus-1 5.09 USD, 1662 s.
- trial-3-haiku-6 closed not_delivered: the decider withheld the release because the SharedList view and the device-side offline page were not built and sync had no HTTP route (r1, r2, r4, r5 not met); right, as the release README says. Its merge approvals had let thin stories through (Haiku decider, 1 reject of 14).
- trial-3-mixed-5 closed not_delivered: the decider withheld the release because GET /api/items answers without a token while vision and roadmap make the list link the only way in; right about the code, and q7 itself conflicts with the vision.
- trial-3-sonnet-1 stopped decider_budget_spent in IDEA-1.todo.access: 14 of its 20 decider calls were the trace gap p-trace-r7 on one finding, whose constraint answer was not recorded, so the gap returned; spec.author ran 15 times (fix: fix/superseded-answers b4aa0d1, not merged).
- No review model made a call in any batch run (the review was a program stub on acd6f12), and no oracle check ran on any trial-3 product: the passes are Canon outcomes closed by the program rule, not products seen working.
- The decider principal reads decider:haiku whatever model decided (strong-1 decided with claude-opus-5-5).

## Links

- docs/trial-3.md on branch docs/trial-3 (commit 243a22f; not on main when this record was written): sections 1, 2, 3, 5, 6 and 8
