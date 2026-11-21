# review-haiku-replay: change.review on Haiku against the recorded Sonnet reviews of a strong run, by recorded replay

Written by `sdc-experiment report review-haiku-replay` from `experiments/review-haiku-replay.yaml` and the run summaries in `experiments/review-haiku-replay/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: prospective; status: planned; created 2026-11-21.
- Purpose: Whether the review class can move from Sonnet to Haiku (0.75 to about 0.04 USD per strong run) without changing a review decision, tested on recorded inputs before any full run.
- Hypothesis: none; the record describes its runs.
- Input `todo-app` (idea, sha256 f384a752a2cb): A to-do list web app for me and my partner that also works on the train without a connection
- Tree: `72a3eefb20e0edff50dc8ad12fcf561f8e0790e3`
- Runs per variant: 3; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (descriptive): Three replays of release-0.1.2.strong.1 (tree 72a3eef, its 5 change.review calls on Sonnet), each with change.review on Haiku and every other call recorded. Launched by hand, not by sdc-experiment run, which passes no divergence policy: SDC_LLM_MODEL_REVIEW=claude-haiku-5-5 scripts/run-trial.sh review-haiku-replay-<n> --autopilot --recorded runs/release-0.1.2.strong.1 --on-divergence live --live-adapter claude-cli --live-cap 5 (experiments/baseline-0.1.0/models.md). Every review call diverges, because the model is part of the request digest, and goes live; any other live call means a review decision differed and sent the run down another path. Pass, per replay: live calls = review calls, every live review returns the recorded decision, a rejection names the same requirement ids, and sdc-replay compare exits 0. Passed in all 3 replays: a full-run record with models.review.model claude-haiku-5-5 against release-0.1.2 strong follows. Any decision differs: the differing reviews are listed with both replies, and the review class stays on Sonnet until the review gets its smallest fact set (record review-minimal-facts) and is tested again. Not decided here: review quality where both models agree on a wrong decision.

## Settings

| setting | haiku-review |
|---|---|
| `adapter` | metaharness |
| `autonomy` | A5 |
| `decider.adapter` | claude-cli |
| `decider.escalate` | final |
| `decider.kind` | model |
| `decider.max_decider_calls` | 40 |
| `decider.model` | claude-opus-5-5 |
| `flags.recorded` | release-0.1.2.strong.1 |
| `max_llm_calls` | 150 |
| `models.implement.model` | claude-sonnet-5-5 |
| `models.other.model` | claude-sonnet-5-5 |
| `models.review.model` | claude-haiku-5-5 |
| `models.writer.model` | claude-sonnet-5-5 |
| `review.adapter` | claude-cli |

Known confounds:

- `tree`: the replay must run on the source's tree 72a3eef; on a later tree a changed input diverges and goes live as well, which this record counts as a differing decision.
- `prompt-size`: the fourth and fifth reviews of a strong run pass 100K tokens, where Haiku's long-prompt price applies and its quality on long prompts is untested.
- `one-source`: one source run, so the 3 replays measure Haiku's agreement on the same 5 inputs, not over different runs.

## Runs

| run | variant | end | clean pass | cost USD | wall s | looping factor | note |
|---|---|---|---|---|---|---|---|
| review-haiku-replay-1 | haiku-review | no summary: not ended | - | - | - | - |  |
| review-haiku-replay-2 | haiku-review | no summary: not ended | - | - | - | - |  |
| review-haiku-replay-3 | haiku-review | no summary: not ended | - | - | - | - |  |

## Per variant

Median [min, max] (n) over the ended runs.

| | haiku-review |
|---|---|
| runs listed | 3 |
| ended | 0 |
| clean passes | 0 |
| `run.cost_usd` | - |
| `run.llm_calls` | - |
| `run.decider_calls` | - |
| `run.wall_s` | - |
| `run.driver_s` | - |
| `run.real_takes` | - |
| `run.rework_takes` | - |
| `run.looping_factor` | - |
| `run.first_pass_yield` | - |
| `run.rework_cost_usd` | - |
| `run.furthest_stage_index` | - |
| `run.findings_raised` | - |
| `run.waiting_on_person_s` | - |
| `run.clean` | - |

## Loops

Per variant, over the ended runs: of the writer and approval nodes taken again at least once, the three with the highest median looping factor; and the three edges with the highest loop cost.

- haiku-review (n 0):
  - no node taken again, no edge

## Verdict

descriptive: no hypothesis: the record describes its runs and decides nothing.

## Facts

- experiments/baseline-0.1.0/models.md: review per strong run 0.75 USD on Sonnet; 5 rejections in three strong runs, all real gaps; Haiku reviews of trial-0.1.0-haiku-1 rejected 3 times, real; never compared on the same inputs.
- The merge decider sees the review as change.reviewed true in checks.json, not its findings (release-0.1.2.strong.1, decider/009-merge.approve), so an agreeing review leaves later inputs as recorded (inferred).

## Links

- experiments/baseline-0.1.0/models.md: the per-class table and the replay design
- experiments/baseline-0.1.0/costs.md: the budget and the target
- experiments/release-0.1.2.yaml: the source run
