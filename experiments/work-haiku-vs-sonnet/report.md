# work-haiku-vs-sonnet: Work nodes on Haiku against Sonnet with a Haiku decider

Written by `sdc-experiment report work-haiku-vs-sonnet` from `experiments/work-haiku-vs-sonnet.yaml` and the run summaries in `experiments/work-haiku-vs-sonnet/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: retrospective; status: complete; created 2026-11-21.
- Purpose: Whether Sonnet on the work nodes lowers change.implement's looping factor against Haiku, read from the trial-3 runs that exist.
- Hypothesis: With claude-sonnet-5-5 instead of claude-haiku-5-5 on the implement class (the writer and review classes changed with it), the looping factor of story/change.implement falls by at least 0.5.
- Input `todo-app` (idea, sha256 f384a752a2cb): A to-do list web app for me and my partner that also works on the train without a connection
- Tree: `several`
  - the runs of trial-3-attempts and trial-3-batch-1; each run's tree is in those records
- Runs per variant: 3; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (complete-separation): held when each variant has at least 3 finished runs that took change.implement, every sonnet run's looping factor lies below every haiku run's and the medians differ by at least 0.5; contradicted when the ranges separate the other way or by less than 0.5; inconclusive otherwise. Guards decide adoption, not the verdict.

## Settings

| setting | haiku | sonnet |
|---|---|---|
| `adapter` | metaharness | metaharness |
| `autonomy` | A5 | A5 |
| `decider.escalate` | final | final |
| `decider.model` | claude-haiku-5-5 | claude-haiku-5-5 |
| `models.implement.model` | claude-haiku-5-5 | claude-sonnet-5-5 |

Known confounds:

- `models.writer.model`: changed together with the variable in every run (claude-haiku-5-5 against claude-sonnet-5-5)
- `models.review.model`: null in the haiku runs (change.review and spec.review were program checks), claude-sonnet-5-5 in the sonnet runs (model reviews)
- `models.other.model`: claude-haiku-5-5 in the trial-3-mixed runs, claude-sonnet-5-5 in trial-3-sonnet-1
- `max_llm_calls`: 90 in most runs, 150 in trial-3-haiku-6, -7 and trial-3-sonnet-1
- `bindings`: the runs read the bindings and tools of two worktrees (sdc-trial-3, sdc-trial-3b) that changed during the day; no run pins a revision of them

## Runs

| run | variant | end | clean pass | cost USD | wall s | looping factor | note |
|---|---|---|---|---|---|---|---|
| trial-3-haiku | haiku | stopped: awaiting_authority in IDEA-1.direction | no | 0.026 | 91 | 1 |  |
| trial-3-haiku-2 | haiku | stopped: writer_spent in IDEA-1.list.spec | no | 0.211 | 786 | 1.75 |  |
| trial-3-haiku-3 | haiku | stopped: rejected_without_rework_path in IDEA-1.app.release | no | 0.469 | 1609 | 1.545 |  |
| trial-3-haiku-4 | haiku | stopped: writer_spent in IDEA-1.todo.list | no | 0.669 | 2107 | 2 |  |
| trial-3-haiku-5 | haiku | closed: operating | no | 0.779 | 3106 | 1.636 |  |
| trial-3-haiku-6 | haiku | closed: not_delivered | no | 0.56 | 1916 | 1.417 |  |
| trial-3-haiku-7 | haiku | closed: operating | yes | 1.034 | 2709 | 1.417 |  |
| trial-3-sonnet-1 | sonnet | stopped: decider_budget_spent in IDEA-1.todo.access | no | 1.882 | 1444 | 2.4 |  |
| trial-3-mixed | sonnet | stopped: awaiting_authority in IDEA-1.direction | no | 0.027 | 94 | 1.2 |  |
| trial-3-mixed-2 | sonnet | stopped: awaiting_authority in IDEA-1.list.items | no | 0.886 | 888 | 1.273 |  |
| trial-3-mixed-3 | sonnet | closed: not_delivered | no | 1.487 | 2096 | 1.417 |  |
| trial-3-mixed-4 | sonnet | stopped: decision_not_takeable in IDEA-1.intake | no | 0.042 | 3036 | 1 |  |
| trial-3-mixed-5 | sonnet | closed: not_delivered | no | 1.588 | 1692 | 1.364 |  |
| trial-3-mixed-6 | sonnet | closed: operating | yes | 1.859 | 2295 | 1.333 |  |

## Per variant

Median [min, max] (n) over the ended runs.

| | haiku | sonnet |
|---|---|---|
| runs listed | 7 | 7 |
| ended | 7 | 7 |
| clean passes | 1 | 1 |
| `run.cost_usd` | 0.5597 [0.0264, 1.0337] (7) | 1.487 [0.0269, 1.8821] (7) |
| `run.llm_calls` | 16 [5, 18] (7) | 15 [4, 24] (7) |
| `run.decider_calls` | 14 [4, 17] (7) | 13 [3, 20] (7) |
| `run.wall_s` | 1916 [91, 3106] (7) | 1692 [94, 3036] (7) |
| `run.driver_s` | 983 [65, 1435] (7) | 432 [55, 775] (7) |
| `run.real_takes` | 122 [29, 128] (7) | 96 [24, 205] (7) |
| `run.rework_takes` | 56 [13, 64] (7) | 34 [13, 155] (7) |
| `run.looping_factor` | 1.545 [1, 2] (7) | 1.333 [1, 2.4] (7) |
| `run.first_pass_yield` | 0.667 [0.636, 1] (7) | 0.8 [0.727, 1] (7) |
| `run.rework_cost_usd` | 0.0705 [0, 0.3208] (7) | 0.325 [0, 0.9153] (7) |
| `run.furthest_stage_index` | 7 [2, 7] (7) | 6 [1, 7] (7) |
| `run.findings_raised` | 15 [2, 19] (7) | 4 [1, 7] (7) |
| `run.waiting_on_person_s` | 0 [0, 553] (7) | 0 [0, 2870] (7) |
| `run.clean` | 0 [0, 1] (7) | 0 [0, 1] (7) |

## Loops

Per variant, over the ended runs: of the writer and approval nodes taken again at least once, the three with the highest median looping factor; and the three edges with the highest loop cost.

- haiku (n 7):
  - `spec/spec.author` (llm-writer): looping factor median 3 (max 7, n 6), rework takes 14, rework cost 0.129 USD
  - `story/change.implement` (llm-writer): looping factor median 2 (max 4.5, n 5), rework takes 17, rework cost 0.794 USD
  - `direction/vision.write` (llm-writer): looping factor median 2 (max 3, n 6), rework takes 6, rework cost 0.018 USD
  - `story: merge.approve:rejected -> change.implement` (approval): count 4 in 3 runs, loop cost 0.775 USD, loop driver 790 s
  - `story: finding@story -> intake` (finding): count 47 in 5 runs, loop cost 0.342 USD, loop driver 551 s
  - `direction: finding@roadmap -> intake` (finding): count 4 in 3 runs, loop cost 0.12 USD, loop driver 76 s
- sonnet (n 7):
  - `spec/spec.author` (llm-writer): looping factor median 2 (max 15, n 5), rework takes 16, rework cost 0.987 USD
  - `direction/vision.write` (llm-writer): looping factor median 2 (max 3, n 5), rework takes 4, rework cost 0.012 USD
  - `story/change.implement` (llm-writer): looping factor median 1.667 (max 2.5, n 5), rework takes 9, rework cost 1.625 USD
  - `story: merge.approve:rejected -> change.implement` (approval): count 6 in 3 runs, loop cost 1.799 USD, loop driver 460 s
  - `spec: finding@spec -> intake` (finding): count 4 in 2 runs, loop cost 1.049 USD, loop driver 207 s
  - `story: finding@story -> intake` (finding): count 7 in 5 runs, loop cost 0.29 USD, loop driver 130 s

## Verdict

inconclusive: the ranges overlap (median difference -0.333): no effect is claimed; retrospective: the hypothesis was written after the runs, so this verdict proposes a hypothesis and decides nothing.

Primary metric `node.story/change.implement.looping_factor` (decrease, at least 0.5):

- haiku: n 5, median 2.0, range [1.333, 4.5]
- sonnet: n 5, median 1.667, range [1.0, 2.5]
- guard `run.delivered`: holds (worsening 0)
- guard `run.cost_usd`: fails (worsening 0.927)
- recommendation: drop

## Links

- experiments/trial-3-attempts.yaml and experiments/trial-3-batch-1.yaml (the runs and their trees)
