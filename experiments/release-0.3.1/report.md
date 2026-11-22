# release-0.3.1: Release 0.3.1 on main ebea58d, the to-do idea in the three baseline settings

Written by `sdc-experiment report release-0.3.1` from `experiments/release-0.3.1.yaml` and the run summaries in `experiments/release-0.3.1/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: prospective; status: complete; created 2026-11-22.
- Purpose: Baseline of release 0.3.1, which replaces the stopped series of release 0.3.0 (record release-0.3.0, abandoned), and the registered comparison with release-0.1.2 on the same input and settings, with the four switches added since at their defaults.
- Hypothesis: none; the record describes its runs.
- Input `todo-app` (idea, sha256 f384a752a2cb): A to-do list web app for me and my partner that also works on the train without a connection
- Tree: `ebea58dee2d1b4c9326a07d4e70128fa922bd01b`
- Runs per variant: 3; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (descriptive): Registered sentence: on the to-do idea, release 0.3.1 delivers in at least as many runs per setting as release 0.1.2 did and run.cost_usd per setting is not higher by complete separation. The tree is the variable, which one record cannot hold (docs/improving.md step 4; a record with a hypothesis needs two variants of one tree that differ in one setting), so sdc-experiment reports this record as descriptive and the comparison with release-0.1.2 (same input, same settings, tree 72a3eef) is made per setting with the registered predictions below. Delivery: a setting meets P1 when its run.delivered count is at least release 0.1.2's (strong 3 of 3, mixed 2 of 3, haiku 2 of 3). Cost: n = 3 against n = 3, run.cost_usd is higher only when every 0.3.1 run of the setting costs more than every release-0.1.2 run of it (exact one-sided p = 1/20); overlapping ranges claim no effect. Every end counts; no run is repaired, reopened or forced. A refuted or inconclusive result is published as such.

## Settings

| setting | strong | mixed | haiku |
|---|---|---|---|
| `adapter` | metaharness | metaharness | metaharness |
| `autonomy` | A5 | A5 | A5 |
| `decider.adapter` | claude-cli | claude-cli | claude-cli |
| `decider.escalate` | final | final | final |
| `decider.kind` | model | model | model |
| `decider.max_decider_calls` | 40 | 40 | 40 |
| `decider.model` | claude-opus-5-5 | claude-haiku-5-5 | claude-haiku-5-5 |
| `implement_coverage` | off | off | off |
| `implement_mode` | session | session | session |
| `max_llm_calls` | 150 | 150 | 150 |
| `max_wall_seconds` | 5400 | 5400 | 5400 |
| `models.implement.model` | claude-sonnet-5-5 | claude-sonnet-5-5 | claude-haiku-5-5 |
| `models.other.model` | claude-sonnet-5-5 | claude-haiku-5-5 | claude-haiku-5-5 |
| `models.review.model` | claude-sonnet-5-5 | claude-sonnet-5-5 | claude-haiku-5-5 |
| `models.writer.model` | claude-sonnet-5-5 | claude-sonnet-5-5 | claude-haiku-5-5 |
| `review.adapter` | claude-cli | claude-cli | claude-cli |
| `review_mode` | session | session | session |
| `rule_approvals` | off | off | off |

Known confounds:

- `unstick`: run.unstick (D147, 0.1.1) is on under autopilot unless SDC_UNSTICK=off, with the model claude-opus-5-5 unless SDC_LLM_MODEL_UNSTICK is set (scripts/run-trial.sh); neither is a launch setting of sdc-experiment, so every variant runs with an Opus unstick, as in release-0.1.2. An unstuck run is not a clean pass.
- `canon`: release-0.1.2 used the canon binary of the canon checkout at run time; 0.3.1 pins Canon 0.10.0 (driver/canon-pin.json) and every script refuses another version, so the two series differ in the Canon version.
- `ess`: sdc-tool.sh runs ess 0.96.0 whatever ess is on PATH (docs/releases/0.3.0.md); release-0.1.2 used the installed ess.
- `instructions`: what a model call is told moved from code to files under instructions/ (622819b) with 40 golden prompts held byte for byte; its effect is unmeasured (experiments/releases.yaml).
- `launcher`: both series are started by sdc-experiment run, with experiment.json; the run directory and the build of the tree are made by the tool.
- `switches`: SDC_IMPLEMENT_MODE, SDC_RULE_APPROVALS, SDC_REVIEW_MODE and SDC_IMPLEMENT_COVERAGE are set in `fixed` to their defaults (session, off, session, off), so run-settings.jsonl records them and none of the four is on; no 0.3.1 run measures a switch.
- `pool-wait`: the wait for a slot of the shared build pool is bounded and reported apart in 0.3.1 (exit 69 repeated, then pool_wait_spent; exit 70, run_over_time), which release-0.1.2 did not have; a run of 0.3.1 that waits in the pool ends differently from a run of 0.1.2 that waited, and the queue of the host differs between the two series.

## Runs

| run | variant | end | clean pass | cost USD | wall s | looping factor | note |
|---|---|---|---|---|---|---|---|
| release-0.3.1.strong.1 | strong | closed: operating | yes | 3.468 | 710 | 1.182 |  |
| release-0.3.1.strong.2 | strong | closed: operating | yes | 5.475 | 1164 | 1.333 |  |
| release-0.3.1.strong.3 | strong | closed: operating | yes | 6.597 | 1432 | 1.417 |  |
| release-0.3.1.mixed.1 | mixed | closed: operating | yes | 2.943 | 1579 | 1.5 |  |
| release-0.3.1.mixed.2 | mixed | closed: operating | yes | 2.545 | 1790 | 1.333 |  |
| release-0.3.1.mixed.3 | mixed | closed: operating | yes | 2.291 | 1574 | 1.667 |  |
| release-0.3.1.haiku.1 | haiku | closed: operating | no | 1.54 | 3824 | 2.455 |  |
| release-0.3.1.haiku.2 | haiku | closed: operating | yes | 1.223 | 2692 | 1.455 |  |
| release-0.3.1.haiku.3 | haiku | stopped: writer_spent in IDEA-1.list.spec | no | 0.396 | 1680 | 2.25 |  |

## Per variant

Median [min, max] (n) over the ended runs.

| | strong | mixed | haiku |
|---|---|---|---|
| runs listed | 3 | 3 | 3 |
| ended | 3 | 3 | 3 |
| clean passes | 3 | 3 | 1 |
| `run.cost_usd` | 5.4754 [3.4682, 6.597] (3) | 2.5446 [2.2907, 2.9431] (3) | 1.2227 [0.3962, 1.54] (3) |
| `run.llm_calls` | 20 [15, 22] (3) | 22 [19, 24] (3) | 19 [18, 29] (3) |
| `run.decider_calls` | 13 [11, 13] (3) | 16 [13, 17] (3) | 13 [12, 19] (3) |
| `run.wall_s` | 1164 [710, 1432] (3) | 1579 [1574, 1790] (3) | 2692 [1680, 3824] (3) |
| `run.driver_s` | 878 [468, 1110] (3) | 851 [841, 865] (3) | 1904 [1268, 2721] (3) |
| `run.real_takes` | 123 [105, 129] (3) | 115 [110, 124] (3) | 124 [122, 211] (3) |
| `run.rework_takes` | 51 [41, 57] (3) | 45 [40, 53] (3) | 89 [57, 146] (3) |
| `run.looping_factor` | 1.333 [1.182, 1.417] (3) | 1.5 [1.333, 1.667] (3) | 2.25 [1.455, 2.455] (3) |
| `run.first_pass_yield` | 0.667 [0.583, 0.818] (3) | 0.667 [0.583, 0.833] (3) | 0.727 [0.727, 0.75] (3) |
| `run.rework_cost_usd` | 0.6818 [0.1215, 1.0133] (3) | 0.3703 [0.0217, 0.488] (3) | 0.8064 [0.2123, 0.8462] (3) |
| `run.furthest_stage_index` | 7 [7, 7] (3) | 7 [7, 7] (3) | 7 [4, 7] (3) |
| `run.findings_raised` | 9 [4, 11] (3) | 8 [7, 12] (3) | 22 [12, 28] (3) |
| `run.waiting_on_person_s` | 0 [0, 0] (3) | 0 [0, 0] (3) | 0 [0, 0] (3) |
| `run.clean` | 1 [1, 1] (3) | 1 [1, 1] (3) | 0 [0, 1] (3) |

## Loops

Per variant, over the ended runs: of the writer and approval nodes taken again at least once, the three with the highest median looping factor; and the three edges with the highest loop cost.

- strong (n 3):
  - `spec/spec.author` (llm-writer): looping factor median 2 (max 2, n 3), rework takes 3, rework cost 0.385 USD
  - `direction/vision.write` (llm-writer): looping factor median 2 (max 2, n 3), rework takes 2, rework cost 0.098 USD
  - `intake/idea.dissect` (llm-writer): looping factor median 2 (max 2, n 3), rework takes 3, rework cost 0.074 USD
  - `direction: finding@vision -> intake` (finding): count 2 in 2 runs, loop cost 1.282 USD, loop driver 55 s
  - `story: change.review:rejected -> change.implement` (check): count 3 in 2 runs, loop cost 1.259 USD, loop driver 226 s
  - `spec: finding@spec -> intake` (finding): count 6 in 3 runs, loop cost 0.543 USD, loop driver 68 s
- mixed (n 3):
  - `direction/roadmap.write` (llm-writer): looping factor median 3 (max 3, n 3), rework takes 5, rework cost 0.026 USD
  - `direction/vision.write` (llm-writer): looping factor median 3 (max 3, n 3), rework takes 6, rework cost 0.025 USD
  - `direction/vision.approve` (approval): looping factor median 3 (max 3, n 3), rework takes 5, rework cost 0 USD
  - `story: change.review:rejected -> change.implement` (check): count 2 in 2 runs, loop cost 0.686 USD, loop driver 129 s
  - `direction: finding@roadmap -> intake` (finding): count 9 in 3 runs, loop cost 0.179 USD, loop driver 217 s
  - `spec: spec.validate:invalid -> spec.author` (check): count 3 in 2 runs, loop cost 0.128 USD, loop driver 57 s
- haiku (n 3):
  - `spec/spec.author` (llm-writer): looping factor median 10 (max 15, n 3), rework takes 23, rework cost 0.744 USD
  - `story/change.implement` (llm-writer): looping factor median 2.25 (max 3, n 2), rework takes 5, rework cost 1.099 USD
  - `direction/vision.write` (llm-writer): looping factor median 2 (max 2, n 3), rework takes 2, rework cost 0.008 USD
  - `story: finding@story -> intake` (finding): count 27 in 2 runs, loop cost 1.076 USD, loop driver 1205 s
  - `spec: spec.synthesize:refused -> spec.author` (check): count 10 in 2 runs, loop cost 0.504 USD, loop driver 1091 s
  - `spec: finding@spec -> intake` (finding): count 23 in 3 runs, loop cost 0.139 USD, loop driver 451 s

## Verdict

descriptive: no hypothesis: the record describes its runs and decides nothing.

## Facts

- Tree: main at ebea58dee2d1b4c9326a07d4e70128fa922bd01b, the merge of pull request 5 and the tag 0.3.1 (experiments/releases.yaml, 0.3.1: no run has used this tree yet).
- Replaces release-0.3.0 on tree ae7a697: that record was abandoned after 8 of 9 runs started, because the test step waited in the shared build pool and was recorded as a failed run, so the runs measured the queue; 0.3.1 fixes that (a step that got no build slot is not a failed test run). 0.3.0 has no registration numbers.
- Comparison basis: experiments/release-0.1.2/report.md, input todo-app (sha256 f384a752a2cb), the same three variants and the same models; strong 3 of 3 clean passes, mixed 2 of 3, haiku 2 of 3; the two stops were writer_spent in IDEA-1.sharedlist.list (mixed.1) and in IDEA-1.list.sync (haiku.1).
- Not launched: this record is committed before any run.

## Links

- experiments/release-0.1.2.yaml: the baseline this record is compared with
- experiments/release-0.3.0.yaml: the stopped series this record replaces
- experiments/releases.yaml: the ledger entry of 0.3.1, whose limit says no run has used this tree yet
