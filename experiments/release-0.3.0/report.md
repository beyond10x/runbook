# release-0.3.0: Release 0.3.0 on main ae7a697, the to-do idea in the three baseline settings

Written by `sdc-experiment report release-0.3.0` from `experiments/release-0.3.0.yaml` and the run summaries in `experiments/release-0.3.0/runs/`; `sdc-experiment check --all` fails when it is stale. Definitions: `docs/workflow-metrics.md`.

- Kind: prospective; status: abandoned; created 2026-11-22.
- Purpose: Baseline of release 0.3.0, and the registered comparison with release-0.1.2 on the same input and settings, with the four switches added since at their defaults.
- Hypothesis: none; the record describes its runs.
- Input `todo-app` (idea, sha256 f384a752a2cb): A to-do list web app for me and my partner that also works on the train without a connection
- Tree: `ae7a697616857a2bf2744c85a33e58b0b5927dc8`
- Runs per variant: 3; pass rule: clean-delivery (closed, delivered, no reopen, forced step or unstick).
- Decision rule (descriptive): Registered sentence: on the to-do idea, release 0.3.0 delivers in at least as many runs per setting as release 0.1.2 did and run.cost_usd per setting is not higher by complete separation. The tree is the variable, which one record cannot hold (docs/improving.md step 4; a record with a hypothesis needs two variants of one tree that differ in one setting), so sdc-experiment reports this record as descriptive and the comparison with release-0.1.2 (same input, same settings, tree 72a3eef) is made per setting with the registered predictions below. Delivery: a setting meets P1 when its run.delivered count is at least release 0.1.2's (strong 3 of 3, mixed 2 of 3, haiku 2 of 3). Cost: n = 3 against n = 3, run.cost_usd is higher only when every 0.3.0 run of the setting costs more than every release-0.1.2 run of it (exact one-sided p = 1/20); overlapping ranges claim no effect. Every end counts; no run is repaired, reopened or forced. A refuted or inconclusive result is published as such.

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
- `canon`: release-0.1.2 used the canon binary of the canon checkout at run time; 0.3.0 pins Canon 0.10.0 (driver/canon-pin.json) and every script refuses another version, so the two series differ in the Canon version.
- `ess`: sdc-tool.sh runs ess 0.96.0 whatever ess is on PATH (docs/releases/0.3.0.md); release-0.1.2 used the installed ess.
- `instructions`: what a model call is told moved from code to files under instructions/ (622819b) with 40 golden prompts held byte for byte; its effect is unmeasured (experiments/releases.yaml).
- `launcher`: both series are started by sdc-experiment run, with experiment.json; the run directory and the build of the tree are made by the tool.
- `switches`: SDC_IMPLEMENT_MODE, SDC_RULE_APPROVALS, SDC_REVIEW_MODE and SDC_IMPLEMENT_COVERAGE are set in `fixed` to their defaults (session, off, session, off), so run-settings.jsonl records them and none of the four is on; no 0.3.0 run measures a switch.

## Runs

| run | variant | end | clean pass | cost USD | wall s | looping factor | note |
|---|---|---|---|---|---|---|---|
| release-0.3.0.strong.1 | strong | stopped: awaiting_person in IDEA-1.direction | no | 1.901 | 2268 | 1.2 |  |
| release-0.3.0.strong.2 | strong | stopped: awaiting_person in IDEA-1.direction | no | 1.711 | 2231 | 1.1 |  |
| release-0.3.0.strong.3 | strong | stopped: awaiting_person in IDEA-1.direction | no | 2.079 | 495 | 1.3 |  |
| release-0.3.0.mixed.1 | mixed | stopped: awaiting_person in IDEA-1.direction | no | 0.595 | 679 | 1.3 |  |
| release-0.3.0.mixed.2 | mixed | stopped: awaiting_person in IDEA-1.direction | no | 0.588 | 593 | 1 |  |
| release-0.3.0.mixed.3 | mixed | stopped: awaiting_person in IDEA-1.intake | no | 0.044 | 225 | 1 |  |
| release-0.3.0.haiku.1 | haiku | stopped: writer_spent in IDEA-1.list.spec | no | 0.688 | 2052 | 2.375 |  |
| release-0.3.0.haiku.2 | haiku | stopped: writer_spent in IDEA-1.todo.spec | no | 0.295 | 1284 | 1.75 |  |

## Per variant

Median [min, max] (n) over the ended runs.

| | strong | mixed | haiku |
|---|---|---|---|
| runs listed | 3 | 3 | 2 |
| ended | 3 | 3 | 2 |
| clean passes | 0 | 0 | 0 |
| `run.cost_usd` | 1.9011 [1.7105, 2.0788] (3) | 0.5882 [0.0444, 0.5953] (3) | 0.4914 [0.2945, 0.6882] (2) |
| `run.llm_calls` | 12 [11, 13] (3) | 10 [4, 13] (3) | 16.5 [14, 19] (2) |
| `run.decider_calls` | 7 [7, 8] (3) | 8 [4, 10] (3) | 10 [10, 10] (2) |
| `run.wall_s` | 2231 [495, 2268] (3) | 593 [225, 679] (3) | 1668 [1284, 2052] (2) |
| `run.driver_s` | 107 [89, 109] (3) | 159 [89, 227] (3) | 1198 [943, 1453] (2) |
| `run.real_takes` | 76 [68, 82] (3) | 66 [24, 82] (3) | 117.5 [79, 156] (2) |
| `run.rework_takes` | 27 [20, 33] (3) | 20 [13, 36] (3) | 83 [44, 122] (2) |
| `run.looping_factor` | 1.2 [1.1, 1.3] (3) | 1 [1, 1.3] (3) | 2.0625 [1.75, 2.375] (2) |
| `run.first_pass_yield` | 0.8 [0.8, 0.9] (3) | 1 [0.9, 1] (3) | 0.8125 [0.75, 0.875] (2) |
| `run.rework_cost_usd` | 0.2603 [0.1824, 0.2644] (3) | 0 [0, 0.0133] (3) | 0.2451 [0.1488, 0.3415] (2) |
| `run.furthest_stage_index` | 6 [6, 6] (3) | 6 [1, 6] (3) | 4 [4, 4] (2) |
| `run.findings_raised` | 3 [2, 4] (3) | 4 [2, 5] (3) | 13.5 [11, 16] (2) |
| `run.waiting_on_person_s` | 2104 [628, 2183] (3) | 1866 [63, 1883] (3) | 0 [0, 0] (2) |
| `run.clean` | 0 [0, 0] (3) | 0 [0, 0] (3) | 0 [0, 0] (2) |

## Loops

Per variant, over the ended runs: of the writer and approval nodes taken again at least once, the three with the highest median looping factor; and the three edges with the highest loop cost.

- strong (n 3):
  - `story/change.implement` (llm-writer): looping factor median 2 (max 2, n 3), rework takes 2, rework cost 0.397 USD
  - `direction/vision.write` (llm-writer): looping factor median 2 (max 2, n 3), rework takes 2, rework cost 0.094 USD
  - `spec/spec.author` (llm-writer): looping factor median 1 (max 3, n 3), rework takes 2, rework cost 0.216 USD
  - `direction: finding@vision -> intake` (finding): count 2 in 2 runs, loop cost 1.044 USD, loop driver 54 s
  - `story: tests.run:failed -> change.implement` (check): count 2 in 2 runs, loop cost 0.397 USD, loop driver 105 s
  - `spec: spec.validate:invalid -> spec.author` (check): count 2 in 1 runs, loop cost 0.216 USD, loop driver 35 s
- mixed (n 3):
  - `direction/vision.write` (llm-writer): looping factor median 2.5 (max 4, n 2), rework takes 3, rework cost 0.013 USD
  - `direction: finding@vision -> intake` (finding): count 3 in 1 runs, loop cost 0.025 USD, loop driver 57 s
  - `direction: finding@context -> intake` (finding): count 3 in 2 runs, loop cost 0.008 USD, loop driver 19 s
  - `intake: finding@requirements -> intake` (finding): count 5 in 3 runs, loop cost 0 USD, loop driver 0 s
- haiku (n 2):
  - `spec/spec.author` (llm-writer): looping factor median 9 (max 12, n 2), rework takes 16, rework cost 0.487 USD
  - `direction/vision.write` (llm-writer): looping factor median 1.5 (max 2, n 2), rework takes 1, rework cost 0.004 USD
  - `spec: spec.synthesize:refused -> spec.author` (check): count 7 in 2 runs, loop cost 0.28 USD, loop driver 725 s
  - `spec: spec.validate:invalid -> spec.author` (check): count 7 in 2 runs, loop cost 0.115 USD, loop driver 542 s
  - `spec: finding@spec -> intake` (finding): count 15 in 2 runs, loop cost 0.099 USD, loop driver 259 s

## Verdict

descriptive: no hypothesis: the record describes its runs and decides nothing.

## Facts

- Tree: main at ae7a697616857a2bf2744c85a33e58b0b5927dc8, the merge of pull request 2 and the tag 0.3.0 (experiments/releases.yaml, 0.3.0: no run has used this tree).
- Comparison basis: experiments/release-0.1.2/report.md, input todo-app (sha256 f384a752a2cb), the same three variants and the same models; strong 3 of 3 clean passes, mixed 2 of 3, haiku 2 of 3; the two stops were writer_spent in IDEA-1.sharedlist.list (mixed.1) and in IDEA-1.list.sync (haiku.1).
- Not launched: this record is committed before any run.

## Links

- experiments/release-0.1.2.yaml: the baseline this record is compared with
- experiments/releases.yaml: the ledger entry of 0.3.0, whose limit says no run has used this tree
