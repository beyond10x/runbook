# trial-3-main-1: Trial 3 from main 8db4b93, the first runs with a review model

Written by `sdc-experiment report trial-3-main-1` from `experiments/trial-3-main-1.yaml` and the run summaries in `experiments/trial-3-main-1/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: retrospective; status: complete; created 2026-11-21.
- Purpose: Run the to-do idea from main with the review node (change.review, class review) in three model settings, the first runs in which a review model is called.
- Hypothesis: none; the record describes its runs.
- Input `todo-app` (idea, sha256 f384a752a2cb): A to-do list web app for me and my partner that also works on the train without a connection
- Tree: `8db4b9338f1d8d32c85b9cb2e014dac56980fac0`
  - trial/main-1 at main 8db4b93, worktree sdc-trial-main-1; driver, watch and goals built there before the runs
- Runs per variant: 2; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (descriptive): No hypothesis. The record describes the runs; one or two runs per setting establish no difference between settings.

## Settings

| setting | strong | mixed | haiku |
|---|---|---|---|
| `adapter` | metaharness | metaharness | metaharness |
| `adapters.review.adapter` | claude-cli | claude-cli | - |
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

- `adapters.review.adapter`: recorded as claude-cli in the runs with a review model; trial-3-haiku-8 has no review model and records no review adapter

## Runs

| run | variant | end | clean pass | cost USD | wall s | looping factor | note |
|---|---|---|---|---|---|---|---|
| trial-3-strong-3 | strong | stopped: review_invalid in IDEA-1.todo.access | no | 1.828 | 1668 | 1.1 |  |
| trial-3-strong-4 | strong | stopped: review_invalid in IDEA-1.todo.list | no | 1.697 | 1668 | 1 |  |
| trial-3-mixed-7 | mixed | stopped: review_invalid in IDEA-1.todo.access | no | 0.709 | 1670 | 1.3 |  |
| trial-3-haiku-8 | haiku | stopped: writer_spent in IDEA-1.list.access | no | 0.346 | 1790 | 1.6 |  |

## Per variant

Median [min, max] (n) over the ended runs.

| | strong | mixed | haiku |
|---|---|---|---|
| runs listed | 2 | 1 | 1 |
| ended | 2 | 1 | 1 |
| clean passes | 0 | 0 | 0 |
| `run.cost_usd` | 1.7627 [1.6974, 1.828] (2) | 0.7095 [0.7095, 0.7095] (1) | 0.3463 [0.3463, 0.3463] (1) |
| `run.llm_calls` | 11.5 [11, 12] (2) | 14 [14, 14] (1) | 16 [16, 16] (1) |
| `run.decider_calls` | 6 [6, 6] (2) | 8 [8, 8] (1) | 11 [11, 11] (1) |
| `run.wall_s` | 1668 [1668, 1668] (2) | 1670 [1670, 1670] (1) | 1790 [1790, 1790] (1) |
| `run.driver_s` | 260 [260, 260] (2) | 351 [351, 351] (1) | 1266 [1266, 1266] (1) |
| `run.real_takes` | 66 [66, 66] (2) | 84 [84, 84] (1) | 115 [115, 115] (1) |
| `run.rework_takes` | 17.5 [17, 18] (2) | 36 [36, 36] (1) | 66 [66, 66] (1) |
| `run.looping_factor` | 1.05 [1, 1.1] (2) | 1.3 [1.3, 1.3] (1) | 1.6 [1.6, 1.6] (1) |
| `run.first_pass_yield` | 0.95 [0.9, 1] (2) | 0.8 [0.8, 0.8] (1) | 0.8 [0.8, 0.8] (1) |
| `run.rework_cost_usd` | 0.0227 [0, 0.0454] (2) | 0.1775 [0.1775, 0.1775] (1) | 0.1155 [0.1155, 0.1155] (1) |
| `run.furthest_stage_index` | 6 [6, 6] (2) | 6 [6, 6] (1) | 6 [6, 6] (1) |
| `run.findings_raised` | 1 [1, 1] (2) | 3 [3, 3] (1) | 12 [12, 12] (1) |
| `run.waiting_on_person_s` | 1318.5 [1318, 1319] (2) | 1027 [1027, 1027] (1) | 0 [0, 0] (1) |
| `run.clean` | 0 [0, 0] (2) | 0 [0, 0] (1) | 0 [0, 0] (1) |

## Loops

Per variant, over the ended runs: of the writer and approval nodes taken again at least once, the three with the highest median looping factor; and the three edges with the highest loop cost.

- strong (n 2):
  - `direction/vision.write` (llm-writer): looping factor median 1.5 (max 2, n 2), rework takes 1, rework cost 0.045 USD
  - `direction: finding@vision -> intake` (finding): count 1 in 1 runs, loop cost 0.521 USD, loop driver 23 s
  - `story: finding@story -> intake` (finding): count 1 in 1 runs, loop cost 0 USD, loop driver 0 s
- mixed (n 1):
  - `spec/spec.author` (llm-writer): looping factor median 3 (max 3, n 1), rework takes 2, rework cost 0.175 USD
  - `direction/vision.write` (llm-writer): looping factor median 2 (max 2, n 1), rework takes 1, rework cost 0.003 USD
  - `spec: spec.synthesize:refused -> spec.author` (check): count 1 in 1 runs, loop cost 0.1 USD, loop driver 18 s
  - `spec: spec.validate:invalid -> spec.author` (check): count 1 in 1 runs, loop cost 0.075 USD, loop driver 16 s
  - `direction: finding@vision -> intake` (finding): count 1 in 1 runs, loop cost 0.03 USD, loop driver 21 s
- haiku (n 1):
  - `story/change.implement` (llm-writer): looping factor median 6 (max 6, n 1), rework takes 5, rework cost 0.094 USD
  - `spec/spec.author` (llm-writer): looping factor median 2 (max 2, n 1), rework takes 1, rework cost 0.021 USD
  - `story: finding@story -> intake` (finding): count 4 in 1 runs, loop cost 0.088 USD, loop driver 313 s
  - `story: tests.run:failed -> change.implement` (check): count 3 in 1 runs, loop cost 0.046 USD, loop driver 460 s
  - `spec: finding@spec -> intake` (finding): count 5 in 1 runs, loop cost 0.021 USD, loop driver 79 s

## Verdict

descriptive: no hypothesis: the record describes its runs and decides nothing.

## Facts

- strong-3, strong-4 and mixed-7 stopped at their first change.review call: the review call (Sonnet, read-only operations) exited 0 and the check refused it, "invalid review reply: no readable reply" at out/review.json after 3 attempts; the review model could not write out/review.json. No story of these runs was merged.
- The fix was in work on branch fix/review-reply; trial-3-main-2 is its rerun.
- trial-3-haiku-8 (all Haiku, no review model) stopped writer_spent in IDEA-1.list.access.
- Cost before the stop (docs/trial-3.md section 9): strong-3 1.828 USD in 350 s, strong-4 1.697 USD in 349 s, mixed-7 0.709 USD in 643 s.

## Links

- docs/trial-3.md on branch docs/trial-3 (written at 243a22f, not on main when this record was written): section 9
