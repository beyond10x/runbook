# decider-haiku-vs-opus: Decider model Haiku against Opus with the work on Sonnet

Written by `sdc-experiment report decider-haiku-vs-opus` from `experiments/decider-haiku-vs-opus.yaml` and the run summaries in `experiments/decider-haiku-vs-opus/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: retrospective; status: complete; created 2026-11-21.
- Purpose: Whether the decider model changes how often a rejected merge sends a story back to the implementer, read from the trial-3 runs that exist.
- Hypothesis: With claude-opus-5-5 instead of claude-haiku-5-5 as the decider and the work on claude-sonnet-5-5, the edge story: merge.approve:rejected -> change.implement occurs at least 1 time fewer per run (median count).
- Input `todo-app` (idea, sha256 f384a752a2cb): A to-do list web app for me and my partner that also works on the train without a connection
- Tree: `several`
  - the runs of trial-3-attempts and trial-3-batch-1; each run's tree is in those records
- Runs per variant: 3; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (complete-separation): held when each variant has at least 3 finished runs with the metric, every opus run lies below every haiku run and the medians differ by at least 1; contradicted when the ranges separate the other way or by less than 1; inconclusive otherwise. Guards decide adoption, not the verdict.

## Settings

| setting | haiku | opus |
|---|---|---|
| `adapter` | metaharness | metaharness |
| `autonomy` | A5 | A5 |
| `decider.escalate` | final | final |
| `decider.model` | claude-haiku-5-5 | claude-opus-5-5 |
| `models.implement.model` | claude-sonnet-5-5 | claude-sonnet-5-5 |
| `models.review.model` | claude-sonnet-5-5 | claude-sonnet-5-5 |
| `models.writer.model` | claude-sonnet-5-5 | claude-sonnet-5-5 |

Known confounds:

- `models.other.model`: the trial-3-mixed runs ran the other class (intake and direction writers) on claude-haiku-5-5, trial-3-sonnet-1 and trial-3-strong on claude-sonnet-5-5
- `max_llm_calls`: 90 in the trial-3-mixed runs, 150 in trial-3-sonnet-1 and trial-3-strong
- `bindings`: the runs read the bindings and tools of two worktrees (sdc-trial-3, sdc-trial-3b) that changed during the day; no run pins a revision of them
- `order`: trial-3-mixed, -2 ran in the morning before the intake and direction fixes; trial-3-mixed and -4 stopped in intake or direction on defects fixed since

## Runs

| run | variant | end | clean pass | cost USD | wall s | looping factor | note |
|---|---|---|---|---|---|---|---|
| trial-3-sonnet-1 | haiku | stopped: decider_budget_spent in IDEA-1.todo.access | no | 1.882 | 1444 | 2.4 |  |
| trial-3-mixed | haiku | stopped: awaiting_authority in IDEA-1.direction | no | 0.027 | 94 | 1.2 |  |
| trial-3-mixed-2 | haiku | stopped: awaiting_authority in IDEA-1.list.items | no | 0.886 | 888 | 1.273 |  |
| trial-3-mixed-3 | haiku | closed: not_delivered | no | 1.487 | 2096 | 1.417 |  |
| trial-3-mixed-4 | haiku | stopped: decision_not_takeable in IDEA-1.intake | no | 0.042 | 3036 | 1 |  |
| trial-3-mixed-5 | haiku | closed: not_delivered | no | 1.588 | 1692 | 1.364 |  |
| trial-3-mixed-6 | haiku | closed: operating | yes | 1.859 | 2295 | 1.333 |  |
| trial-3-strong-1 | opus | closed: operating | yes | 2.926 | 473 | 1 |  |
| trial-3-strong-2 | opus | closed: operating | yes | 2.996 | 454 | 1 |  |

## Per variant

Median [min, max] (n) over the ended runs.

| | haiku | opus |
|---|---|---|
| runs listed | 7 | 2 |
| ended | 7 | 2 |
| clean passes | 1 | 2 |
| `run.cost_usd` | 1.487 [0.0269, 1.8821] (7) | 2.9612 [2.9259, 2.9964] (2) |
| `run.llm_calls` | 15 [4, 24] (7) | 12 [12, 12] (2) |
| `run.decider_calls` | 13 [3, 20] (7) | 11 [11, 11] (2) |
| `run.wall_s` | 1692 [94, 3036] (7) | 463.5 [454, 473] (2) |
| `run.driver_s` | 432 [55, 775] (7) | 292 [278, 306] (2) |
| `run.real_takes` | 96 [24, 205] (7) | 84.5 [82, 87] (2) |
| `run.rework_takes` | 34 [13, 155] (7) | 16 [14, 18] (2) |
| `run.looping_factor` | 1.333 [1, 2.4] (7) | 1 [1, 1] (2) |
| `run.first_pass_yield` | 0.8 [0.727, 1] (7) | 1 [1, 1] (2) |
| `run.rework_cost_usd` | 0.325 [0, 0.9153] (7) | 0 [0, 0] (2) |
| `run.furthest_stage_index` | 6 [1, 7] (7) | 7 [7, 7] (2) |
| `run.findings_raised` | 4 [1, 7] (7) | 1 [0, 2] (2) |
| `run.waiting_on_person_s` | 0 [0, 2870] (7) | 0 [0, 0] (2) |
| `run.clean` | 0 [0, 1] (7) | 1 [1, 1] (2) |

## Loops

Per variant, over the ended runs: of the writer and approval nodes taken again at least once, the three with the highest median looping factor; and the three edges with the highest loop cost.

- haiku (n 7):
  - `spec/spec.author` (llm-writer): looping factor median 2 (max 15, n 5), rework takes 16, rework cost 0.987 USD
  - `direction/vision.write` (llm-writer): looping factor median 2 (max 3, n 5), rework takes 4, rework cost 0.012 USD
  - `story/change.implement` (llm-writer): looping factor median 1.667 (max 2.5, n 5), rework takes 9, rework cost 1.625 USD
  - `story: merge.approve:rejected -> change.implement` (approval): count 6 in 3 runs, loop cost 1.799 USD, loop driver 460 s
  - `spec: finding@spec -> intake` (finding): count 4 in 2 runs, loop cost 1.049 USD, loop driver 207 s
  - `story: finding@story -> intake` (finding): count 7 in 5 runs, loop cost 0.29 USD, loop driver 130 s
- opus (n 2):
  - `intake: finding@requirements -> intake` (finding): count 1 in 1 runs, loop cost 0 USD, loop driver 0 s
  - `story: finding@story -> intake` (finding): count 1 in 1 runs, loop cost 0 USD, loop driver 0 s

## Verdict

inconclusive: too few ended runs with the metric: haiku 5 and opus 2, the rule needs 3 each; retrospective: the hypothesis was written after the runs, so this verdict proposes a hypothesis and decides nothing.

Primary metric `edge.story: merge.approve:rejected -> change.implement.count` (decrease, at least 1.0):

- haiku: n 5, median 2.0, range [0.0, 2.0]
- opus: n 2, median 0.0, range [0.0, 0.0]
- guard `run.delivered`: holds (worsening -1)
- guard `run.cost_usd`: holds (worsening 1.474)
- recommendation: run more

## Links

- experiments/trial-3-attempts.yaml and experiments/trial-3-batch-1.yaml (the runs and their trees)
